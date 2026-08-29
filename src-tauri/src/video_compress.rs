use std::path::PathBuf;
use std::process::Command;

use crate::config::AppConfig;
use crate::notify_system;
use tauri::AppHandle;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

const DISCORD_LIMIT_MB: u64 = 8;
const OVERHEAD_FACTOR: f64 = 0.98;
const AUDIO_BITRATE_BPS: u64 = 128_000;
const MAX_RETRIES: u32 = 3;
const CREATE_NO_WINDOW: u32 = 0x08000000;

fn hide_window(cmd: &mut Command) {
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);
}

fn find_ffmpeg() -> Result<String, String> {
    let config = AppConfig::load();
    if !config.ffmpeg_path.trim().is_empty() {
        let configured = config.ffmpeg_path.trim();
        let mut cmd = Command::new(configured);
        hide_window(&mut cmd);
        if cmd.arg("-version").output().is_ok() {
            return Ok(configured.to_string());
        }
        return Err(format!("FFmpeg configurado no funciona: {}", configured));
    }

    let candidates = [
        "ffmpeg",
        "C:\\ffmpeg\\bin\\ffmpeg.exe",
        "C:\\Program Files\\ffmpeg\\bin\\ffmpeg.exe",
    ];
    for c in &candidates {
        let mut cmd = Command::new(c);
        hide_window(&mut cmd);
        if cmd.arg("-version").output().is_ok() {
            return Ok(c.to_string());
        }
    }
    Err("FFmpeg no encontrado. Instalalo y agregalo al PATH".to_string())
}

fn find_ffprobe() -> Result<String, String> {
    let config = AppConfig::load();
    if !config.ffprobe_path.trim().is_empty() {
        let configured = config.ffprobe_path.trim();
        let mut cmd = Command::new(configured);
        hide_window(&mut cmd);
        if cmd.arg("-version").output().is_ok() {
            return Ok(configured.to_string());
        }
        return Err(format!("FFprobe configurado no funciona: {}", configured));
    }

    let candidates = [
        "ffprobe",
        "C:\\ffmpeg\\bin\\ffprobe.exe",
        "C:\\Program Files\\ffmpeg\\bin\\ffprobe.exe",
    ];
    for c in &candidates {
        let mut cmd = Command::new(c);
        hide_window(&mut cmd);
        if cmd.arg("-version").output().is_ok() {
            return Ok(c.to_string());
        }
    }
    Err("FFprobe no encontrado".to_string())
}

fn get_duration_secs(input: &str, ffprobe: &str) -> Result<f64, String> {
    let mut cmd = Command::new(ffprobe);
    hide_window(&mut cmd);
    let output = cmd
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "csv=p=0",
            input,
        ])
        .output()
        .map_err(|e| format!("Error ejecutando ffprobe: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let duration: f64 = stdout
        .trim()
        .parse()
        .map_err(|_| format!("No pude leer la duración: {}", stdout))?;

    Ok(duration)
}

fn compress_once(
    input: &str,
    output: &str,
    video_bitrate_kbps: u64,
    audio_bitrate_kbps: u64,
    ffmpeg: &str,
) -> Result<(), String> {
    let video_bitrate = format!("{}k", video_bitrate_kbps);
    let audio_bitrate = format!("{}k", audio_bitrate_kbps);

    let mut cmd1 = Command::new(ffmpeg);
    hide_window(&mut cmd1);
    let pass1 = cmd1
        .args([
            "-y",
            "-i",
            input,
            "-c:v",
            "libx264",
            "-b:v",
            &video_bitrate,
            "-preset",
            "fast",
            "-pass",
            "1",
            "-an",
            "-f",
            "mp4",
            if cfg!(target_os = "windows") {
                "NUL"
            } else {
                "/dev/null"
            },
        ])
        .output()
        .map_err(|e| format!("Error en pass 1: {}", e))?;

    if !pass1.status.success() {
        let stderr = String::from_utf8_lossy(&pass1.stderr);
        return Err(format!(
            "FFmpeg pass 1 falló: {}",
            stderr.chars().take(200).collect::<String>()
        ));
    }

    let mut cmd2 = Command::new(ffmpeg);
    hide_window(&mut cmd2);
    let pass2 = cmd2
        .args([
            "-y",
            "-i",
            input,
            "-c:v",
            "libx264",
            "-b:v",
            &video_bitrate,
            "-preset",
            "fast",
            "-pass",
            "2",
            "-c:a",
            "aac",
            "-b:a",
            &audio_bitrate,
            "-movflags",
            "+faststart",
            output,
        ])
        .output()
        .map_err(|e| format!("Error en pass 2: {}", e))?;

    if !pass2.status.success() {
        let stderr = String::from_utf8_lossy(&pass2.stderr);
        return Err(format!(
            "FFmpeg pass 2 falló: {}",
            stderr.chars().take(200).collect::<String>()
        ));
    }

    Ok(())
}

fn compress_sync(input: String) -> Result<String, String> {
    let ffmpeg = find_ffmpeg()?;
    let ffprobe = find_ffprobe()?;

    let input_path = PathBuf::from(&input);
    if !input_path.exists() {
        return Err(format!("No encontré el archivo: {}", input));
    }

    let target_bytes = DISCORD_LIMIT_MB * 1024 * 1024;
    let duration = get_duration_secs(&input, &ffprobe)?;

    if duration <= 0.0 {
        return Err("No pude obtener la duración del video".to_string());
    }

    let output_path = input_path.with_file_name(format!(
        "{}_discord.mp4",
        input_path.file_stem().unwrap_or_default().to_string_lossy()
    ));
    let output = output_path.to_string_lossy().to_string();

    eprintln!(
        "[NEEKO] Comprimiendo: {} -> {} (duración: {:.1}s)",
        input, output, duration
    );

    for attempt in 0..MAX_RETRIES {
        let adjusted_target = if attempt == 0 {
            target_bytes
        } else {
            let current_size = std::fs::metadata(&output)
                .map(|m| m.len())
                .unwrap_or(target_bytes);
            if current_size <= target_bytes {
                return Ok(format!(
                    "Comprimido: {} ({:.1} MB)\nOriginal: {} MB",
                    output,
                    current_size as f64 / (1024.0 * 1024.0),
                    std::fs::metadata(&input)
                        .map(|m| m.len() as f64 / (1024.0 * 1024.0))
                        .unwrap_or(0.0)
                ));
            }
            (current_size as f64 * 0.85) as u64
        };

        let usable_bits = (adjusted_target as f64 * 8.0 * OVERHEAD_FACTOR) as u64;
        let audio_bits = (AUDIO_BITRATE_BPS as f64 * duration) as u64;
        let video_bits = usable_bits.saturating_sub(audio_bits);
        let video_bitrate_kbps = (video_bits / duration as u64) / 1000;
        let audio_bitrate_kbps = AUDIO_BITRATE_BPS / 1000;

        if video_bitrate_kbps < 50 {
            return Err(format!(
                "El video es demasiado grande para {} MB (duración: {:.0}s)",
                DISCORD_LIMIT_MB, duration
            ));
        }

        eprintln!(
            "[NEEKO] Intento {}: video={}kbps audio={}kbps target={:.1}MB",
            attempt + 1,
            video_bitrate_kbps,
            audio_bitrate_kbps,
            adjusted_target as f64 / (1024.0 * 1024.0)
        );

        compress_once(
            &input,
            &output,
            video_bitrate_kbps,
            audio_bitrate_kbps,
            &ffmpeg,
        )?;
    }

    let final_size = std::fs::metadata(&output).map(|m| m.len()).unwrap_or(0);

    Ok(format!(
        "Comprimido: {} ({:.1} MB)\nOriginal: {} MB",
        output,
        final_size as f64 / (1024.0 * 1024.0),
        std::fs::metadata(&input)
            .map(|m| m.len() as f64 / (1024.0 * 1024.0))
            .unwrap_or(0.0)
    ))
}

#[tauri::command]
pub async fn compress_for_discord(app: AppHandle, input: String) -> Result<String, String> {
    let result = tokio::task::spawn_blocking(move || compress_sync(input))
        .await
        .map_err(|e| format!("Error en compresion: {}", e))?;

    match &result {
        Ok(msg) => notify_system(&app, "Video comprimido", msg),
        Err(err) => notify_system(&app, "No pude comprimir el video", err),
    }

    result
}
