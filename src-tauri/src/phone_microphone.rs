//! Native addon adapter. The audio/transport implementation lives in vendor/micyou.
use cpal::traits::{DeviceTrait, HostTrait};
use micyou_audio::AudioDspSettings;
use neeko_micyou_core::{events::{AecStatus, ServerEvents, SharedEvents}, server::ServerState,
    stats::AudioMetrics, tcp_server::DeviceInfo};
use serde::{Deserialize, Serialize};
use std::{net::Ipv4Addr, path::PathBuf, sync::{Arc, Mutex, OnceLock, atomic::{AtomicBool, Ordering}}};
use tauri::{Emitter, Manager};

#[derive(Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub mode: String,
    pub bind_address: String,
    pub port: u16,
    pub output_device: String,
    pub usb_serial: String,
    pub gain: f32,
    pub noise_suppression: bool,
    pub echo_cancellation: bool,
    pub automatic_gain: bool,
    pub auto_reconnect: bool,
    pub muted: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self { mode: "wifi".into(), bind_address: String::new(), port: 9123,
            output_device: String::new(), usb_serial: String::new(), gain: 0.,
            noise_suppression: false, echo_cancellation: false, automatic_gain: false,
            auto_reconnect: true, muted: false }
    }
}
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Status {
    running: bool,
    connected: bool,
    device: Option<DeviceInfo>,
    level: u32,
    metrics: Option<AudioMetrics>,
    message: String,
    aec: Option<AecStatus>,
    settings: Settings,
}
struct Events {
    app: tauri::AppHandle,
    status: Arc<Mutex<Status>>,
    allowed: Arc<AtomicBool>,
    remembered_ip: Mutex<Option<String>>,
}
impl Events {
    fn update(&self, change: impl FnOnce(&mut Status)) {
        let snapshot = {
            let mut status = self.status.lock().unwrap_or_else(|p| p.into_inner());
            change(&mut status);
            status.clone()
        };
        let _ = self.app.emit("phone-microphone:status", &snapshot);
    }
}
impl ServerEvents for Events {
    fn device_connected(&self, info: DeviceInfo) {
        self.update(|s| {
            let approved = s.settings.auto_reconnect
                && self.remembered_ip.lock().unwrap().as_deref() == Some(&info.ip);
            self.allowed.store(approved, Ordering::Release);
            s.connected = approved;
            s.device = Some(info);
            s.message = if approved { "Teléfono reconectado" } else { "Teléfono detectado. Pulsá Conectar para enviar audio al micrófono virtual." }.into();
        });
    }
    fn device_disconnected(&self) {
        self.allowed.store(false, Ordering::Release);
        self.update(|s| { s.connected = false; s.device = None; s.level = 0;
            s.message = "Teléfono desconectado. Esperando conexión de MicYou…".into(); });
    }
    fn audio_metrics(&self, metrics: AudioMetrics) { self.update(|s| s.metrics = Some(metrics)); }
    fn udp_audio_warning(&self) { self.update(|s| s.message = "No llega audio UDP. Revisá el firewall o seleccioná TCP en Android.".into()); }
    fn mute_state_changed(&self, muted: bool) {
        self.update(|s| { if muted { s.level = 0; s.message = "El teléfono silenció el audio".into(); }
            else { s.message = "Mute del teléfono desactivado".into(); } });
    }
    fn audio_level(&self, level: u32) {
        self.update(|s| s.level = if s.connected && !s.settings.muted { level } else { 0 });
    }
    fn audio_spectrum(&self, _: Vec<f32>, _: Vec<f32>) {}
    fn server_stopped(&self) { self.update(|s| { s.running = false; s.connected = false; s.device = None; s.level = 0; }); }
    fn web_client_count(&self, _: u32) {}
    fn install_progress(&self, message: String) { self.update(|s| s.message = message); }
    fn aec_status_changed(&self, status: AecStatus) { self.update(|s| s.aec = Some(status)); }
}
struct Runtime {
    core: ServerState,
    events: Arc<Events>,
    usb: Option<(String, u16)>,
}
static RUNTIME: OnceLock<tokio::sync::Mutex<Option<Runtime>>> = OnceLock::new();
fn runtime() -> &'static tokio::sync::Mutex<Option<Runtime>> { RUNTIME.get_or_init(|| tokio::sync::Mutex::new(None)) }
fn config_path() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from("."))
        .join("neeko-assistant/phone-microphone.json")
}
fn load_settings() -> Settings {
    std::fs::read(config_path()).ok().and_then(|s| serde_json::from_slice(&s).ok()).unwrap_or_default()
}
fn save_settings(settings: &Settings) -> Result<(), String> {
    let path = config_path();
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(path, serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}
fn local_ip(value: &str) -> bool {
    value.parse::<Ipv4Addr>().map(|ip| ip.is_private() || ip.is_loopback() || ip.is_link_local()).unwrap_or(false)
}
fn virtual_device(name: &str) -> bool {
    let name = name.to_lowercase();
    name.contains("cable input") || (name.contains("vb-audio") && !name.contains("output"))
}
fn outputs() -> Vec<String> {
    let mut names: Vec<_> = cpal::default_host().output_devices().into_iter().flatten()
        .filter_map(|d| d.name().ok()).filter(|n| virtual_device(n)).collect();
    names.sort(); names.dedup(); names
}
fn validate(s: &Settings) -> Result<(), String> {
    if !matches!(s.mode.as_str(), "wifi" | "usb") || !(1024..=65534).contains(&s.port) {
        return Err("Modo o puerto inválido (1024–65534)".into());
    }
    if !s.bind_address.is_empty() && !local_ip(&s.bind_address) { return Err("Elegí una dirección IPv4 privada/local".into()); }
    if !s.gain.is_finite() || !(-50.0..=50.0).contains(&s.gain) { return Err("Ganancia inválida".into()); }
    Ok(())
}
fn resource_dir(app: &tauri::AppHandle) -> PathBuf {
    if cfg!(debug_assertions) {
        return PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../addons/phone-microphone/resources");
    }
    app.path().resource_dir().unwrap_or_default().join("addons/phone-microphone/resources")
}
fn aec_available(app: &tauri::AppHandle) -> bool {
    let root = resource_dir(app);
    root.join("onnxruntime.dll").is_file() && root.join("aec7_ep0185.onnx").is_file()
}
fn dsp(s: &Settings) -> AudioDspSettings {
    AudioDspSettings { gain: s.gain, ns_enabled: s.noise_suppression, ns_type: "RNNoise".into(),
        aec_enabled: s.echo_cancellation, agc_enabled: s.automatic_gain, ..Default::default() }
}

async fn stop_locked(slot: &mut Option<Runtime>) -> Result<(), String> {
    if let Some(rt) = slot.as_ref() {
        rt.core.audio_allowed.store(false, Ordering::Release);
        let events: SharedEvents = rt.events.clone();
        // Keep ownership if upstream reports residual cleanup, preventing overlapping servers.
        neeko_micyou_core::system::stop_server_inner(&rt.core, events).await?;
        rt.core.audio_output.shutdown();
        if let Some((serial, port)) = &rt.usb {
            if let Err(error) = neeko_micyou_core::adb_manager::remove_reverse(serial, *port) {
                eprintln!("[NEEKO Phone Microphone] USB cleanup ({serial}): {error}");
            }
        }
    }
    *slot = None;
    Ok(())
}
pub async fn stop() -> Result<(), String> { stop_locked(&mut *runtime().lock().await).await }

#[tauri::command]
pub async fn phone_microphone(app: tauri::AppHandle, window: tauri::WebviewWindow,
    action: String, settings: Option<Settings>) -> Result<serde_json::Value, String> {
    if !matches!(window.label(), "main" | "settings") { return Err("Ventana no autorizada".into()); }
    if !crate::addon_manager().scan_addons().iter().any(|a| a.manifest.id == "phone-microphone" && a.enabled) {
        return Err("Phone Microphone está deshabilitado".into());
    }
    match action.as_str() {
        "devices" => {
            let interfaces: Vec<_> = neeko_micyou_core::server::query_network_interfaces().into_iter()
                .filter(|i| local_ip(&i.ip)).collect();
            return Ok(serde_json::json!({"outputs": outputs(), "interfaces": interfaces,
                "aecAvailable": aec_available(&app)}));
        }
        "usb-devices" => return serde_json::to_value(neeko_micyou_core::adb_manager::list_adb_devices()?).map_err(|e| e.to_string()),
        _ => {}
    }
    let mut slot = runtime().lock().await;
    match action.as_str() {
        "start" => {
            if slot.is_some() { return Err("El servidor ya está iniciado; detenelo antes de cambiar la conexión".into()); }
            if !cfg!(windows) { return Err("Este addon requiere Windows y VB-CABLE".into()); }
            let mut s = settings.unwrap_or_else(load_settings);
            validate(&s)?;
            if s.echo_cancellation && !aec_available(&app) { return Err("Faltan los recursos locales de AEC".into()); }
            if s.echo_cancellation {
                micyou_audio::init_ort_runtime(&resource_dir(&app).join("onnxruntime.dll")).map_err(|e| format!("No se pudo cargar AEC: {e}"))?;
            }
            let devices = outputs();
            if s.output_device.is_empty() { s.output_device = devices.first().cloned().unwrap_or_default(); }
            if !devices.contains(&s.output_device) { return Err("Instalá VB-CABLE y seleccioná CABLE Input. No se usará el altavoz.".into()); }
            if s.mode == "usb" { s.bind_address = "127.0.0.1".into(); }
            else if s.bind_address.is_empty() {
                s.bind_address = neeko_micyou_core::server::query_network_interfaces().into_iter()
                    .find(|i| local_ip(&i.ip) && i.ip != "127.0.0.1")
                    .ok_or("No hay una interfaz LAN privada disponible")?.ip;
            }
            save_settings(&s)?;
            let core = ServerState::default();
            *core.dsp_settings.write().unwrap() = dsp(&s);
            core.local_muted.store(s.muted, Ordering::Release);
            let events = Arc::new(Events { app: app.clone(), status: Arc::new(Mutex::new(Status {
                settings: s.clone(), ..Default::default() })), allowed: core.audio_allowed.clone(), remembered_ip: Mutex::new(None) });
            let mut rt = Runtime { core, events, usb: None };
            if s.mode == "usb" {
                let devices = neeko_micyou_core::adb_manager::list_adb_devices()?;
                let candidates: Vec<_> = devices.iter().filter(|d| d.state == "device"
                    && (s.usb_serial.is_empty() || d.serial == s.usb_serial)).collect();
                if candidates.len() != 1 { rt.core.audio_output.shutdown(); return Err("Seleccioná un único teléfono USB autorizado".into()); }
                let serial = candidates[0].serial.clone();
                if let Err(e) = neeko_micyou_core::adb_manager::enable_usb_mode(s.port, Some(&serial)) {
                    rt.core.audio_output.shutdown(); return Err(e);
                }
                rt.usb = Some((serial, s.port));
            }
            let sink: SharedEvents = rt.events.clone();
            if let Err(e) = neeko_micyou_core::system::start_server_inner(&rt.core, s.port,
                s.mode.clone(), Some(s.bind_address.clone()), Some(s.output_device.clone()),
                Some(resource_dir(&app)), sink).await {
                rt.core.audio_output.shutdown();
                if let Some((serial, port)) = &rt.usb { let _ = neeko_micyou_core::adb_manager::remove_reverse(serial, *port); }
                return Err(e);
            }
            rt.events.update(|status| { status.running = true; status.message = "Servidor iniciado. Abrí MicYou en Android y conectalo a esta PC.".into(); });
            *slot = Some(rt);
        }
        "stop" => stop_locked(&mut slot).await?,
        "connect" => {
            let rt = slot.as_ref().ok_or("Iniciá el servidor primero")?;
            let snapshot = {
                let mut status = rt.events.status.lock().unwrap();
                let device = status.device.as_ref().ok_or("Abrí MicYou en el teléfono y conectalo a esta PC primero")?;
                *rt.events.remembered_ip.lock().unwrap() = Some(device.ip.clone());
                rt.core.audio_allowed.store(true, Ordering::Release);
                status.connected = true;
                status.message = "Audio conectado a CABLE Input. Seleccioná CABLE Output como micrófono en Discord/OBS.".into();
                status.clone()
            };
            let _ = rt.events.app.emit("phone-microphone:status", &snapshot);
        }
        "disconnect" => {
            if let Some(rt) = slot.as_ref() {
                rt.core.audio_allowed.store(false, Ordering::Release);
                *rt.events.remembered_ip.lock().unwrap() = None;
                neeko_micyou_core::tcp_server::cleanup_session_state(&rt.core.active_connection, &rt.core.active_audio_session).await;
                rt.events.device_disconnected();
            }
        }
        "settings" => {
            let s = settings.ok_or("Falta configuración")?;
            validate(&s)?;
            if s.echo_cancellation && !aec_available(&app) { return Err("Faltan los recursos locales de AEC".into()); }
            if s.echo_cancellation {
                micyou_audio::init_ort_runtime(&resource_dir(&app).join("onnxruntime.dll")).map_err(|e| format!("No se pudo cargar AEC: {e}"))?;
            }
            if let Some(rt) = slot.as_ref() {
                let old = rt.events.status.lock().unwrap().settings.clone();
                if (s.mode.as_str(), s.port, s.bind_address.as_str(), s.output_device.as_str(), s.usb_serial.as_str())
                    != (old.mode.as_str(), old.port, old.bind_address.as_str(), old.output_device.as_str(), old.usb_serial.as_str()) {
                    return Err("Detené el servidor antes de cambiar red o dispositivo de salida".into());
                }
                save_settings(&s)?;
                *rt.core.dsp_settings.write().unwrap() = dsp(&s);
                rt.core.local_muted.store(s.muted, Ordering::Release);
                rt.events.update(|status| status.settings = s);
            } else { save_settings(&s)?; }
        }
        "status" => {},
        _ => return Err("Acción desconocida".into()),
    }
    let status = if let Some(rt) = slot.as_ref() { rt.events.status.lock().unwrap().clone() }
        else { Status { settings: load_settings(), message: "Servidor detenido".into(), ..Default::default() } };
    serde_json::to_value(status).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_public_and_wildcard_binds() {
        for ip in ["0.0.0.0", "8.8.8.8", "::", "example.com"] {
            assert!(validate(&Settings { bind_address: ip.into(), ..Default::default() }).is_err());
        }
        for ip in ["127.0.0.1", "192.168.1.5", "172.16.0.2", "10.0.0.2"] { assert!(local_ip(ip)); }
    }
    #[test]
    fn restricts_devices_and_numeric_settings() {
        assert!(virtual_device("CABLE Input (VB-Audio Virtual Cable)"));
        assert!(!virtual_device("Speakers (Realtek)"));
        assert!(!virtual_device("CABLE Output (VB-Audio)"));
        assert!(validate(&Settings { gain: f32::NAN, ..Default::default() }).is_err());
        assert!(validate(&Settings { port: 65535, ..Default::default() }).is_err());
    }
}
