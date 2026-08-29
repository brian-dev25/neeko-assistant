use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub git_pat: String,
    pub git_default_path: String,
    #[serde(default)]
    pub ffmpeg_path: String,
    #[serde(default)]
    pub ffprobe_path: String,
    #[serde(default = "default_neeko_sprite")]
    pub neeko_sprite: String,
    pub lol_region: String,
    pub riot_id: String,
    #[serde(default = "default_true")]
    pub llama_auto_start: bool,
    #[serde(default)]
    pub system_commands_enabled: bool,
}

fn default_true() -> bool {
    true
}

fn default_neeko_sprite() -> String {
    "NEEKO.png".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            git_pat: String::new(),
            git_default_path: String::new(),
            ffmpeg_path: String::new(),
            ffprobe_path: String::new(),
            neeko_sprite: default_neeko_sprite(),
            lol_region: "las".to_string(),
            riot_id: String::new(),
            llama_auto_start: default_true(),
            system_commands_enabled: false,
        }
    }
}

impl AppConfig {
    fn config_path() -> Option<PathBuf> {
        let dir = dirs::config_dir()
            .or_else(|| dirs::data_dir())
            .or_else(|| dirs::home_dir())?;
        Some(dir.join("neeko-assistant").join("config.json"))
    }

    pub fn load() -> Self {
        let path = match Self::config_path() {
            Some(p) => p,
            None => return Self::default(),
        };
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path().ok_or("No se pudo determinar ruta de config")?;
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(
            &path,
            serde_json::to_string_pretty(self).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    }
}
