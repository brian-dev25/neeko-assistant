use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager,
};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_updater::UpdaterExt;
use tokio::sync::{broadcast, watch};

mod config;
mod git_commands;
mod lol_api;
mod video_compress;
mod web_server;

const LLAMA_SERVER_URL: &str = "http://127.0.0.1:8080";

static LLAMA_PROCESS: OnceLock<Mutex<Option<Child>>> = OnceLock::new();
static EXIT_REQUESTED: AtomicBool = AtomicBool::new(false);
static DOWNLOAD_PROGRESS: OnceLock<broadcast::Sender<DownloadProgress>> = OnceLock::new();
static CANCEL_DOWNLOADS: OnceLock<Mutex<HashMap<String, watch::Sender<bool>>>> = OnceLock::new();

pub fn llama_process() -> &'static Mutex<Option<Child>> {
    LLAMA_PROCESS.get_or_init(|| Mutex::new(None))
}

pub(crate) fn is_llama_server_running() -> bool {
    let Ok(mut process) = llama_process().lock() else {
        return false;
    };

    let Some(child) = process.as_mut() else {
        return false;
    };

    match child.try_wait() {
        Ok(Some(_)) => {
            *process = None;
            false
        }
        Ok(None) => true,
        Err(_) => {
            *process = None;
            false
        }
    }
}

fn stop_tracked_llama_process() {
    if let Ok(mut process) = llama_process().lock() {
        if let Some(mut child) = process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

pub(crate) fn notify_system(app: &AppHandle, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
}

#[cfg(windows)]
fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
unsafe fn check_single_instance_windows() {
    use windows_sys::Win32::Foundation::*;
    use windows_sys::Win32::System::Threading::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    let name = to_wide("Local\\NeekoAssistantSingleInstance");
    let handle = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
    if handle.is_null() {
        eprintln!("[NEEKO] CreateMutexW failed; allowing multiple instances.");
        return;
    }

    if GetLastError() == ERROR_ALREADY_EXISTS {
        CloseHandle(handle);
        let msg = to_wide("Neeko Assistant ya esta abierto.\n\nUsa la ventana existente o abrila desde la bandeja.");
        let title = to_wide("Neeko Assistant");
        MessageBoxW(std::ptr::null_mut(), msg.as_ptr(), title.as_ptr(), 0x50010);
        std::process::exit(1);
    }
}

fn cleanup_before_exit() {
    if let Ok(mut process) = llama_process().lock() {
        if let Some(mut child) = process.take() {
            eprintln!("[NEEKO] Cerrando llama-server...");
            match child.kill() {
                Ok(_) => {
                    let _ = child.wait();
                    eprintln!("[NEEKO] llama-server cerrado");
                }
                Err(e) => {
                    eprintln!("[NEEKO] No pude cerrar llama-server: {}", e);
                }
            }
        }
    }

    let temp_dir = std::env::temp_dir().join("neeko-files");
    let _ = std::fs::remove_dir_all(&temp_dir);
    eprintln!("[NEEKO] Cleaned up temp files on exit");
}

#[tauri::command]
fn llama_status() -> Result<bool, String> {
    if get_model_path().is_empty() {
        stop_tracked_llama_process();
        return Ok(false);
    }

    Ok(is_llama_server_running())
}

#[tauri::command]
fn get_llama_auto_start() -> Result<bool, String> {
    let config = config::AppConfig::load();
    Ok(config.llama_auto_start)
}

#[tauri::command]
fn set_llama_auto_start(enabled: bool) -> Result<String, String> {
    let mut config = config::AppConfig::load();
    config.llama_auto_start = enabled;
    config.save().map_err(|e| e.to_string())?;
    Ok(if enabled {
        "Auto-start activado"
    } else {
        "Auto-start desactivado"
    }
    .to_string())
}

#[tauri::command]
fn get_system_commands_enabled() -> Result<bool, String> {
    let config = config::AppConfig::load();
    Ok(config.system_commands_enabled)
}

#[tauri::command]
fn set_system_commands_enabled(enabled: bool) -> Result<String, String> {
    let mut config = config::AppConfig::load();
    config.system_commands_enabled = enabled;
    config.save().map_err(|e| e.to_string())?;
    Ok(if enabled {
        "Comandos de sistema activados"
    } else {
        "Comandos de sistema desactivados"
    }
    .to_string())
}

#[tauri::command]
async fn start_llama_server() -> Result<String, String> {
    let model_path = get_model_path();
    if model_path.is_empty() {
        stop_tracked_llama_process();
        return Err("No encontre el modelo GGUF".to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .map_err(|e| e.to_string())?;

    // Already running?
    if let Ok(resp) = client
        .get(format!("{}/health", LLAMA_SERVER_URL))
        .send()
        .await
    {
        if resp.status().is_success() {
            return Ok("LLaMA ya está corriendo 🦎".to_string());
        }
    }

    if model_path.is_empty() {
        return Err("No encontré el modelo GGUF".to_string());
    }

    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));

    let candidates_exe = [
        exe_dir.join("binaries").join("llama-server-x86_64-pc-windows-msvc.exe"),
        exe_dir.join("..\\..\\binaries").join("llama-server-x86_64-pc-windows-msvc.exe"),
        PathBuf::from("D:\\NEEKO API\\neeko-assistant\\src-tauri\\binaries\\llama-server-x86_64-pc-windows-msvc.exe"),
    ];

    let sidecar_exe = candidates_exe
        .iter()
        .find(|p| p.exists())
        .cloned()
        .ok_or_else(|| "No encontré el sidecar llama-server.exe".to_string())?;

    let binaries_dir = sidecar_exe.parent().unwrap().to_path_buf();

    let mut command = Command::new(&sidecar_exe);
    command.current_dir(&binaries_dir).args([
        "-m",
        &model_path,
        "--host",
        "127.0.0.1",
        "--port",
        "8080",
        "-ngl",
        "15",
        "-c",
        "1024",
        "-t",
        "4",
    ]);

    #[cfg(target_os = "windows")]
    command.creation_flags(0x08000000);

    let child = command.spawn().map_err(|e| format!("Error: {}", e))?;
    eprintln!("[NEEKO] llama-server spawned OK");
    if let Ok(mut process) = llama_process().lock() {
        *process = Some(child);
    }

    // Wait for ready
    for _ in 0..30 {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        if let Ok(resp) = client
            .get(format!("{}/health", LLAMA_SERVER_URL))
            .send()
            .await
        {
            if resp.status().is_success() {
                return Ok("LLaMA iniciado 🦎".to_string());
            }
        }
    }

    Err("LLaMA no arrancó a tiempo".to_string())
}

#[tauri::command]
async fn stop_llama_server() -> Result<String, String> {
    let mut process = llama_process().lock().unwrap();
    if let Some(mut child) = process.take() {
        eprintln!("[NEEKO] Cerrando llama-server...");
        match child.kill() {
            Ok(_) => {
                let _ = child.wait();
                eprintln!("[NEEKO] llama-server cerrado");
                Ok("LLaMA cerrado 🦎".to_string())
            }
            Err(e) => {
                eprintln!("[NEEKO] No pude cerrar llama-server: {}", e);
                Err(format!("No pude cerrar LLaMA: {}", e))
            }
        }
    } else {
        Ok("LLaMA ya estaba apagado".to_string())
    }
}

pub(crate) fn get_model_path() -> String {
    // Buscar el modelo GGUF en la carpeta IA relativa al ejecutable
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));

    let app_data_model_dir = app_data_dir().join("IA");

    // Buscar en distintas ubicaciones posibles
    let candidates = [
        app_data_model_dir.join("neeko-qwen3-4b-Q4_K_M.gguf"),
        exe_dir.join("IA").join("neeko-qwen3-4b-Q4_K_M.gguf"),
        exe_dir.join("neeko-qwen3-4b-Q4_K_M.gguf"),
        #[cfg(debug_assertions)]
        PathBuf::from("D:\\NEEKO API\\neeko-assistant\\IA\\neeko-qwen3-4b-Q4_K_M.gguf"),
    ];

    for path in &candidates {
        if path.exists() {
            return path.to_string_lossy().to_string();
        }
    }

    // Fallback: buscar cualquier .gguf en la carpeta IA
    for ia_dir in [app_data_model_dir, exe_dir.join("IA")] {
        if let Ok(entries) = std::fs::read_dir(&ia_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("gguf") {
                    return path.to_string_lossy().to_string();
                }
            }
        }
    }

    "".to_string()
}

pub(crate) fn app_data_dir() -> PathBuf {
    dirs::data_dir()
        .or_else(|| dirs::config_dir())
        .or_else(|| dirs::home_dir())
        .unwrap_or_else(|| PathBuf::from("."))
        .join("neeko-assistant")
}

pub(crate) fn installed_models_dir() -> PathBuf {
    app_data_dir().join("IA")
}

pub(crate) fn sanitize_model_file_name(file_name: &str) -> Result<String, String> {
    let name = Path::new(file_name)
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .ok_or_else(|| "No pude leer el nombre del modelo".to_string())?;

    if !name.to_lowercase().ends_with(".gguf") {
        return Err("El archivo del modelo tiene que ser .gguf".to_string());
    }

    Ok(name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            _ => c,
        })
        .collect())
}

#[tauri::command]
fn minimize_window(window: tauri::Window) {
    let _ = window.minimize();
}

#[tauri::command]
fn close_window(window: tauri::Window) {
    let _ = window.hide();
}

#[tauri::command]
fn open_app(app: String) -> Result<String, String> {
    let app_lower = app.to_lowercase();
    let result = match app_lower.as_str() {
        "spotify" => {
            if let Some(path) = find_app_in_start_menu("spotify") {
                let resolved = resolve_unc_path(&path);
                let path_buf = std::path::PathBuf::from(&resolved);
                let parent_dir = path_buf.parent().unwrap_or(std::path::Path::new("C:\\"));
                Command::new(&resolved)
                    .current_dir(parent_dir)
                    .spawn()
                    .map(|_| ())
            } else {
                open::that("spotify")
            }
        }
        "discord" => {
            if let Some(path) = find_app_in_start_menu("discord") {
                let resolved = resolve_unc_path(&path);
                let path_buf = std::path::PathBuf::from(&resolved);
                let parent_dir = path_buf.parent().unwrap_or(std::path::Path::new("C:\\"));
                Command::new(&resolved)
                    .current_dir(parent_dir)
                    .spawn()
                    .map(|_| ())
            } else {
                open::that("discord")
            }
        }
        "chrome" | "navegador" | "internet" => open::that("chrome"),
        "firefox" => open::that("firefox"),
        "edge" => open::that("microsoft-edge:"),
        "notepad" | "bloc de notas" => Command::new("notepad").output().map(|_| ()),
        "calculator" | "calculadora" => Command::new("calc").output().map(|_| ()),
        "explorer" | "archivos" | "carpeta" => Command::new("explorer").output().map(|_| ()),
        "terminal" | "powershell" | "consola" => Command::new("powershell").output().map(|_| ()),
        "vscode" | "code" => open::that("vscode"),
        "youtube" => open::that("https://www.youtube.com"),
        _ => open::that(&app),
    };

    match result {
        Ok(_) => Ok(format!("Abrí {}", app)),
        Err(e) => Err(format!("No pude abrir {}: {}", app, e)),
    }
}

#[tauri::command]
fn open_url(url: String) -> Result<String, String> {
    let full_url = if url.starts_with("http") {
        url
    } else {
        format!("https://{}", url)
    };

    match open::that(&full_url) {
        Ok(_) => Ok(format!("Abrí {}", full_url)),
        Err(e) => Err(format!("No pude abrir la URL: {}", e)),
    }
}

#[tauri::command]
fn search_web(query: String) -> Result<String, String> {
    let url = format!(
        "https://www.google.com/search?q={}",
        query.replace(" ", "+")
    );
    match open::that(&url) {
        Ok(_) => Ok(format!("Busqué: {}", query)),
        Err(e) => Err(format!("No pude buscar: {}", e)),
    }
}

#[tauri::command]
fn open_folder(folder: String) -> Result<String, String> {
    let path = match folder.to_lowercase().as_str() {
        "desktop" | "escritorio" => dirs::desktop_dir(),
        "downloads" | "descargas" => dirs::download_dir(),
        "documents" | "documentos" => dirs::document_dir(),
        "home" | "inicio" => dirs::home_dir(),
        _ => None,
    };

    if let Some(p) = path {
        match Command::new("explorer").arg(p).output() {
            Ok(_) => Ok(format!("Abrí la carpeta {}", folder)),
            Err(e) => Err(format!("No pude abrir la carpeta: {}", e)),
        }
    } else {
        Err(format!("No conozco la carpeta: {}", folder))
    }
}

#[tauri::command]
async fn check_local_ai() -> Result<String, String> {
    let model_path = get_model_path();
    if model_path.is_empty() {
        return Err("no_model".to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .map_err(|e| e.to_string())?;

    // Ya corriendo?
    if let Ok(resp) = client
        .get(format!("{}/health", LLAMA_SERVER_URL))
        .send()
        .await
    {
        if resp.status().is_success() {
            return Ok("running".to_string());
        }
    }

    Err("not_running".to_string())
}

#[tauri::command]
async fn list_models() -> Result<Vec<String>, String> {
    let model_path = get_model_path();
    if model_path.is_empty() {
        return Ok(vec![]);
    }
    let name = Path::new(&model_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();
    Ok(vec![name])
}

#[tauri::command]
fn get_model_path_cmd() -> Result<String, String> {
    let path = get_model_path();
    if path.is_empty() {
        Err("no_model".to_string())
    } else {
        Ok(path)
    }
}

#[derive(Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Clone)]
struct ChatRequest {
    result: Arc<tokio::sync::Mutex<Option<Result<String, String>>>>,
    done: Arc<tokio::sync::watch::Sender<bool>>,
}

static CHAT_REQUESTS: OnceLock<Mutex<HashMap<String, ChatRequest>>> = OnceLock::new();

fn chat_requests() -> &'static Mutex<HashMap<String, ChatRequest>> {
    CHAT_REQUESTS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[tauri::command]
async fn chat_start(messages: Vec<ChatMessage>) -> Result<String, String> {
    let request_id = uuid_simple();
    let (done_tx, _done_rx) = tokio::sync::watch::channel(false);
    let result: Arc<tokio::sync::Mutex<Option<Result<String, String>>>> =
        Arc::new(tokio::sync::Mutex::new(None));

    let result_clone = result.clone();
    let done_tx_clone = done_tx.clone();
    let id_clone = request_id.clone();

    tokio::spawn(async move {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .unwrap();

        let body = serde_json::json!({
            "model": "neeko",
            "messages": messages,
            "stream": false
        });

        let resp = client
            .post(format!("{}/v1/chat/completions", LLAMA_SERVER_URL))
            .json(&body)
            .send()
            .await;

        let res = match resp {
            Ok(r) if r.status().is_success() => {
                let data: serde_json::Value = r.json().await.unwrap_or_default();
                Ok(data["choices"][0]["message"]["content"]
                    .as_str()
                    .unwrap_or("No entendí 🥺")
                    .to_string())
            }
            Ok(r) => {
                let status = r.status();
                let text = r.text().await.unwrap_or_default();
                Err(format!("llama-server respondió {}: {}", status, text))
            }
            Err(e) => Err(format!("No se pudo conectar a llama-server: {}", e)),
        };

        *result_clone.lock().await = Some(res);
        let _ = done_tx_clone.send(true);
        eprintln!("[NEEKO] chat response ready for {}", id_clone);
    });

    chat_requests().lock().unwrap().insert(
        request_id.clone(),
        ChatRequest {
            result,
            done: Arc::new(done_tx),
        },
    );

    Ok(request_id)
}

#[tauri::command]
async fn chat_cancel(request_id: String) -> Result<String, String> {
    let chat_requests = chat_requests();
    let mut requests = chat_requests.lock().unwrap();
    if let Some(req) = requests.remove(&request_id) {
        let _ = req.done.send(true);
        Ok("cancelado".to_string())
    } else {
        Err("request no encontrada".to_string())
    }
}

#[tauri::command]
async fn chat_finish(request_id: String) -> Result<String, String> {
    let chat_req = {
        let requests = chat_requests().lock().unwrap();
        requests.get(&request_id).cloned()
    };

    let req = match chat_req {
        Some(r) => r,
        None => return Err("cancelado".to_string()),
    };

    let mut done_rx = req.done.subscribe();

    let result = tokio::select! {
        _ = done_rx.changed() => {
            let lock = req.result.lock().await;
            lock.clone().unwrap_or(Err("cancelado".to_string()))
        }
        _ = tokio::time::sleep(std::time::Duration::from_secs(130)) => {
            Err("timeout".to_string())
        }
    };

    chat_requests().lock().unwrap().remove(&request_id);
    result
}

fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    format!("{:x}-{:x}", t.as_secs(), t.subsec_nanos())
}

#[derive(Serialize, Clone)]
pub(crate) struct DownloadProgress {
    pub id: String,
    pub label: String,
    pub downloaded: u64,
    pub total: Option<u64>,
    pub percent: Option<u64>,
    pub message: String,
}

pub(crate) fn download_progress_sender() -> broadcast::Sender<DownloadProgress> {
    DOWNLOAD_PROGRESS
        .get_or_init(|| broadcast::channel(100).0)
        .clone()
}

pub(crate) fn subscribe_download_progress() -> broadcast::Receiver<DownloadProgress> {
    download_progress_sender().subscribe()
}

fn emit_download_progress(
    app: &AppHandle,
    id: &str,
    label: &str,
    downloaded: u64,
    total: Option<u64>,
    message: &str,
) {
    let percent = total.and_then(|t| {
        if t == 0 {
            None
        } else {
            Some(((downloaded as f64 / t as f64) * 100.0).round() as u64)
        }
    });

    let payload = DownloadProgress {
        id: id.to_string(),
        label: label.to_string(),
        downloaded,
        total,
        percent,
        message: message.to_string(),
    };

    let _ = app.emit("dependency-download-progress", payload.clone());
    let _ = download_progress_sender().send(payload);
}

fn register_download_cancel(id: &str) -> watch::Receiver<bool> {
    let (cancel_tx, cancel_rx) = watch::channel(false);
    CANCEL_DOWNLOADS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap()
        .insert(id.to_string(), cancel_tx);
    cancel_rx
}

fn unregister_download_cancel(id: &str) {
    CANCEL_DOWNLOADS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap()
        .remove(id);
}

async fn write_response_to_file(
    app: &AppHandle,
    id: &str,
    label: &str,
    mut response: reqwest::Response,
    destination: &Path,
    mut cancel_rx: watch::Receiver<bool>,
) -> Result<(), String> {
    if let Some(parent) = destination.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| e.to_string())?;
    }

    if !response.status().is_success() {
        unregister_download_cancel(id);
        return Err(format!("La descarga respondio HTTP {}", response.status()));
    }

    let total = response.content_length();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();

    if destination
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("gguf"))
        .unwrap_or(false)
        && content_type.contains("text/html")
    {
        unregister_download_cancel(id);
        return Err("El link del modelo devolvio una pagina web, no un archivo GGUF directo. Usa 'Instalar modelo desde archivo' o pega un enlace de descarga directa.".to_string());
    }

    let mut file = tokio::fs::File::create(destination).await.map_err(|e| {
        unregister_download_cancel(id);
        e.to_string()
    })?;
    let mut downloaded = 0_u64;

    use tokio::io::AsyncWriteExt;
    while let Some(chunk) = response.chunk().await.map_err(|e| {
        let _ = std::fs::remove_file(destination);
        unregister_download_cancel(id);
        e.to_string()
    })? {
        if *cancel_rx.borrow_and_update() {
            let _ = tokio::fs::remove_file(destination).await;
            unregister_download_cancel(id);
            return Err("Descarga cancelada".to_string());
        }
        file.write_all(&chunk).await.map_err(|e| {
            let _ = std::fs::remove_file(destination);
            unregister_download_cancel(id);
            e.to_string()
        })?;
        downloaded += chunk.len() as u64;
        emit_download_progress(app, id, label, downloaded, total, "Descargando...");
    }

    file.flush().await.map_err(|e| e.to_string())?;
    unregister_download_cancel(id);
    emit_download_progress(app, id, label, downloaded, total, "Descarga completa");
    Ok(())
}

async fn download_to_file(
    app: &AppHandle,
    id: &str,
    label: &str,
    url: &str,
    destination: &Path,
) -> Result<(), String> {
    let cancel_rx = register_download_cancel(id);
    emit_download_progress(app, id, label, 0, None, "Conectando...");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60 * 60))
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .get(url)
        .header(reqwest::header::USER_AGENT, "Neeko Assistant")
        .send()
        .await
        .map_err(|e| {
            unregister_download_cancel(id);
            format!("No pude iniciar la descarga: {}", e)
        })?;

    write_response_to_file(app, id, label, response, destination, cancel_rx).await
}

fn google_drive_file_id(url: &str) -> Result<Option<String>, String> {
    if !url.contains("drive.google.com") && !url.contains("docs.google.com") {
        return Ok(None);
    }

    let patterns = [r"drive\.google\.com/file/d/([^/?]+)", r"[?&]id=([^&]+)"];
    for pattern in patterns {
        let re = regex::Regex::new(pattern).map_err(|e| e.to_string())?;
        if let Some(file_id) = re.captures(url).and_then(|c| c.get(1)) {
            return Ok(Some(file_id.as_str().to_string()));
        }
    }

    Err("No pude extraer el ID del archivo de Google Drive".to_string())
}

async fn download_google_drive_file(
    app: &AppHandle,
    file_id: &str,
    destination: &Path,
) -> Result<(), String> {
    let cancel_rx = register_download_cancel("model");
    emit_download_progress(
        app,
        "model",
        "Modelo IA",
        0,
        None,
        "Conectando con Google Drive...",
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60 * 60))
        .cookie_store(true)
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| {
            unregister_download_cancel("model");
            e.to_string()
        })?;

    let direct_url = format!("https://drive.google.com/uc?export=download&id={}", file_id);

    let resp1 = client
        .get(&direct_url)
        .header(reqwest::header::USER_AGENT, "Neeko Assistant")
        .send()
        .await
        .map_err(|e| {
            unregister_download_cancel("model");
            format!("No pude abrir Google Drive: {}", e)
        })?;

    let ct1 = resp1
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();

    if !ct1.contains("text/html") {
        return write_response_to_file(app, "model", "Modelo IA", resp1, destination, cancel_rx)
            .await;
    }

    let html = resp1.text().await.map_err(|e| {
        unregister_download_cancel("model");
        format!("No pude leer la pagina de Google Drive: {}", e)
    })?;

    let uuid_re =
        regex::Regex::new(r#"name="uuid"\s+value="([^"]+)""#).map_err(|e| e.to_string())?;
    let uuid = uuid_re
        .captures(&html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str())
        .unwrap_or("");

    let confirm_url = if !uuid.is_empty() {
        format!(
            "https://drive.google.com/uc?export=download&id={}&confirm=t&uuid={}",
            file_id, uuid
        )
    } else {
        format!(
            "https://drive.google.com/uc?export=download&id={}&confirm=t",
            file_id
        )
    };

    emit_download_progress(
        app,
        "model",
        "Modelo IA",
        0,
        None,
        "Confirmando descarga...",
    );

    let resp2 = client
        .get(&confirm_url)
        .header(reqwest::header::USER_AGENT, "Neeko Assistant")
        .send()
        .await
        .map_err(|e| {
            unregister_download_cancel("model");
            format!("No pude confirmar la descarga: {}", e)
        })?;

    let ct2 = resp2
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();

    if ct2.contains("text/html") {
        unregister_download_cancel("model");
        return Err(
            "Google Drive devolvio una pagina HTML en vez del archivo. Intenta de nuevo o usa 'Instalar desde archivo'."
                .to_string(),
        );
    }

    write_response_to_file(app, "model", "Modelo IA", resp2, destination, cancel_rx).await
}

#[tauri::command]
fn cancel_download(id: String) -> Result<String, String> {
    let map = CANCEL_DOWNLOADS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap();
    if let Some(tx) = map.get(&id) {
        let _ = tx.send(true);
        Ok(format!("Cancelando descarga {}...", id))
    } else {
        Err(format!("No hay descarga activa con id {}", id))
    }
}

async fn resolve_download_url(url: &str) -> Result<String, String> {
    if url.contains("drive.google.com/file/d/") {
        let re =
            regex::Regex::new(r"drive\.google\.com/file/d/([^/]+)").map_err(|e| e.to_string())?;
        let file_id = re
            .captures(url)
            .and_then(|c| c.get(1))
            .ok_or_else(|| "No pude extraer el ID del archivo de Google Drive".to_string())?;
        let fid = file_id.as_str();

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| e.to_string())?;

        let direct_url = format!(
            "https://drive.google.com/uc?export=download&id={}",
            fid
        );

        let response = client
            .get(&direct_url)
            .header(reqwest::header::USER_AGENT, "Neeko Assistant")
            .send()
            .await
            .map_err(|e| format!("No pude abrir Google Drive: {}", e))?;

        let status = response.status();
        if status.is_redirection() || status.is_success() {
            let ct = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_lowercase();
            if !ct.contains("text/html") {
                let location = response
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|v| v.to_str().ok());
                if let Some(loc) = location {
                    return Ok(loc.to_string());
                }
                return Ok(direct_url);
            }
        }

        let html = response.text().await.unwrap_or_default();

        let action_re = regex::Regex::new(
            r#"action="(https?://[^"]*download[^"]*\?[^"]*)""#,
        )
        .map_err(|e| e.to_string())?;
        if let Some(caps) = action_re.captures(&html) {
            let action_url = caps.get(1).unwrap().as_str().replace("&amp;", "&");
            return Ok(action_url);
        }

        let uuid_re = regex::Regex::new(r#"uuid=([^&"]+)"#).map_err(|e| e.to_string())?;
        let uuid = uuid_re
            .captures(&html)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str())
            .unwrap_or("");

        let confirm_re = regex::Regex::new(r#""confirm"\s*:\s*"([^"]+)""#)
            .map_err(|e| e.to_string())?;
        let confirm = confirm_re
            .captures(&html)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str())
            .unwrap_or("t");

        if !uuid.is_empty() {
            return Ok(format!(
                "https://drive.usercontent.google.com/download?id={}&export=download&confirm={}&uuid={}",
                fid, confirm, uuid
            ));
        }

        return Ok(format!(
            "https://drive.google.com/uc?export=download&id={}&confirm={}",
            fid, confirm
        ));
    }

    if !url.contains("mediafire.com/file/") {
        return Ok(url.to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| e.to_string())?;

    let html = client
        .get(url)
        .header(reqwest::header::USER_AGENT, "Neeko Assistant")
        .send()
        .await
        .map_err(|e| format!("No pude abrir MediaFire: {}", e))?
        .text()
        .await
        .map_err(|e| format!("No pude leer MediaFire: {}", e))?;

    let re = regex::Regex::new(r#"https?://download[^"']+mediafire[^"']+"#)
        .map_err(|e| e.to_string())?;
    let found = re
        .find(&html)
        .map(|m| m.as_str().replace("&amp;", "&"))
        .ok_or_else(|| "No pude resolver el link directo de MediaFire".to_string())?;

    Ok(found)
}

fn find_file_recursive(dir: &Path, file_name: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_file_recursive(&path, file_name) {
                return Some(found);
            }
        } else if path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.eq_ignore_ascii_case(file_name))
            .unwrap_or(false)
        {
            return Some(path);
        }
    }
    None
}

pub(crate) async fn install_ffmpeg_impl(app: AppHandle) -> Result<String, String> {
    let url = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip";
    let install_dir = app_data_dir().join("tools").join("ffmpeg");
    let download_dir = app_data_dir().join("downloads");
    let zip_path = download_dir.join("ffmpeg-release-essentials.zip");
    let extract_dir = download_dir.join("ffmpeg-extract");

    download_to_file(&app, "ffmpeg", "FFmpeg + FFprobe", url, &zip_path).await?;

    let _ = std::fs::remove_dir_all(&extract_dir);
    std::fs::create_dir_all(&extract_dir).map_err(|e| e.to_string())?;
    emit_download_progress(
        &app,
        "ffmpeg",
        "FFmpeg + FFprobe",
        0,
        None,
        "Extrayendo ZIP...",
    );

    let mut command = Command::new("powershell");
    #[cfg(target_os = "windows")]
    command.creation_flags(0x08000000);
    let status = command
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            "Expand-Archive -LiteralPath $args[0] -DestinationPath $args[1] -Force",
            zip_path.to_string_lossy().as_ref(),
            extract_dir.to_string_lossy().as_ref(),
        ])
        .status()
        .map_err(|e| format!("No pude extraer FFmpeg: {}", e))?;

    if !status.success() {
        return Err("No pude extraer el ZIP de FFmpeg".to_string());
    }

    let ffmpeg_exe = find_file_recursive(&extract_dir, "ffmpeg.exe")
        .ok_or_else(|| "No encontre ffmpeg.exe dentro del ZIP".to_string())?;
    let bin_dir = ffmpeg_exe
        .parent()
        .ok_or_else(|| "No pude resolver la carpeta bin de FFmpeg".to_string())?
        .to_path_buf();

    let _ = std::fs::remove_dir_all(&install_dir);
    std::fs::create_dir_all(&install_dir).map_err(|e| e.to_string())?;
    std::fs::rename(&bin_dir, install_dir.join("bin"))
        .or_else(|_| {
            std::fs::create_dir_all(install_dir.join("bin"))?;
            for entry in std::fs::read_dir(&bin_dir)? {
                let entry = entry?;
                let target = install_dir.join("bin").join(entry.file_name());
                std::fs::copy(entry.path(), target)?;
            }
            Ok::<(), std::io::Error>(())
        })
        .map_err(|e| format!("No pude instalar FFmpeg: {}", e))?;

    let ffmpeg_path = install_dir.join("bin").join("ffmpeg.exe");
    let ffprobe_path = install_dir.join("bin").join("ffprobe.exe");
    let mut config = config::AppConfig::load();
    config.ffmpeg_path = ffmpeg_path.to_string_lossy().to_string();
    config.ffprobe_path = ffprobe_path.to_string_lossy().to_string();
    config.save()?;

    emit_download_progress(
        &app,
        "ffmpeg",
        "FFmpeg + FFprobe",
        1,
        Some(1),
        "FFmpeg y FFprobe instalados",
    );
    Ok(format!(
        "FFmpeg y FFprobe instalados en {}",
        install_dir.display()
    ))
}

#[tauri::command]
async fn install_ffmpeg(app: AppHandle) -> Result<String, String> {
    install_ffmpeg_impl(app).await
}

pub(crate) async fn install_git_impl(app: AppHandle) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| e.to_string())?;

    let release: serde_json::Value = client
        .get("https://api.github.com/repos/git-for-windows/git/releases/latest")
        .header(reqwest::header::USER_AGENT, "Neeko Assistant")
        .send()
        .await
        .map_err(|e| format!("No pude abrir GitHub releases: {}", e))?
        .json()
        .await
        .map_err(|e| format!("No pude leer GitHub releases: {}", e))?;

    let url = release["assets"]
        .as_array()
        .into_iter()
        .flatten()
        .find_map(|asset| {
            let name = asset["name"].as_str()?.to_lowercase();
            if name.ends_with("64-bit.exe") && name.starts_with("git-") {
                asset["browser_download_url"]
                    .as_str()
                    .map(|url| url.to_string())
            } else {
                None
            }
        })
        .ok_or_else(|| "No pude encontrar el instalador 64-bit de Git en GitHub".to_string())?;

    let file_re = regex::Regex::new(r#"/([^/]+\.exe)$"#).map_err(|e| e.to_string())?;
    let file_name = file_re
        .captures(&url)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str())
        .unwrap_or("Git-64-bit.exe");

    let installer_path = app_data_dir().join("downloads").join(file_name);
    download_to_file(&app, "git", "Git", &url, &installer_path).await?;

    let mut command = Command::new(&installer_path);
    command
        .spawn()
        .map_err(|e| format!("Descargue Git, pero no pude abrir el instalador: {}", e))?;

    Ok(format!(
        "Instalador de Git descargado y abierto: {}",
        installer_path.display()
    ))
}

#[tauri::command]
async fn install_git(app: AppHandle) -> Result<String, String> {
    install_git_impl(app).await
}

pub(crate) async fn install_model_impl(
    app: AppHandle,
    model_url: String,
) -> Result<String, String> {
    let url = model_url.trim();
    if url.is_empty() || !url.starts_with("http") {
        return Err("Pegame una URL directa al modelo .gguf".to_string());
    }

    let file_name = url
        .split('/')
        .last()
        .and_then(|s| s.split('?').next())
        .filter(|s| s.to_lowercase().ends_with(".gguf"))
        .unwrap_or("neeko-qwen3-4b-Q4_K_M.gguf");

    let model_path = installed_models_dir().join(file_name);
    if let Some(file_id) = google_drive_file_id(url)? {
        download_google_drive_file(&app, &file_id, &model_path).await?;
    } else {
        let resolved_url = resolve_download_url(url).await?;
        download_to_file(&app, "model", "Modelo IA", &resolved_url, &model_path).await?;
    }

    let size = model_path.metadata().map(|m| m.len()).unwrap_or(0);
    if !model_path.exists() || size == 0 {
        return Err("La descarga del modelo quedo vacia".to_string());
    }
    if size < 1024 * 1024 {
        let _ = std::fs::remove_file(&model_path);
        return Err("La descarga del modelo no parece ser un GGUF valido. Usa un enlace directo o instala el modelo desde un archivo local.".to_string());
    }

    let mut config = config::AppConfig::load();
    config.llama_auto_start = true;
    config.save()?;

    Ok(format!("Modelo instalado en {}", model_path.display()))
}

#[tauri::command]
async fn install_model(app: AppHandle, model_url: String) -> Result<String, String> {
    install_model_impl(app, model_url).await
}

pub(crate) async fn install_model_from_file_impl(
    app: AppHandle,
    source_path: String,
) -> Result<String, String> {
    let source = PathBuf::from(source_path.trim());
    if !source.exists() || !source.is_file() {
        return Err("No encontre el archivo del modelo".to_string());
    }

    let file_name = source
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "No pude leer el nombre del modelo".to_string())?;
    let file_name = sanitize_model_file_name(file_name)?;
    let total = source.metadata().map_err(|e| e.to_string())?.len();
    if total == 0 {
        return Err("El archivo del modelo esta vacio".to_string());
    }

    let target_dir = installed_models_dir();
    std::fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;
    let target = target_dir.join(file_name);

    if source == target {
        emit_download_progress(
            &app,
            "model",
            "Modelo IA",
            total,
            Some(total),
            "Modelo ya instalado",
        );
        let mut config = config::AppConfig::load();
        config.llama_auto_start = true;
        config.save()?;
        return Ok(format!(
            "Modelo ya estaba instalado en {}",
            target.display()
        ));
    }

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut input = tokio::fs::File::open(&source)
        .await
        .map_err(|e| format!("No pude abrir el modelo: {}", e))?;
    let mut output = tokio::fs::File::create(&target)
        .await
        .map_err(|e| format!("No pude crear el modelo instalado: {}", e))?;
    let mut buffer = vec![0_u8; 1024 * 1024];
    let mut copied = 0_u64;

    emit_download_progress(
        &app,
        "model",
        "Modelo IA",
        0,
        Some(total),
        "Copiando modelo...",
    );
    loop {
        let read = input
            .read(&mut buffer)
            .await
            .map_err(|e| format!("No pude leer el modelo: {}", e))?;
        if read == 0 {
            break;
        }
        output
            .write_all(&buffer[..read])
            .await
            .map_err(|e| format!("No pude copiar el modelo: {}", e))?;
        copied += read as u64;
        emit_download_progress(
            &app,
            "model",
            "Modelo IA",
            copied,
            Some(total),
            "Copiando modelo...",
        );
    }
    output
        .flush()
        .await
        .map_err(|e| format!("No pude terminar de guardar el modelo: {}", e))?;

    let mut config = config::AppConfig::load();
    config.llama_auto_start = true;
    config.save()?;

    Ok(format!("Modelo instalado en {}", target.display()))
}

#[tauri::command]
async fn install_model_from_file(app: AppHandle, source_path: String) -> Result<String, String> {
    install_model_from_file_impl(app, source_path).await
}

#[tauri::command]
fn pick_model_file(app: AppHandle) -> Result<Option<String>, String> {
    let Some(path) = app
        .dialog()
        .file()
        .add_filter("Modelos GGUF", &["gguf"])
        .blocking_pick_file()
    else {
        return Ok(None);
    };

    path.into_path()
        .map(|path| Some(path.to_string_lossy().to_string()))
        .map_err(|e| format!("No pude leer la ruta seleccionada: {}", e))
}

pub(crate) fn uninstall_ffmpeg_impl() -> Result<String, String> {
    let install_dir = app_data_dir().join("tools").join("ffmpeg");
    let download_zip = app_data_dir()
        .join("downloads")
        .join("ffmpeg-release-essentials.zip");
    let extract_dir = app_data_dir().join("downloads").join("ffmpeg-extract");

    let _ = std::fs::remove_file(download_zip);
    let _ = std::fs::remove_dir_all(extract_dir);
    if install_dir.exists() {
        std::fs::remove_dir_all(&install_dir)
            .map_err(|e| format!("No pude borrar FFmpeg: {}", e))?;
    }

    let mut config = config::AppConfig::load();
    if config
        .ffmpeg_path
        .contains("\\neeko-assistant\\tools\\ffmpeg")
        || config.ffmpeg_path.contains("/neeko-assistant/tools/ffmpeg")
    {
        config.ffmpeg_path.clear();
    }
    if config
        .ffprobe_path
        .contains("\\neeko-assistant\\tools\\ffmpeg")
        || config
            .ffprobe_path
            .contains("/neeko-assistant/tools/ffmpeg")
    {
        config.ffprobe_path.clear();
    }
    config.save()?;

    Ok("FFmpeg y FFprobe instalados por Neeko fueron eliminados".to_string())
}

#[tauri::command]
fn uninstall_ffmpeg() -> Result<String, String> {
    uninstall_ffmpeg_impl()
}

pub(crate) fn uninstall_git_impl() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        open::that("ms-settings:appsfeatures")
            .map_err(|e| format!("No pude abrir Apps instaladas: {}", e))?;
        Ok("Abrí Apps instaladas de Windows. Buscá Git y tocá Desinstalar.".to_string())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("La desinstalacion guiada de Git solo esta preparada para Windows.".to_string())
    }
}

#[tauri::command]
fn uninstall_git() -> Result<String, String> {
    uninstall_git_impl()
}

pub(crate) fn uninstall_model_impl() -> Result<String, String> {
    stop_tracked_llama_process();

    let mut config = config::AppConfig::load();
    config.llama_auto_start = false;
    config.save()?;

    let ia_dir = installed_models_dir();
    if !ia_dir.exists() {
        return Ok("No habia modelos descargados por Neeko".to_string());
    }

    let mut removed = 0_u32;
    for entry in std::fs::read_dir(&ia_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("gguf"))
            .unwrap_or(false)
        {
            std::fs::remove_file(&path)
                .map_err(|e| format!("No pude borrar {}: {}", path.display(), e))?;
            removed += 1;
        }
    }

    let _ = std::fs::remove_dir(&ia_dir);

    Ok(if removed == 0 {
        "No habia modelos GGUF descargados por Neeko".to_string()
    } else {
        format!("Modelos eliminados: {}", removed)
    })
}

#[tauri::command]
fn uninstall_model() -> Result<String, String> {
    uninstall_model_impl()
}

#[tauri::command]
fn check_dependencies() -> Result<String, String> {
    let mut missing = Vec::new();
    let config = config::AppConfig::load();

    if !command_works("git", &["--version"]) {
        missing.push("git");
    }

    let ffmpeg = configured_or_default(&config.ffmpeg_path, "ffmpeg");
    if !command_works(&ffmpeg, &["-version"]) {
        missing.push("ffmpeg");
    }

    if missing.is_empty() {
        Ok("all_ok".to_string())
    } else {
        Err(missing.join(","))
    }
}

#[derive(Serialize)]
struct ToolStatus {
    name: String,
    command: String,
    ok: bool,
    detail: String,
}

fn configured_or_default(configured: &str, default_command: &str) -> String {
    let trimmed = configured.trim();
    if trimmed.is_empty() {
        default_command.to_string()
    } else {
        trimmed.to_string()
    }
}

fn command_works(command: &str, args: &[&str]) -> bool {
    let mut cmd = Command::new(command);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);
    cmd.args(args)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn tool_status(name: &str, command: String, args: &[&str]) -> ToolStatus {
    let mut cmd = Command::new(&command);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);

    match cmd.args(args).output() {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let detail = stdout
                .lines()
                .chain(stderr.lines())
                .next()
                .unwrap_or("OK")
                .trim()
                .to_string();
            ToolStatus {
                name: name.to_string(),
                command,
                ok: true,
                detail,
            }
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            ToolStatus {
                name: name.to_string(),
                command,
                ok: false,
                detail: stderr
                    .lines()
                    .next()
                    .unwrap_or("El comando respondio con error")
                    .trim()
                    .to_string(),
            }
        }
        Err(e) => ToolStatus {
            name: name.to_string(),
            command,
            ok: false,
            detail: e.to_string(),
        },
    }
}

#[tauri::command]
fn save_environment_config(
    ffmpeg_path: Option<String>,
    ffprobe_path: Option<String>,
) -> Result<String, String> {
    let mut config = config::AppConfig::load();
    if let Some(path) = ffmpeg_path {
        config.ffmpeg_path = path.trim().to_string();
    }
    if let Some(path) = ffprobe_path {
        config.ffprobe_path = path.trim().to_string();
    }
    config.save()?;
    Ok("Herramientas guardadas".to_string())
}

#[tauri::command]
async fn check_environment_tools() -> Result<Vec<ToolStatus>, String> {
    tokio::task::spawn_blocking(|| {
        let config = config::AppConfig::load();

        vec![
            tool_status("Git", "git".to_string(), &["--version"]),
            tool_status(
                "FFmpeg",
                configured_or_default(&config.ffmpeg_path, "ffmpeg"),
                &["-version"],
            ),
            tool_status(
                "FFprobe",
                configured_or_default(&config.ffprobe_path, "ffprobe"),
                &["-version"],
            ),
        ]
    })
    .await
    .map_err(|e| format!("Error probando herramientas: {}", e))
}

#[tauri::command]
fn get_local_ip() -> Result<String, String> {
    if let Ok(socket) = std::net::UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = socket.local_addr() {
                return Ok(addr.ip().to_string());
            }
        }
    }
    Err("No pude obtener la IP".to_string())
}

fn check_system_commands_enabled() -> Result<(), String> {
    let config = config::AppConfig::load();
    if !config.system_commands_enabled {
        return Err(
            "Los comandos de sistema están desactivados. Activalos en Configuración ⚙️".to_string(),
        );
    }
    Ok(())
}

#[tauri::command]
fn system_shutdown(seconds: Option<u64>) -> Result<String, String> {
    system_shutdown_impl(seconds)
}

pub(crate) fn system_shutdown_impl(seconds: Option<u64>) -> Result<String, String> {
    check_system_commands_enabled()?;
    let delay = seconds.unwrap_or(0);
    let output = Command::new("shutdown")
        .args(["/s", "/t", &delay.to_string()])
        .creation_flags(0x08000000)
        .output()
        .map_err(|e| format!("Error: {}", e))?;
    if output.status.success() {
        if delay == 0 {
            Ok("Apagando la PC... 💤".to_string())
        } else {
            let mins = delay / 60;
            let secs = delay % 60;
            let tiempo = if mins > 0 && secs > 0 {
                format!("{} minutos y {} segundos", mins, secs)
            } else if mins > 0 {
                format!("{} minutos", mins)
            } else {
                format!("{} segundos", secs)
            };
            Ok(format!("PC se apaga en {} ⏰", tiempo))
        }
    } else {
        Err("No pude apagar la PC".to_string())
    }
}

#[tauri::command]
fn system_cancel_shutdown() -> Result<String, String> {
    system_cancel_shutdown_impl()
}

pub(crate) fn system_cancel_shutdown_impl() -> Result<String, String> {
    check_system_commands_enabled()?;
    let output = Command::new("shutdown")
        .args(["/a"])
        .creation_flags(0x08000000)
        .output()
        .map_err(|e| format!("Error: {}", e))?;
    if output.status.success() {
        Ok("Apagado cancelado ✅".to_string())
    } else {
        Err("No había ningún apagado programado".to_string())
    }
}

#[tauri::command]
fn system_restart_explorer() -> Result<String, String> {
    system_restart_explorer_impl()
}

pub(crate) fn system_restart_explorer_impl() -> Result<String, String> {
    check_system_commands_enabled()?;

    let _ = Command::new("taskkill")
        .args(["/f", "/im", "explorer.exe"])
        .creation_flags(0x08000000)
        .output();
    std::thread::sleep(std::time::Duration::from_secs(1));

    let del_cmds = [
        r#"del /A /Q "%localappdata%\IconCache.db""#,
        r#"del /A /F /Q "%localappdata%\Microsoft\Windows\Explorer\iconcache*""#,
        r#"del /A /F /Q "%localappdata%\Microsoft\Windows\Explorer\thumbcache*""#,
    ];
    for cmd in &del_cmds {
        let _ = Command::new("cmd")
            .args(["/C", cmd])
            .creation_flags(0x08000000)
            .output();
    }

    let output = Command::new("explorer.exe")
        .creation_flags(0x08000000)
        .spawn()
        .map_err(|e| format!("Error: {}", e))?;
    Ok(format!(
        "Explorer reiniciado y caché de iconos limpiada (PID: {}) 🔄",
        output.id()
    ))
}

#[tauri::command]
fn system_restart_wifi() -> Result<String, String> {
    system_restart_wifi_impl()
}

pub(crate) fn system_restart_wifi_impl() -> Result<String, String> {
    check_system_commands_enabled()?;
    let disable = Command::new("netsh")
        .args(["interface", "set", "interface", "Wi-Fi", "disable"])
        .creation_flags(0x08000000)
        .output()
        .map_err(|e| format!("Error al deshabilitar WiFi: {}", e))?;
    if !disable.status.success() {
        let msg = String::from_utf8_lossy(&disable.stderr);
        return Err(format!("No pude deshabilitar WiFi: {}", msg.trim()));
    }
    std::thread::sleep(std::time::Duration::from_secs(2));
    let enable = Command::new("netsh")
        .args(["interface", "set", "interface", "Wi-Fi", "enable"])
        .creation_flags(0x08000000)
        .output()
        .map_err(|e| format!("Error al habilitar WiFi: {}", e))?;
    if enable.status.success() {
        Ok("WiFi reiniciado correctamente 📶".to_string())
    } else {
        let msg = String::from_utf8_lossy(&enable.stderr);
        Err(format!("No pude habilitar WiFi: {}", msg.trim()))
    }
}

#[tauri::command]
fn system_restart_bluetooth() -> Result<String, String> {
    system_restart_bluetooth_impl()
}

pub(crate) fn system_restart_bluetooth_impl() -> Result<String, String> {
    check_system_commands_enabled()?;
    let ps_disable =
        r#"Get-PnpDevice -FriendlyName '*Bluetooth*' | Disable-PnpDevice -Confirm:$false"#;
    let disable = Command::new("powershell")
        .args(["-Command", ps_disable])
        .creation_flags(0x08000000)
        .output()
        .map_err(|e| format!("Error al deshabilitar Bluetooth: {}", e))?;
    if !disable.status.success() {
        let msg = String::from_utf8_lossy(&disable.stderr);
        return Err(format!("No pude deshabilitar Bluetooth: {}", msg.trim()));
    }
    std::thread::sleep(std::time::Duration::from_secs(2));
    let ps_enable =
        r#"Get-PnpDevice -FriendlyName '*Bluetooth*' | Enable-PnpDevice -Confirm:$false"#;
    let enable = Command::new("powershell")
        .args(["-Command", ps_enable])
        .creation_flags(0x08000000)
        .output()
        .map_err(|e| format!("Error al habilitar Bluetooth: {}", e))?;
    if enable.status.success() {
        Ok("Bluetooth reiniciado correctamente 🔵".to_string())
    } else {
        let msg = String::from_utf8_lossy(&enable.stderr);
        Err(format!("No pude habilitar Bluetooth: {}", msg.trim()))
    }
}

#[tauri::command]
async fn open_any_app(app: AppHandle, app_name: String) -> Result<String, String> {
    open_any_app_with_notify(Some(&app), app_name).await
}

pub async fn open_any_app_with_notify(
    app: Option<&AppHandle>,
    app_name: String,
) -> Result<String, String> {
    let result = open_any_app_impl(app_name.clone()).await;
    if let Some(app) = app {
        match &result {
            Ok(msg) => notify_system(app, "Neeko", msg),
            Err(err) => notify_system(app, "Neeko no pudo abrir la app", err),
        }
    }
    result
}

async fn open_any_app_impl(app_name: String) -> Result<String, String> {
    let query = app_name.trim().to_lowercase();

    if query.is_empty() {
        return Err("No me dijiste qué abrir 🦎".to_string());
    }

    // 1. Buscar en accesos directos del Menú Inicio (.lnk) - lanzar .exe directo SIN cmd
    if let Some(path) = find_app_in_start_menu(&query) {
        let resolved = resolve_unc_path(&path);
        let path_buf = std::path::PathBuf::from(&resolved);
        let parent_dir = path_buf.parent().unwrap_or(std::path::Path::new("C:\\"));
        eprintln!(
            "[NEEKO] Launching: {} | WorkDir: {}",
            resolved,
            parent_dir.display()
        );

        // Si es .exe, usar Command::new. Si no (protocolo, URL, etc.), usar open::that
        if resolved.to_lowercase().ends_with(".exe") {
            let result = Command::new(&resolved).current_dir(parent_dir).spawn();
            match result {
                Ok(_) => return Ok(format!("¡Encontré y abrí {}! 🦎", app_name)),
                Err(e) => {
                    eprintln!("[NEEKO] Failed to launch {}: {}", resolved, e);
                    if open::that(&resolved).is_ok() {
                        return Ok(format!("¡Encontré y abrí {}! 🦎", app_name));
                    }
                }
            }
        } else {
            // Target no es .exe (protocolo, URL, etc.) - usar open
            match open::that(&resolved) {
                Ok(_) => return Ok(format!("¡Encontré y abrí {}! 🦎", app_name)),
                Err(e) => eprintln!("[NEEKO] Failed to open {}: {}", resolved, e),
            }
        }
    } else {
        eprintln!("[NEEKO] No .lnk found for: {}", query);
    }

    // 2. Intentar abrir directamente con open
    if open::that(&app_name).is_ok() {
        return Ok(format!("Intenté abrir {} 🦎", app_name));
    }

    Err(format!("No encontré \"{}\" en el sistema 🥺", app_name))
}

/// Busca una app en los accesos directos del Menú Inicio
fn find_app_in_start_menu(query: &str) -> Option<String> {
    let mut dirs_to_scan: Vec<PathBuf> = Vec::new();

    // Menú Inicio (todos los usuarios)
    if let Ok(program_data) = std::env::var("ProgramData") {
        dirs_to_scan.push(
            PathBuf::from(&program_data)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs"),
        );
    }

    // Menú Inicio (usuario actual)
    if let Ok(appdata) = std::env::var("APPDATA") {
        dirs_to_scan.push(
            PathBuf::from(&appdata)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs"),
        );
    }

    // Escritorio (usuario actual)
    if let Ok(userprofile) = std::env::var("USERPROFILE") {
        dirs_to_scan.push(PathBuf::from(userprofile).join("Desktop"));
    }

    // Escritorio (público)
    if let Ok(public) = std::env::var("PUBLIC") {
        dirs_to_scan.push(PathBuf::from(public).join("Desktop"));
    }

    let mut seen_paths = HashSet::new();
    let mut candidates: Vec<(String, String, i32)> = Vec::new();

    for dir in &dirs_to_scan {
        eprintln!(
            "[NEEKO] Scanning dir: {} (exists={})",
            dir.display(),
            dir.exists()
        );
        if !dir.exists() {
            continue;
        }
        scan_lnk_dir(dir, query, &mut candidates, &mut seen_paths);
    }

    // Debug: mostrar todos los candidatos
    for (path, name, score) in &candidates {
        eprintln!(
            "[NEEKO] Candidate: name=\"{}\" score={} path={}",
            name, score, path
        );
    }

    // Ordenar por score (mayor primero) y devolver el mejor
    candidates.sort_by(|a, b| b.2.cmp(&a.2));
    candidates.into_iter().next().map(|(path, name, score)| {
        eprintln!(
            "[NEEKO] Best match: \"{}\" (score={}) -> {}",
            name, score, path
        );
        path
    })
}

/// Escanea recursivamente una carpeta buscando archivos .lnk
fn scan_lnk_dir(
    dir: &Path,
    query: &str,
    candidates: &mut Vec<(String, String, i32)>,
    seen: &mut HashSet<String>,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_lnk_dir(&path, query, candidates, seen);
            continue;
        }

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !ext.eq_ignore_ascii_case("lnk") {
            continue;
        }

        if let Some((target, name, score)) = parse_lnk_file(&path, query, seen) {
            candidates.push((target, name, score));
        }
    }
}

/// Parsea un archivo .lnk y devuelve (ruta_destino, nombre, score)
fn parse_lnk_file(
    lnk_path: &Path,
    query: &str,
    seen: &mut HashSet<String>,
) -> Option<(String, String, i32)> {
    let shell_link = lnk::ShellLink::open(lnk_path, lnk::encoding::WINDOWS_1252).ok()?;

    // Filtrar desinstaladores y actualizadores
    let name_lower = lnk_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    if name_lower.contains("uninstall")
        || name_lower.contains("update")
        || name_lower.contains("updater")
    {
        return None;
    }

    // Obtener la ruta destino del atajo (puede ser None si es un protocolo/URI)
    let target = match shell_link.link_target() {
        Some(t) => t.to_string(),
        None => {
            // No se pudo leer el target - usar el .lnk directamente con open::that()
            eprintln!(
                "[NEEKO] No target found for {}, using .lnk directly",
                lnk_path.display()
            );
            let lnk_str = lnk_path.to_str()?.to_string();
            let name = lnk_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown")
                .to_string();
            let name_lower = name.to_lowercase();
            let score = calculate_lnk_score(&name_lower, &lnk_str.to_lowercase(), query);
            return Some((lnk_str, name, score));
        }
    };

    let target_lower = target.to_lowercase();

    // Deduplicar por ruta destino
    if seen.contains(&target_lower) {
        return None;
    }
    seen.insert(target_lower.clone());

    let name = lnk_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown")
        .to_string();

    let name_lower = name.to_lowercase();
    let score = calculate_lnk_score(&name_lower, &target_lower, query);

    eprintln!("[NEEKO] LNK: \"{}\" -> {} (score={})", name, target, score);

    Some((target, name, score))
}

/// Resuelve un UNC path (\\hostname\Users\...) a un path local (C:\Users\...)
fn resolve_unc_path(path: &str) -> String {
    if !path.starts_with("\\\\") {
        return path.to_string();
    }

    if let Ok(home) = std::env::var("USERPROFILE") {
        if let Some(rest) = path.get(2..) {
            if let Some(pos) = rest.find('\\') {
                let after_host = &rest[pos..];
                if let Some(users_pos) = after_host.find("\\Users\\") {
                    let after_users = &after_host[users_pos + 7..];
                    if let Some(slash_pos) = after_users.find('\\') {
                        let username = &after_users[..slash_pos];
                        let remaining = &after_users[slash_pos..];
                        if let Some(drive) = home.get(0..2) {
                            let local_path = format!("{}\\Users\\{}{}", drive, username, remaining);
                            let local = std::path::Path::new(&local_path);
                            if local.exists() {
                                eprintln!("[NEEKO] UNC resolved: {} -> {}", path, local_path);
                                return local_path;
                            }
                        }
                    }
                }
            }
        }
    }

    path.to_string()
}

/// Calcula un score de coincidencia entre el nombre del atajo y la búsqueda
fn calculate_lnk_score(name: &str, target: &str, query: &str) -> i32 {
    let mut score = 0;

    // Stop words que no deben causar match
    let stop_words = [
        "the", "a", "an", "of", "for", "and", "or", "to", "in", "on", "at", "by", "de", "la", "el",
        "en", "un", "una", "los", "las",
    ];

    // Coincidencia exacta del nombre
    if name == query {
        return 1000;
    }

    // El nombre empieza con el query
    if name.starts_with(query) {
        score += 500;
    }

    // El nombre contiene el query completo
    if name.contains(query) {
        score += 200;
    }

    // El target (exe) contiene el query
    if target.contains(query) {
        score += 300;
    }

    // Palabras clave individuales (splits por espacio)
    let query_words: Vec<&str> = query.split_whitespace().collect();
    for word in &query_words {
        // Ignorar stop words y palabras muy cortas
        if word.len() < 3 || stop_words.contains(&word) {
            continue;
        }
        if name.contains(word) {
            score += 50;
        }
        if target.contains(word) {
            score += 30;
        }
    }

    // Bonus: cada palabra del query está en el nombre (en orden)
    let name_words: Vec<&str> = name.split_whitespace().collect();
    let mut all_found = true;
    for qw in &query_words {
        if qw.len() < 3 || stop_words.contains(&qw) {
            continue;
        }
        if !name_words.iter().any(|nw| nw.contains(qw)) {
            all_found = false;
            break;
        }
    }
    if all_found
        && query_words
            .iter()
            .filter(|w| w.len() >= 3 && !stop_words.contains(w))
            .count()
            > 1
    {
        score += 100;
    }

    score
}

#[tauri::command]
async fn check_updates(app: AppHandle) -> Result<serde_json::Value, String> {
    let current = env!("CARGO_PKG_VERSION").to_string();
    let updater = app
        .updater()
        .map_err(|e| format!("Updater no configurado: {}", e))?;
    let update = updater
        .check()
        .await
        .map_err(|e| format!("Error al buscar actualizaciones: {}", e))?;

    let Some(update) = update else {
        return Ok(serde_json::json!({
            "hasUpdate": false,
            "currentVersion": current,
            "latestVersion": current,
        }));
    };

    let download_url = update.download_url.to_string();
    let notes = update.body.unwrap_or_default();
    let version = update.version;

    Ok(serde_json::json!({
        "hasUpdate": true,
        "currentVersion": current,
        "latestVersion": version,
        "releaseName": format!("Neeko Assistant {}", version),
        "downloadUrl": download_url,
        "notes": notes,
    }))
}

#[tauri::command]
async fn download_and_install_update(app: AppHandle) -> Result<String, String> {
    let updater = app
        .updater()
        .map_err(|e| format!("Updater no configurado: {}", e))?;
    let update = updater
        .check()
        .await
        .map_err(|e| format!("Error al buscar actualizaciones: {}", e))?
        .ok_or_else(|| "No hay una actualizacion nueva para instalar.".to_string())?;

    let version = update.version.clone();
    update
        .download_and_install(
            move |_chunk, _total| {},
            || {},
        )
        .await
        .map_err(|e| format!("Error al instalar la actualizacion: {}", e))?;

    app.request_restart();
    Ok(format!("Actualizacion a v{} instalada. Reiniciando...", version))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(windows)]
    unsafe {
        check_single_instance_windows();
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let app_handle = app.handle().clone();
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(web_server::start_web_server(app_handle));
            });

            let open_item = MenuItem::with_id(app, "show", "Abrir", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Cerrar", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open_item, &quit_item])?;
            let mut tray = TrayIconBuilder::new()
                .menu(&menu)
                .show_menu_on_left_click(false)
                .tooltip("Neeko Assistant");

            if let Some(icon) = app.default_window_icon().cloned() {
                tray = tray.icon(icon);
            }

            tray.build(app)?;
            Ok(())
        })
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "quit" => {
                EXIT_REQUESTED.store(true, Ordering::SeqCst);
                cleanup_before_exit();
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            minimize_window,
            close_window,
            open_app,
            open_url,
            search_web,
            open_folder,
            check_local_ai,
            list_models,
            get_model_path_cmd,
            chat_start,
            chat_cancel,
            chat_finish,
            install_ffmpeg,
            install_git,
            install_model,
            install_model_from_file,
            pick_model_file,
            uninstall_ffmpeg,
            uninstall_git,
            uninstall_model,
            check_dependencies,
            check_environment_tools,
            save_environment_config,
            open_any_app,
            get_local_ip,
            git_commands::git_check_installed,
            git_commands::git_init,
            git_commands::git_add,
            git_commands::git_commit,
            git_commands::git_push,
            git_commands::git_pull,
            git_commands::git_status,
            git_commands::git_log,
            git_commands::git_branch,
            git_commands::git_remote_add,
            lol_api::lol_get_match_history,
            lol_api::lol_save_config,
            lol_api::lol_get_config,
            lol_api::lol_get_rank,
            video_compress::compress_for_discord,
            llama_status,
            get_llama_auto_start,
            set_llama_auto_start,
            start_llama_server,
            stop_llama_server,
            system_shutdown,
            system_cancel_shutdown,
            system_restart_explorer,
            system_restart_wifi,
            system_restart_bluetooth,
            get_system_commands_enabled,
            set_system_commands_enabled,
            cancel_download,
            check_updates,
            download_and_install_update,
        ])
        .on_window_event(|_window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if !EXIT_REQUESTED.load(Ordering::SeqCst) {
                    api.prevent_close();
                    let _ = _window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running neeko assistant");
}
