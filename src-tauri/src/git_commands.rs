use std::path::Path;
use std::process::Command;

use crate::config::AppConfig;
use crate::notify_system;
use tauri::AppHandle;

fn git_cmd(path: &str, args: &[&str]) -> Result<String, String> {
    let path_obj = Path::new(path);
    if !path_obj.exists() {
        return Err(format!("La ruta {} no existe", path));
    }
    if !path_obj.join(".git").exists() {
        return Err(format!(
            "No es un repositorio git (no hay .git en {})",
            path
        ));
    }
    let mut full_args = vec!["-C", path];
    full_args.extend(args);
    let output = Command::new("git").args(&full_args).output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            "Git no está instalado o no está en el PATH. Descargalo de https://git-scm.com"
                .to_string()
        } else {
            format!("Error ejecutando git: {}", e)
        }
    })?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        let err = stderr.trim();
        if err.is_empty() {
            return Err("Error desconocido de git".to_string());
        }
        return Err(err.to_string());
    }
    Ok(stdout.trim().to_string())
}

fn default_path() -> String {
    let config = AppConfig::load();
    let git_default_path = config.git_default_path.trim();
    if !git_default_path.is_empty() {
        return git_default_path.to_string();
    }

    std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| ".".to_string())
}

fn notify_git_result(app: &AppHandle, success_title: &str, result: &Result<String, String>) {
    match result {
        Ok(msg) => notify_system(app, success_title, msg),
        Err(err) => notify_system(app, "Git fallo", err),
    }
}

#[tauri::command]
pub fn git_check_installed() -> Result<String, String> {
    let output = Command::new("git").arg("--version").output();
    match output {
        Ok(o) if o.status.success() => Ok(String::from_utf8_lossy(&o.stdout).trim().to_string()),
        _ => Err("Git no está instalado. Descargalo de https://git-scm.com".to_string()),
    }
}

#[tauri::command]
pub fn git_init(app: AppHandle, path: Option<String>) -> Result<String, String> {
    let p = path.unwrap_or_else(default_path);
    let result = git_cmd(&p, &["init"]).map(|_| format!("Repositorio inicializado en {}", p));
    notify_git_result(&app, "Git init terminado", &result);
    result
}

#[tauri::command]
pub fn git_add(
    app: AppHandle,
    path: Option<String>,
    files: Option<String>,
) -> Result<String, String> {
    let p = path.unwrap_or_else(default_path);
    let f = files.unwrap_or_else(|| ".".to_string());
    let file_args: Vec<&str> = f.split_whitespace().collect();
    let mut args = vec!["add"];
    args.extend(file_args.iter());
    let result = git_cmd(&p, &args).map(|_| format!("Archivos añadidos: {}", f));
    notify_git_result(&app, "Git add terminado", &result);
    result
}

#[tauri::command]
pub fn git_commit(app: AppHandle, path: Option<String>, message: String) -> Result<String, String> {
    let p = path.unwrap_or_else(default_path);
    let result =
        git_cmd(&p, &["commit", "-m", &message]).map(|_| format!("Commit realizado: {}", message));
    notify_git_result(&app, "Git commit terminado", &result);
    result
}

#[tauri::command]
pub fn git_push(app: AppHandle, path: Option<String>) -> Result<String, String> {
    let p = path.unwrap_or_else(default_path);
    let config = AppConfig::load();

    let remote = git_cmd(&p, &["remote"])?;
    let remote_name = remote.lines().next().unwrap_or("origin");

    if !config.git_pat.is_empty() {
        let remote_url = git_cmd(&p, &["remote", "get-url", remote_name])?;
        if remote_url.contains("github.com") {
            let auth_url =
                remote_url.replacen("https://", &format!("https://{}@", config.git_pat), 1);
            let _ = git_cmd(&p, &["remote", "set-url", remote_name, &auth_url]);
        }
    }

    git_cmd(&p, &["push", remote_name, "--porcelain"])?;
    let result = Ok("Push realizado con exito".to_string());
    notify_git_result(&app, "Git push terminado", &result);
    result
}

#[tauri::command]
pub fn git_pull(app: AppHandle, path: Option<String>) -> Result<String, String> {
    let p = path.unwrap_or_else(default_path);
    let config = AppConfig::load();

    let remote = git_cmd(&p, &["remote"])?;
    let remote_name = remote.lines().next().unwrap_or("origin");

    if !config.git_pat.is_empty() {
        let remote_url = git_cmd(&p, &["remote", "get-url", remote_name])?;
        if remote_url.contains("github.com") {
            let auth_url =
                remote_url.replacen("https://", &format!("https://{}@", config.git_pat), 1);
            let _ = git_cmd(&p, &["remote", "set-url", remote_name, &auth_url]);
        }
    }

    git_cmd(&p, &["pull", remote_name])?;
    let result = Ok("Pull realizado con exito".to_string());
    notify_git_result(&app, "Git pull terminado", &result);
    result
}

#[tauri::command]
pub fn git_status(path: Option<String>) -> Result<String, String> {
    let p = path.unwrap_or_else(default_path);
    let output = git_cmd(&p, &["status", "--short"])?;
    if output.is_empty() {
        Ok("Working tree limpio, no hay cambios pendientes ✅".to_string())
    } else {
        Ok(format!("Cambios pendientes:\n{}", output))
    }
}

#[tauri::command]
pub fn git_log(path: Option<String>, count: Option<i32>) -> Result<String, String> {
    let p = path.unwrap_or_else(default_path);
    let n = count.unwrap_or(10).max(1);
    let limit = format!("-{}", n);
    git_cmd(&p, &["log", &limit, "--oneline", "--graph", "--decorate"])
}

#[tauri::command]
pub fn git_branch(path: Option<String>) -> Result<String, String> {
    let p = path.unwrap_or_else(default_path);
    git_cmd(&p, &["branch", "-a"])
}

#[tauri::command]
pub fn git_remote_add(
    app: AppHandle,
    path: Option<String>,
    name: String,
    url: String,
) -> Result<String, String> {
    let p = path.unwrap_or_else(default_path);
    let result = git_cmd(&p, &["remote", "add", &name, &url])
        .map(|_| format!("Remote '{}' añadido: {}", name, url));
    notify_git_result(&app, "Git remote terminado", &result);
    result
}
