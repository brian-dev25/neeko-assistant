use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::Emitter;

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
                let value: serde_json::Value = serde_json::from_slice(&result.stdout)
                    .map_err(|_| "No se pudo iniciar el reconocimiento. Pulsá Preparar Shazam para reparar la instalación".to_string())?;
                if let Some(error) = value.get("error").and_then(|v| v.as_str()) { return Err(error.into()); }
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
