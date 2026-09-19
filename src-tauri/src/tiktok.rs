#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::{
    path::PathBuf,
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

static BUSY: AtomicBool = AtomicBool::new(false);
static LAST_OUTPUT: Mutex<Option<(PathBuf, PathBuf)>> = Mutex::new(None);
struct BusyGuard;
impl Drop for BusyGuard {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::Release);
    }
}

#[tauri::command]
pub fn open_tiktok_window(app: AppHandle) -> Result<String, String> {
    if let Some(window) = app.get_webview_window("tiktok") {
        window.show().map_err(|e| e.to_string())?;
        window.unminimize().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
    } else {
        WebviewWindowBuilder::new(&app, "tiktok", WebviewUrl::App("tiktok.html".into()))
            .title("Preparar para TikTok")
            .inner_size(800.0, 700.0)
            .min_inner_size(520.0, 430.0)
            .decorations(false)
            .build()
            .map_err(|e| e.to_string())?;
    }
    Ok("Abrí Preparar para TikTok".into())
}

#[tauri::command]
pub fn tiktok_engines() -> Result<serde_json::Value, String> {
    serde_json::from_str(include_str!("../scripts/tiktok-engines.json")).map_err(|e| e.to_string())
}

fn process(
    app: AppHandle,
    input: String,
    engine: String,
    options: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let python = crate::find_system_python()
        .ok_or("Necesitás Python 3.10 o superior instalado para ejecutar los motores TikTok.")?;
    let root = if cfg!(debug_assertions) {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    } else {
        app.path().resource_dir().map_err(|e| e.to_string())?
    };
    let config = crate::config::AppConfig::load();
    let mut cmd = Command::new(python);
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);
    let output = cmd
        .args(["-I", "-B", "-X", "utf8"])
        .arg(root.join("scripts/tiktok_runner.py"))
        .arg(root.join("vendor/tiktok-quality/src"))
        .arg(input.trim().trim_matches('"'))
        .arg(engine)
        .arg(options.to_string())
        .arg(config.ffmpeg_path.trim())
        .arg(config.ffprobe_path.trim())
        .env("PYTHONIOENCODING", "utf-8")
        .output()
        .map_err(|e| format!("No pude iniciar el motor TikTok: {e}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if detail.is_empty() {
            format!(
                "El motor TikTok terminó sin completar el video ({}).",
                output.status
            )
        } else {
            detail
        });
    }
    let result: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Respuesta inválida del motor TikTok: {e}"))?;
    let path = result["path"]
        .as_str()
        .ok_or("El motor no devolvió el archivo de salida")?;
    let source = result["source"]
        .as_str()
        .ok_or("El motor no devolvió el archivo original")?;
    *LAST_OUTPUT.lock().map_err(|e| e.to_string())? =
        Some((PathBuf::from(path), PathBuf::from(source)));
    Ok(result)
}

#[tauri::command]
pub async fn prepare_tiktok(
    app: AppHandle,
    input: String,
    engine: Option<String>,
    options: Option<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    BUSY.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| "Ya hay un video preparándose para TikTok")?;
    let guard = BusyGuard;
    tokio::task::spawn_blocking(move || {
        let _guard = guard;
        process(
            app,
            input,
            engine.unwrap_or_else(|| "bastien".into()),
            options.unwrap_or_else(|| serde_json::json!({})),
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn open_tiktok_output_folder() -> Result<(), String> {
    let output = LAST_OUTPUT.lock().map_err(|e| e.to_string())?;
    let folder = output
        .as_ref()
        .and_then(|(p, _)| p.parent())
        .ok_or("Todavía no hay un video preparado")?;
    open::that(folder).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_tiktok_original() -> Result<(), String> {
    let result = LAST_OUTPUT.lock().map_err(|e| e.to_string())?;
    let (_, source) = result.as_ref().ok_or("Todavía no hay un video preparado")?;
    open::that(source).map_err(|e| e.to_string())
}
