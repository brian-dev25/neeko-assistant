use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ModelLoadEngine {
    Llama,
    Python,
}

impl ModelLoadEngine {
    pub fn from_user_value(value: &str) -> Result<Self, String> {
        match value.trim().to_lowercase().as_str() {
            "llama" | "llama-server" => Ok(Self::Llama),
            "python" | "llama-cpp-python" => Ok(Self::Python),
            _ => Err("Motor de carga invalido".to_string()),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Llama => "llama",
            Self::Python => "python",
        }
    }
}

fn default_model_load_engine() -> ModelLoadEngine {
    ModelLoadEngine::Llama
}

fn default_llama_gpu_layers() -> u32 {
    15
}

fn default_python_gpu_layers() -> u32 {
    0
}

fn default_llama_context_size() -> u32 {
    1024
}

fn default_python_context_size() -> u32 {
    4096
}

fn default_model_threads() -> u32 {
    4
}

fn default_language() -> String {
    "es".to_string()
}

pub fn normalize_language(language: &str) -> Option<&'static str> {
    match language.trim().to_lowercase().as_str() {
        "es" | "esp" | "espanol" | "spanish" => Some("es"),
        "en" | "eng" | "ingles" | "english" => Some("en"),
        _ => None,
    }
}

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
    pub llama_was_running: bool,
    #[serde(default = "default_model_load_engine")]
    pub model_load_engine: ModelLoadEngine,
    #[serde(default = "default_llama_gpu_layers")]
    pub llama_gpu_layers: u32,
    #[serde(default = "default_python_gpu_layers")]
    pub python_gpu_layers: u32,
    #[serde(default = "default_llama_context_size")]
    pub llama_context_size: u32,
    #[serde(default = "default_python_context_size")]
    pub python_context_size: u32,
    #[serde(default = "default_model_threads")]
    pub llama_threads: u32,
    #[serde(default = "default_model_threads")]
    pub python_threads: u32,
    #[serde(default)]
    pub system_commands_enabled: bool,
    #[serde(default)]
    pub start_with_windows: bool,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default)]
    pub render_3d: bool,
    #[serde(default = "default_neeko_3d_animation")]
    pub neeko_3d_animation: String,
}

fn default_true() -> bool {
    true
}

fn default_neeko_sprite() -> String {
    "NEEKO.png".to_string()
}

fn default_neeko_3d_animation() -> String {
    "Neeko_idle3.anm".to_string()
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
            llama_was_running: false,
            model_load_engine: default_model_load_engine(),
            llama_gpu_layers: default_llama_gpu_layers(),
            python_gpu_layers: default_python_gpu_layers(),
            llama_context_size: default_llama_context_size(),
            python_context_size: default_python_context_size(),
            llama_threads: default_model_threads(),
            python_threads: default_model_threads(),
            system_commands_enabled: false,
            start_with_windows: false,
            language: default_language(),
            render_3d: false,
            neeko_3d_animation: default_neeko_3d_animation(),
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
