use super::*;
use tauri_plugin_dialog::DialogExt;

pub(super) async fn export(app: &AppHandle) -> Result<(), String> {
    let app_copy = app.clone();
    let path = tokio::task::spawn_blocking(move || app_copy.dialog().file()
        .add_filter("Screen Translate", &["json"]).set_file_name("screen-translate.json").blocking_save_file())
        .await.map_err(|e| e.to_string())?;
    let Some(path) = path else { return Ok(()); };
    let s = snapshot();
    let config = Config { settings: s.settings, profiles: s.profiles };
    let data = serde_json::to_vec_pretty(&config).map_err(|e| e.to_string())?;
    tokio::fs::write(path.into_path().map_err(|e|e.to_string())?, data).await.map_err(|e|e.to_string())?;
    update(app, None, |s| s.message = "Configuración exportada.".into());
    Ok(())
}

pub(super) async fn import(app: &AppHandle) -> Result<(), String> {
    let app_copy = app.clone();
    let path = tokio::task::spawn_blocking(move || app_copy.dialog().file()
        .add_filter("Screen Translate", &["json"]).blocking_pick_file())
        .await.map_err(|e| e.to_string())?;
    let Some(path) = path else { return Ok(()); };
    let path = path.into_path().map_err(|e|e.to_string())?;
    let file = tokio::fs::File::open(&path).await.map_err(|e|e.to_string())?;
    use tokio::io::AsyncReadExt;
    let mut data = Vec::new();
    file.take(1_048_577).read_to_end(&mut data).await.map_err(|e|e.to_string())?;
    if data.len() > 1_048_576 { return Err("El archivo de configuración supera 1 MB.".into()); }
    let config: Config = serde_json::from_slice(&data).map_err(|e|format!("Configuración inválida: {e}"))?;
    config.settings.validate()?;
    validate_profiles(&config.profiles)?;
    save_config(&config.settings, &config.profiles)?;
    update(app, None, |s| { s.settings=config.settings; s.profiles=config.profiles; s.message="Configuración importada.".into(); });
    Ok(())
}
