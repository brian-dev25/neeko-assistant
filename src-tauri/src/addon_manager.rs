use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AddonManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default = "default_min_app_version")]
    pub min_app_version: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub dangerous_permissions: Vec<String>,
    #[serde(default)]
    pub commands: Vec<AddonCommand>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_min_app_version() -> String {
    "1.0.0".to_string()
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AddonCommand {
    pub id: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub patterns: HashMap<String, Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AddonInfo {
    pub manifest: AddonManifest,
    pub enabled: bool,
    pub has_js: bool,
    pub has_css: bool,
}

fn addon_config_path() -> PathBuf {
    dirs::config_dir()
        .or_else(|| dirs::data_dir())
        .or_else(|| dirs::home_dir())
        .unwrap_or_else(|| PathBuf::from("."))
        .join("neeko-assistant")
        .join("addons.json")
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct AddonConfig {
    enabled_addons: Vec<String>,
}

fn load_addon_config() -> AddonConfig {
    let path = addon_config_path();
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_addon_config(config: &AddonConfig) -> Result<(), String> {
    let path = addon_config_path();
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(
        &path,
        serde_json::to_string_pretty(config).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

pub struct AddonManager {
    addon_dirs: Vec<PathBuf>,
    enabled_cache: Arc<Mutex<Vec<String>>>,
}

impl AddonManager {
    pub fn new() -> Self {
        Self::with_resource_dir(None)
    }

    pub fn with_resource_dir(resource_dir: Option<PathBuf>) -> Self {
        let config_dir = dirs::config_dir()
            .or_else(|| dirs::data_dir())
            .or_else(|| dirs::home_dir())
            .unwrap_or_else(|| PathBuf::from("."))
            .join("neeko-assistant")
            .join("addons");

        let mut addon_dirs = vec![config_dir.clone()];
        if let Some(resource_dir) = resource_dir {
            addon_dirs.push(resource_dir.join("addons").join("addons"));
            addon_dirs.push(resource_dir.join("addons"));
        }
        for candidate in Self::dev_addon_dirs() {
            if !addon_dirs.iter().any(|dir| dir == &candidate) {
                addon_dirs.push(candidate);
            }
        }

        let config = load_addon_config();
        Self {
            addon_dirs,
            enabled_cache: Arc::new(Mutex::new(config.enabled_addons)),
        }
    }

    fn dev_addon_dirs() -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        if let Ok(current_dir) = std::env::current_dir() {
            dirs.push(current_dir.join("addons"));
            if let Some(parent) = current_dir.parent() {
                dirs.push(parent.join("addons"));
            }
        }
        if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
            let manifest_dir = PathBuf::from(manifest_dir);
            dirs.push(manifest_dir.join("addons"));
            if let Some(parent) = manifest_dir.parent() {
                dirs.push(parent.join("addons"));
            }
        }
        dirs.into_iter().filter(|dir| dir.exists()).collect()
    }

    pub fn addon_dir(&self) -> &PathBuf {
        &self.addon_dirs[0]
    }

    pub fn scan_addons(&self) -> Vec<AddonInfo> {
        let mut infos = Vec::new();

        if let Some(config_addon_dir) = self.addon_dirs.first() {
            if !config_addon_dir.exists() {
                let _ = std::fs::create_dir_all(config_addon_dir);
            }
        }

        let enabled = self.enabled_cache.lock().unwrap().clone();
        let config_exists = addon_config_path().exists();

        for addon_dir in &self.addon_dirs {
            if !addon_dir.exists() {
                continue;
            }

            for entry in std::fs::read_dir(addon_dir).into_iter().flatten().flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let manifest_path = path.join("addon.json");
                if !manifest_path.exists() {
                    continue;
                }
                match std::fs::read_to_string(&manifest_path) {
                    Ok(content) => match serde_json::from_str::<AddonManifest>(&content) {
                        Ok(mut manifest) => {
                            if infos
                                .iter()
                                .any(|info: &AddonInfo| info.manifest.id == manifest.id)
                            {
                                continue;
                            }
                            let is_enabled = if config_exists {
                                enabled.contains(&manifest.id)
                            } else {
                                manifest.enabled
                            };
                            manifest.enabled = is_enabled;
                            infos.push(AddonInfo {
                                manifest,
                                enabled: is_enabled,
                                has_js: path.join("main.js").exists(),
                                has_css: path.join("styles.css").exists(),
                            });
                        }
                        Err(e) => {
                            eprintln!(
                                "[NEEKO ADDON] Manifesto invalido en {}: {}",
                                path.display(),
                                e
                            );
                        }
                    },
                    Err(e) => {
                        eprintln!(
                            "[NEEKO ADDON] No se pudo leer {}: {}",
                            manifest_path.display(),
                            e
                        );
                    }
                }
            }
        }

        infos.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));
        infos
    }

    pub fn enable_addon(&self, id: &str) -> Result<(), String> {
        let mut enabled = self.enabled_cache.lock().unwrap();
        if !enabled.contains(&id.to_string()) {
            enabled.push(id.to_string());
        }
        let config = AddonConfig {
            enabled_addons: enabled.clone(),
        };
        save_addon_config(&config)
    }

    pub fn disable_addon(&self, id: &str) -> Result<(), String> {
        let mut enabled = self.enabled_cache.lock().unwrap();
        enabled.retain(|a| a != id);
        let config = AddonConfig {
            enabled_addons: enabled.clone(),
        };
        save_addon_config(&config)
    }

    pub fn is_enabled(&self, id: &str) -> bool {
        self.enabled_cache.lock().unwrap().contains(&id.to_string())
    }

    pub fn get_all_js(&self) -> String {
        let mut combined = String::new();
        for addon in self.scan_addons() {
            if !addon.enabled || !addon.has_js {
                continue;
            }
            let Some(js_path) = self.find_addon_file(&addon.manifest.id, "main.js") else {
                continue;
            };
            if let Ok(js) = std::fs::read_to_string(&js_path) {
                combined.push_str(&format!(
                    "\n// === Addon: {} v{} ===\ntry {{ window.__NEEKO_ADDON_ACTIVE__ = window.__NEEKO_ADDON_ACTIVE__ || []; window.__NEEKO_ADDON_ACTIVE__.push('{}');\n{}\n}} catch(e) {{ console.error('[NEEKO ADDON Error: {}]', e); }}\n",
                    addon.manifest.name, addon.manifest.version, addon.manifest.id, js, addon.manifest.id
                ));
            }
        }
        combined
    }

    pub fn get_all_css(&self) -> String {
        let mut combined = String::new();
        for addon in self.scan_addons() {
            if !addon.enabled || !addon.has_css {
                continue;
            }
            let Some(css_path) = self.find_addon_file(&addon.manifest.id, "styles.css") else {
                continue;
            };
            if let Ok(css) = std::fs::read_to_string(&css_path) {
                combined.push_str(&format!(
                    "\n/* === Addon: {} v{} === */\n{}\n",
                    addon.manifest.name, addon.manifest.version, css
                ));
            }
        }
        combined
    }

    pub fn get_addon_js(&self, id: &str) -> Option<String> {
        self.find_addon_file(id, "main.js")
            .and_then(|path| std::fs::read_to_string(path).ok())
    }

    pub fn get_addon_css(&self, id: &str) -> Option<String> {
        self.find_addon_file(id, "styles.css")
            .and_then(|path| std::fs::read_to_string(path).ok())
    }

    fn find_addon_file(&self, id: &str, file_name: &str) -> Option<PathBuf> {
        self.addon_dirs
            .iter()
            .map(|dir| dir.join(id).join(file_name))
            .find(|path| path.exists())
    }
}
