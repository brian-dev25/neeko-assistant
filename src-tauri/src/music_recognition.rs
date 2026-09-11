use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::Emitter;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

static HISTORY_LOCK: Mutex<()> = Mutex::new(());

#[derive(Serialize, Deserialize)]
struct HistorySettings { limit: usize }

fn history_limit() -> Result<usize, String> {
    let path = crate::app_data_dir().join("shazam-settings.json");
    match std::fs::read(path) {
        Ok(data) => {
            let settings: HistorySettings = serde_json::from_slice(&data).map_err(|e| e.to_string())?;
            validate_limit(settings.limit)?;
            Ok(settings.limit)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(15),
        Err(e) => Err(e.to_string()),
    }
}

fn validate_limit(limit: usize) -> Result<(), String> {
    if (1..=1000).contains(&limit) { Ok(()) }
    else { Err("Elegí entre 1 y 1000 canciones.".into()) }
}

#[tauri::command]
pub fn shazam_history_limit(window: tauri::WebviewWindow) -> Result<usize, String> {
    if !matches!(window.label(), "main" | "settings") { return Err("Ventana no válida".into()); }
    let _lock = HISTORY_LOCK.lock().map_err(|e| e.to_string())?;
    history_limit()
}

#[tauri::command]
pub fn shazam_set_history_limit(window: tauri::WebviewWindow, limit: usize) -> Result<(), String> {
    if !matches!(window.label(), "main" | "settings") { return Err("Ventana no válida".into()); }
    validate_limit(limit)?;
    let _lock = HISTORY_LOCK.lock().map_err(|e| e.to_string())?;
    let path = crate::app_data_dir().join("shazam-settings.json");
    let mut songs = read_history(&history_path())?;
    songs.truncate(limit);
    std::fs::create_dir_all(crate::app_data_dir()).map_err(|e| e.to_string())?;
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, serde_json::to_vec(&HistorySettings { limit }).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(temporary, path).map_err(|e| e.to_string())?;
    write_history(&history_path(), &songs)
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Song {
    id: u64,
    title: String,
    artist: String,
    found_at: u64,
}

fn history_path() -> std::path::PathBuf { crate::app_data_dir().join("shazam-history.json") }

fn read_history(path: &std::path::Path) -> Result<Vec<Song>, String> {
    match std::fs::read(path) {
        Ok(data) => serde_json::from_slice(&data).map_err(|e| format!("No se pudo leer el historial: {e}")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e.to_string()),
    }
}

fn write_history(path: &std::path::Path, songs: &[Song]) -> Result<(), String> {
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
    let data = serde_json::to_vec_pretty(songs).map_err(|e| e.to_string())?;
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, data).map_err(|e| e.to_string())?;
    std::fs::rename(temporary, path).map_err(|e| e.to_string())
}

fn add_song(songs: &mut Vec<Song>, result: &serde_json::Value, now: u64, limit: usize) -> bool {
    let Some(title) = result.get("title").and_then(|v| v.as_str()).filter(|s| !s.trim().is_empty()) else { return false; };
    let artist = result.get("artist").and_then(|v| v.as_str()).unwrap_or("");
    let id = songs.iter().map(|song| song.id).max().unwrap_or(0).saturating_add(1).max(now);
    songs.insert(0, Song { id, title: title.into(), artist: artist.into(), found_at: now });
    songs.truncate(limit);
    true
}

fn save_song(result: &serde_json::Value) -> Result<(), String> {
    if result.get("title").and_then(|v| v.as_str()).is_none_or(|s| s.trim().is_empty()) { return Ok(()); }
    let _lock = HISTORY_LOCK.lock().map_err(|e| e.to_string())?;
    let path = history_path();
    let mut songs = read_history(&path)?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_millis() as u64;
    if add_song(&mut songs, result, now, history_limit()?) { write_history(&path, &songs)?; }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn only_matches_enter_history_and_newest_comes_first() {
        let mut songs = Vec::new();
        assert!(!add_song(&mut songs, &json!({"message": "No encontré la canción"}), 1, 15));
        assert!(!add_song(&mut songs, &json!({"title": " "}), 1, 15));
        assert!(add_song(&mut songs, &json!({"title": "First", "artist": "A"}), 10, 15));
        assert!(add_song(&mut songs, &json!({"title": "Second", "artist": "B"}), 10, 15));
        assert_eq!(songs[0].title, "Second");
        assert_ne!(songs[0].id, songs[1].id);
        assert_eq!(songs[0].found_at, 10);
    }

    #[test]
    fn history_is_bounded_and_survives_reloading_and_deletion() {
        let mut songs = Vec::new();
        for n in 0..20 { add_song(&mut songs, &json!({"title": format!("Song {n}"), "artist": "Artist"}), n, 15); }
        assert_eq!(songs.len(), 15);
        assert_eq!(songs.last().unwrap().title, "Song 5");
        let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("neeko-history-test-{}-{unique}.json", std::process::id()));
        write_history(&path, &songs).unwrap();
        let mut reloaded = read_history(&path).unwrap();
        assert_eq!(reloaded[0].title, "Song 19");
        reloaded.remove(0);
        write_history(&path, &reloaded).unwrap();
        assert_eq!(read_history(&path).unwrap().len(), 14);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn configurable_history_retains_newest_and_rejects_invalid_limits() {
        let mut songs = Vec::new();
        for n in 0..30 { add_song(&mut songs, &json!({"title": format!("Song {n}")}), n, 25); }
        assert_eq!(songs.len(), 25);
        add_song(&mut songs, &json!({"title": "Newest"}), 30, 3);
        assert_eq!(songs.len(), 3);
        assert_eq!(songs[0].title, "Newest");
        assert!(validate_limit(0).is_err());
        assert!(validate_limit(1001).is_err());
        assert!(validate_limit(15).is_ok());
    }
}

#[tauri::command]
pub fn shazam_history(window: tauri::WebviewWindow) -> Result<Vec<Song>, String> {
    if !matches!(window.label(), "main" | "settings") { return Err("Ventana no válida".into()); }
    let _lock = HISTORY_LOCK.lock().map_err(|e| e.to_string())?;
    let mut songs = read_history(&history_path())?;
    let previous_count = songs.len();
    songs.truncate(history_limit()?);
    if songs.len() != previous_count { write_history(&history_path(), &songs)?; }
    Ok(songs)
}

#[tauri::command]
pub fn shazam_history_delete(window: tauri::WebviewWindow, id: u64) -> Result<Vec<Song>, String> {
    if !matches!(window.label(), "main" | "settings") { return Err("Ventana no válida".into()); }
    let _lock = HISTORY_LOCK.lock().map_err(|e| e.to_string())?;
    let path = history_path();
    let mut songs = read_history(&path)?;
    songs.retain(|song| song.id != id);
    write_history(&path, &songs)?;
    Ok(songs)
}

fn runtime_python() -> Result<std::path::PathBuf, String> {
    Ok(dirs::config_dir().ok_or("No se encontró la carpeta de configuración")?
        .join("neeko-assistant").join("shazam-runtime").join("Scripts").join("python.exe"))
}

#[tauri::command]
pub async fn shazam_prepare(app: tauri::AppHandle, window: tauri::WebviewWindow) -> Result<String, String> {
    if !matches!(window.label(), "main" | "settings") || !crate::addon_manager().is_enabled("shazam") {
        return Err("Activá el addon Shazam primero".into());
    }
    if BUSY.swap(true, Ordering::SeqCst) { return Err("Shazam ya está trabajando".into()); }
    // Keep the guard in the worker so a dropped request cannot unlock an active install.
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = BusyGuard;
        let progress = |message: &str| { let _ = app.emit("shazam-setup-progress", message); };
        progress("Buscando Python…");
        let system_python = match crate::find_system_python() {
            Some(python) => python,
            None => {
                progress("Instalando Python. Esto puede tardar unos minutos…");
                crate::install_python_with_winget(&app)?;
                crate::find_system_python().ok_or("Reiniciá Neeko para detectar Python y volvé a preparar Shazam")?
            }
        };
        let python = runtime_python()?;
        let run = |executable: &std::path::Path, args: &[&str]| -> Result<(), String> {
            let mut command = std::process::Command::new(executable);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                command.creation_flags(0x08000000);
            }
            let output = command.args(args).output().map_err(|e| e.to_string())?;
            if !output.status.success() {
                let detail = String::from_utf8_lossy(&output.stderr);
                return Err(format!("No se pudo preparar Shazam: {}", detail.chars().rev().take(1800).collect::<String>().chars().rev().collect::<String>()));
            }
            Ok(())
        };
        if !python.is_file() {
            progress("Creando el entorno de Shazam…");
            let directory = python.parent().and_then(|p| p.parent()).ok_or("Ruta de Python inválida")?;
            std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
            run(&system_python, &["-m", "venv", directory.to_str().ok_or("Ruta de Python inválida")?])?;
        }
        progress("Instalando dependencias de Shazam. Esto puede tardar unos minutos…");
        let requirements = include_str!("../../addons/shazam/requirements.txt");
        let mut args = vec!["-m", "pip", "install", "--disable-pip-version-check"];
        args.extend(requirements.lines().map(str::trim).filter(|line| !line.is_empty()));
        run(&python, &args)?;
        progress("Verificando la instalación…");
        run(&python, &["-I", "-c", "import soundcard, numpy, shazamio"])?;
        Ok("Shazam listo. Ya podés pulsar Escuchar.".to_string())
    }).await.map_err(|e| e.to_string())?
}

static BUSY: AtomicBool = AtomicBool::new(false);
static CANCEL: AtomicBool = AtomicBool::new(false);
struct BusyGuard;
impl Drop for BusyGuard {
    fn drop(&mut self) { BUSY.store(false, Ordering::SeqCst); }
}

#[tauri::command]
pub fn shazam_cancel(window: tauri::WebviewWindow) -> Result<(), String> {
    if !matches!(window.label(), "main" | "settings") { return Err("Solo disponible en Neeko".into()); }
    CANCEL.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub async fn shazam_listen(window: tauri::WebviewWindow) -> Result<serde_json::Value, String> {
    if !matches!(window.label(), "main" | "settings") || !crate::addon_manager().is_enabled("shazam") {
        return Err("Activá el addon Shazam primero".into());
    }
    if BUSY.swap(true, Ordering::SeqCst) { return Err("Ya hay una escucha en curso".into()); }
    let _guard = BusyGuard;
    CANCEL.store(false, Ordering::SeqCst);
    let python = runtime_python()?;
    if !python.is_file() {
        return Err("Pulsá Preparar Shazam en la pestaña Shazam para instalar lo necesario".into());
    }
    let mut command = tokio::process::Command::new(python);
    command.args(["-I", "-c", include_str!("../../addons/shazam/recognize.py")])
        .kill_on_drop(true).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    let child = command.spawn().map_err(|e| e.to_string())?;
    let output = child.wait_with_output();
    tokio::pin!(output);
    let deadline = tokio::time::sleep(Duration::from_secs(60));
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            result = &mut output => {
                let result = result.map_err(|e| e.to_string())?;
                let mut value: serde_json::Value = serde_json::from_slice(&result.stdout)
                    .map_err(|_| "No se pudo iniciar el reconocimiento. Pulsá Preparar Shazam para reparar la instalación".to_string())?;
                if let Some(error) = value.get("error").and_then(|v| v.as_str()) { return Err(error.into()); }
                if CANCEL.load(Ordering::SeqCst) { return Err("Escucha cancelada".into()); }
                if let Err(error) = save_song(&value) { value["history_error"] = error.into(); }
                return Ok(value);
            }
            _ = &mut deadline => return Err("Se agotó el tiempo de reconocimiento. Intentá otra vez".into()),
            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                if CANCEL.load(Ordering::SeqCst) || !crate::addon_manager().is_enabled("shazam") {
                    return Err("Escucha cancelada".into());
                }
            }
        }
    }
}
