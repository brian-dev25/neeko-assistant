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
const MAX_RETRIES: u32 = 5;
const MIN_TARGET_FILL_RATIO: f64 = 0.80;
const MAX_INTERNAL_TARGET_MULTIPLIER: f64 = 2.0;
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Clone, Copy)]
struct CompressionOptions {
    target_size_mb: Option<u64>,
    video_bitrate_kbps: Option<u64>,
}

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
        .map_err(|_| format!("No pude leer la duracion: {}", stdout))?;

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
            "FFmpeg pass 1 fallo: {}",
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
            "FFmpeg pass 2 fallo: {}",
            stderr.chars().take(200).collect::<String>()
        ));
    }

    Ok(())
}

fn format_mb(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

fn output_suffix(options: CompressionOptions) -> String {
    match (options.target_size_mb, options.video_bitrate_kbps) {
        (Some(target), Some(bitrate)) => format!("_{}mb_{}kbps", target, bitrate),
        (Some(target), None) => format!("_{}mb", target),
        (None, Some(bitrate)) => format!("_{}kbps", bitrate),
        (None, None) => "_discord".to_string(),
    }
}

fn calculate_video_bitrate_kbps(target_bytes: u64, duration: f64, audio_bitrate_bps: u64) -> u64 {
    let usable_bits = target_bytes as f64 * 8.0 * OVERHEAD_FACTOR;
    let audio_bits = audio_bitrate_bps as f64 * duration;
    let video_bits = (usable_bits - audio_bits).max(0.0);
    (video_bits / duration / 1000.0).floor() as u64
}

fn success_message(input: &str, output: &str, target_bytes: Option<u64>) -> String {
    let final_size = std::fs::metadata(output).map(|m| m.len()).unwrap_or(0);
    let original_size = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
    let target_text = target_bytes
        .map(|bytes| format!("\nObjetivo maximo: {:.1} MB", format_mb(bytes)))
        .unwrap_or_default();

    format!(
        "Comprimido: {} ({:.1} MB)\nOriginal: {:.1} MB{}",
        output,
        format_mb(final_size),
        format_mb(original_size),
        target_text
    )
}

fn compress_sync(input: String, options: CompressionOptions) -> Result<String, String> {
    let ffmpeg = find_ffmpeg()?;
    let ffprobe = find_ffprobe()?;

    let input_path = PathBuf::from(&input);
    if !input_path.exists() {
        return Err(format!("No encontre el archivo: {}", input));
    }

    let target_size_mb = options.target_size_mb.unwrap_or(DISCORD_LIMIT_MB);
    if !(1..=65_536).contains(&target_size_mb) {
        return Err("El tamano maximo tiene que estar entre 1 y 65536 MB".to_string());
    }
    if let Some(bitrate) = options.video_bitrate_kbps {
        if !(1..=100_000).contains(&bitrate) {
            return Err("El bitrate de video tiene que estar entre 1 y 100000 kbps".to_string());
        }
    }

    let duration = get_duration_secs(&input, &ffprobe)?;
    if duration <= 0.0 {
        return Err("No pude obtener la duracion del video".to_string());
    }

    let output_path = input_path.with_file_name(format!(
        "{}{}.mp4",
        input_path.file_stem().unwrap_or_default().to_string_lossy(),
        output_suffix(options)
    ));
    let output = output_path.to_string_lossy().to_string();

    if options.target_size_mb.is_none() && options.video_bitrate_kbps.is_some() {
        let video_bitrate_kbps = options.video_bitrate_kbps.unwrap();
        let audio_bitrate_kbps = AUDIO_BITRATE_BPS / 1000;
        eprintln!(
            "[NEEKO] Comprimiendo por bitrate: {} -> {} (video={}kbps audio={}kbps)",
            input, output, video_bitrate_kbps, audio_bitrate_kbps
        );
        compress_once(
            &input,
            &output,
            video_bitrate_kbps,
            audio_bitrate_kbps,
            &ffmpeg,
        )?;
        return Ok(success_message(&input, &output, None));
    }

    let target_bytes = target_size_mb * 1024 * 1024;
    let min_preferred_bytes = (target_bytes as f64 * MIN_TARGET_FILL_RATIO) as u64;
    let mut adjusted_target = target_bytes;
    let mut last_size = 0_u64;

    eprintln!(
        "[NEEKO] Comprimiendo: {} -> {} (duracion: {:.1}s, max: {} MB)",
        input, output, duration, target_size_mb
    );

    for attempt in 0..MAX_RETRIES {
        let calculated_bitrate =
            calculate_video_bitrate_kbps(adjusted_target, duration, AUDIO_BITRATE_BPS);
        let video_bitrate_kbps = options
            .video_bitrate_kbps
            .map(|cap| cap.min(calculated_bitrate))
            .unwrap_or(calculated_bitrate);
        let audio_bitrate_kbps = AUDIO_BITRATE_BPS / 1000;

        if video_bitrate_kbps < 50 {
            return Err(format!(
                "El video es demasiado largo para {} MB (duracion: {:.0}s)",
                target_size_mb, duration
            ));
        }

        eprintln!(
            "[NEEKO] Intento {}: video={}kbps audio={}kbps target={:.1}MB",
            attempt + 1,
            video_bitrate_kbps,
            audio_bitrate_kbps,
            format_mb(adjusted_target)
        );

        compress_once(
            &input,
            &output,
            video_bitrate_kbps,
            audio_bitrate_kbps,
            &ffmpeg,
        )?;

        last_size = std::fs::metadata(&output).map(|m| m.len()).unwrap_or(0);
        if last_size == 0 {
            return Err("FFmpeg genero un archivo vacio".to_string());
        }

        if last_size <= target_bytes
            && (last_size >= min_preferred_bytes || options.video_bitrate_kbps.is_some())
        {
            return Ok(success_message(&input, &output, Some(target_bytes)));
        }

        if last_size > target_bytes {
            adjusted_target =
                ((adjusted_target as f64) * (target_bytes as f64 / last_size as f64) * 0.96) as u64;
        } else if last_size < min_preferred_bytes {
            adjusted_target = ((adjusted_target as f64)
                * (min_preferred_bytes as f64 / last_size as f64)
                * 1.03) as u64;
            adjusted_target =
                adjusted_target.min((target_bytes as f64 * MAX_INTERNAL_TARGET_MULTIPLIER) as u64);
        }
    }

    if last_size > target_bytes {
        return Err(format!(
            "No pude dejarlo por debajo de {} MB. Ultimo intento: {:.1} MB",
            target_size_mb,
            format_mb(last_size)
        ));
    }

    Ok(success_message(&input, &output, Some(target_bytes)))
}

#[tauri::command]
pub async fn compress_for_discord(
    app: AppHandle,
    input: String,
    target_size_mb: Option<u64>,
    video_bitrate_kbps: Option<u64>,
) -> Result<String, String> {
    let options = CompressionOptions {
        target_size_mb,
        video_bitrate_kbps,
    };
    let result = tokio::task::spawn_blocking(move || compress_sync(input, options))
        .await
        .map_err(|e| format!("Error en compresion: {}", e))?;

    match &result {
        Ok(msg) => notify_system(&app, "Video comprimido", msg),
        Err(err) => notify_system(&app, "No pude comprimir el video", err),
    }

    result
}
