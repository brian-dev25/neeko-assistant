//! Screen Translate: one native runtime shared by main/settings, private stdio
//! Local OCR worker and Google Web text translation. No screenshots are written to disk.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::watch,
    task::JoinHandle,
};

const ADDON: &str = "screen-translate";
const EVENT: &str = "screen-translate:status";
const OVERLAY: &str = "screen-translate-overlay";
const REMOTE: &str = "screen-translate-remote";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Region {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}
impl Region {
    fn validate(&self) -> Result<(), String> {
        if !(-65536..=65536).contains(&self.x)
            || !(-65536..=65536).contains(&self.y)
            || !(20..=4096).contains(&self.width)
            || !(20..=4096).contains(&self.height)
            || u64::from(self.width) * u64::from(self.height) > 8_000_000
        {
            return Err(
                "Región inválida: entre 20 y 4096 píxeles por lado y hasta 8 megapíxeles".into(),
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    region: Option<Region>,
    source: String,
    translation_source: String,
    tesseract_english_filter: bool,
    target: String,
    interval_ms: u64,
    font_size: u32,
    opacity: u32,
    show_original: bool,
    engine: String,
    mode: String,
    regions: Vec<Region>,
    exclusions: Vec<Region>,
    scale: f64,
    threshold: u32,
    invert: bool,
    follow_mouse: bool,
    window_id: String,
    window_x: i32,
    window_y: i32,
    retain_ms: u64,
    stabilize_ms: u64,
    text_color: String,
    background_color: String,
    outline_color: String,
    font_family: String,
    alignment: String,
    corrections: Vec<Rule>,
    glossary: Vec<Rule>,
    provider: String,
    input_mode: String,
    overlay_bounds: Option<Region>,
    brightness: i32,
    contrast: i32,
    grayscale: bool,
    vertical_text: bool,
    auto_font_size: bool,
    tts_enabled: bool,
    tts_rate: i32,
    language_patch: bool,
    region_mouse_follow: bool,
    color_filter: String,
    filter_color: String,
    color_tolerance: u32,
    erode: bool,
    merge_paragraphs: bool,
    join_lines: bool,
    batch_translation: bool,
    persistent_cache: bool,
    whole_words: bool,
    min_font_size: u32,
    max_font_size: u32,
    auto_colors: bool,
    expand_blocks: bool,

}
impl Default for Settings {
    fn default() -> Self {
        Self {
            region: None,
            source: "eng_standard".into(),
            translation_source: String::new(),
            tesseract_english_filter: true,
            target: "es".into(),
            interval_ms: 1000,
            font_size: 20,
            opacity: 67,
            show_original: true,
            engine: "tesseract".into(),
            mode: "layer".into(),
            regions: vec![],
            exclusions: vec![],
            scale: 2.,
            threshold: 0,
            invert: false,
            follow_mouse: false,
            window_id: String::new(),
            window_x: 0,
            window_y: 0,
            retain_ms: 0,
            stabilize_ms: 0,
            text_color: "#ffffff".into(),
            background_color: "#000000".into(),
            outline_color: "#000000".into(),
            font_family: "Malgun Gothic".into(),
            alignment: "left".into(),
            corrections: vec![],
            glossary: vec![],
            provider: "google".into(),
            input_mode: "screen".into(),
            overlay_bounds: None,
            brightness: 0,
            contrast: 0,
            grayscale: false,
            vertical_text: false,
            auto_font_size: false,
            tts_enabled: false,
            tts_rate: 0,
            language_patch: false,
            region_mouse_follow: false,
            color_filter: "none".into(),
            filter_color: "#ffffff".into(),
            color_tolerance: 20,
            erode: false,
            merge_paragraphs: true,
            join_lines: true,
            batch_translation: true,
            persistent_cache: false,
            whole_words: false,
            min_font_size: 10,
            max_font_size: 48,
            auto_colors: false,
            expand_blocks: true,

        }
    }
}
impl Settings {
    fn validate(&self) -> Result<(), String> {
        if !["none", "rgb", "hsv"].contains(&self.color_filter.as_str()) || self.color_tolerance > 100
            || self.filter_color.len() != 7 || !self.filter_color.starts_with('#')
            || !self.filter_color[1..].bytes().all(|c| c.is_ascii_hexdigit())
            || !(8..=48).contains(&self.min_font_size) || !(8..=72).contains(&self.max_font_size)
            || self.min_font_size > self.max_font_size {
            return Err("Filtro de color o l?mites de fuente inv?lidos".into());
        }
        if let Some(region) = &self.region {
            region.validate()?;
        }
        if (!self.translation_source.is_empty() && !valid_language(&self.translation_source))
            || self.source.len() > 40
            || !self
                .source
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            || !valid_language(&self.target)
            || !(500..=10000).contains(&self.interval_ms)
            || !(14..=48).contains(&self.font_size)
            || self.opacity > 100
        {
            return Err("Configuración de traducción inválida".into());
        }
        if !["windows", "tesseract", "oneocr"].contains(&self.engine.as_str())
            || !["dark", "layer", "over"].contains(&self.mode.as_str())
            || !["google", "papago", "db"].contains(&self.provider.as_str())
            || self.regions.len() > 8
            || self.exclusions.len() > 20
            || !self.scale.is_finite()
            || !(1.0..=3.0).contains(&self.scale)
            || self.threshold > 255
            || self.retain_ms > 10000
            || self.stabilize_ms > 2000
            || self.window_id.len() > 20
            || !self.window_id.bytes().all(|c| c.is_ascii_digit())
            || self.font_family.len() > 80
            || !["left", "center", "right"].contains(&self.alignment.as_str())
            || [
                &self.text_color,
                &self.background_color,
                &self.outline_color,
            ]
            .iter()
            .any(|s| {
                s.len() != 7
                    || !s.starts_with('#')
                    || !s[1..].bytes().all(|b| b.is_ascii_hexdigit())
            })
            || self.corrections.len() > 5000
            || self.glossary.len() > 5000
            || self
                .corrections
                .iter()
                .chain(self.glossary.iter())
                .any(|r| r.from.is_empty() || r.from.len() > 1000 || r.to.len() > 2000)
        {
            return Err("Opciones de OCR u overlay inválidas".into());
        }
        for region in self.regions.iter().chain(self.exclusions.iter()) {
            region.validate()?;
        }
        if !["screen","clipboard"].contains(&self.input_mode.as_str()) || (self.input_mode=="clipboard" && self.mode=="over") {
            return Err("El portapapeles admite texto flotante o panel de resultados".into());
        }
        if let Some(bounds)=&self.overlay_bounds {bounds.validate()?;}
        if !(-100..=100).contains(&self.brightness)
            || !(-100..=100).contains(&self.contrast)
            || !(-10..=10).contains(&self.tts_rate)
        {
            return Err("Valores de imagen o TTS fuera de rango".into());
        }
        Ok(())
    }
}
fn valid_language(code: &str) -> bool {
    !code.is_empty()
        && code.len() <= 20
        && code.bytes().all(|c| c.is_ascii_alphabetic() || c == b'-')
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Rule {
    from: String,
    to: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Profile {
    name: String,
    settings: Settings,
}
#[derive(Clone, Default, Serialize, Deserialize)]
struct Config {
    settings: Settings,
    profiles: Vec<Profile>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Language {
    code: String,
    name: String,
    #[serde(default = "windows_engine")]
    engine: String,
    #[serde(default = "yes")]
    installed: bool,
    #[serde(default, rename = "translationCode")]
    translation_code: String,
    #[serde(default)]
    size: u64,
}
fn windows_engine() -> String {
    "windows".into()
}
fn yes() -> bool {
    true
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Block {
    text: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    #[serde(default)]
    translation: String,
    #[serde(default)]
    background: Option<String>,
    #[serde(default)]
    foreground: Option<String>,
}
#[derive(Clone, Debug, Deserialize)]
struct Frame {
    text: String,
    #[serde(default)]
    blocks: Vec<Block>,
    width: u32,
    height: u32,
    region: Region,
    #[serde(default)]
    unchanged: bool,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    phase: String,
    message: String,
    active: bool,
    prepared: bool,
    settings: Settings,
    profiles: Vec<Profile>,
    languages: Vec<Language>,
    original: String,
    translation: String,
    revision: u64,
    windows: Vec<Value>,
    blocks: Vec<Block>,
    overlay_region: Option<Region>,
    locked: bool,
    paused: bool,
    cache_hits: u64,
    requests: u64,
    #[serde(skip)]
    generation: u64,
}
impl Default for Status {
    fn default() -> Self {
        let config = load_config();
        Self {
            phase: "idle".into(),
            message: "Comprobá el motor OCR para comenzar.".into(),
            active: false,
            prepared: false,
            settings: config.settings,
            profiles: config.profiles,
            languages: vec![],
            original: String::new(),
            translation: String::new(),
            generation: 0,
            revision: 0,
            windows: vec![],
            blocks: vec![],
            overlay_region: None,
            locked: false,
            paused: false,
            cache_hits: 0,
            requests: 0,
        }
    }
}
static STATE: OnceLock<Mutex<Status>> = OnceLock::new();
static CONTROL: tokio::sync::Mutex<Option<Job>> = tokio::sync::Mutex::const_new(None);
struct Job {
    cancel: watch::Sender<bool>,
    task: JoinHandle<()>,
}
fn state() -> &'static Mutex<Status> {
    STATE.get_or_init(|| Mutex::new(Status::default()))
}
fn snapshot() -> Status {
    state().lock().unwrap_or_else(|e| e.into_inner()).clone()
}
fn accepts_generation(status: &Status, generation: Option<u64>) -> bool {
    generation.is_none_or(|id| id == status.generation)
}
fn update(app: &AppHandle, generation: Option<u64>, change: impl FnOnce(&mut Status)) {
    let value = {
        let mut status = state().lock().unwrap_or_else(|e| e.into_inner());
        if !accepts_generation(&status, generation) {
            return;
        }
        change(&mut status);
        status.revision += 1;
        status.clone()
    };
    // Do not broadcast OCR text to chat/web or unrelated windows.
    for label in ["main", "settings", OVERLAY, REMOTE] {
        let _ = app.emit_to(label, EVENT, &value);
    }
}
fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("neeko-assistant/screen-translate.json")
}
fn load_config() -> Config {
    let value = std::fs::read(config_path())
        .ok()
        .filter(|bytes| bytes.len() <= 64 * 1024)
        .and_then(|bytes| serde_json::from_slice::<Config>(&bytes).ok())
        .unwrap_or_default();
    if value.settings.validate().is_err() || validate_profiles(&value.profiles).is_err() {
        Config::default()
    } else {
        value
    }
}
fn save_config(settings: &Settings, profiles: &[Profile]) -> Result<(), String> {
    let path = config_path();
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec_pretty(&Config {
        settings: settings.clone(),
        profiles: profiles.to_vec(),
    })
    .map_err(|e| e.to_string())?;
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&temp, &path).map_err(|e| e.to_string())
}
fn validate_profiles(profiles: &[Profile]) -> Result<(), String> {
    if profiles.len() > 20 {
        return Err("Máximo 20 perfiles".into());
    }
    let mut names = std::collections::HashSet::new();
    for p in profiles {
        if p.name.trim().is_empty()
            || p.name.chars().count() > 60
            || !names.insert(p.name.trim().to_lowercase())
        {
            return Err("Los perfiles necesitan nombres únicos de hasta 60 caracteres".into());
        }
        p.settings.validate()?;
    }
    Ok(())
}
fn enabled() -> bool {
    crate::addon_manager()
        .scan_addons()
        .iter()
        .any(|a| a.manifest.id == ADDON && a.enabled)
}
fn helper_path(app: &AppHandle) -> Result<PathBuf, String> {
    if !cfg!(windows) {
        return Err("Screen Translate requiere Windows 10 2004 o posterior, de 64 bits".into());
    }
    #[cfg(debug_assertions)]
    {
        let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("binaries/screen-translate/neeko-screen-ocr.exe");
        if dev.is_file() {
            return Ok(dev);
        }
    }
    let path = app
        .path()
        .resource_dir()
        .map_err(|e| e.to_string())?
        .join("binaries/screen-translate/neeko-screen-ocr.exe");
    if path.is_file() {
        Ok(path)
    } else {
        Err("Falta el motor OCR incluido con Neeko. Reinstalá una versión que incluya Screen Translate; en desarrollo ejecutá node scripts/build-screen-ocr.mjs.".into())
    }
}

struct Helper {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}
impl Helper {
    fn spawn(path: &PathBuf, mode: &str) -> Result<Self, String> {
        let mut command = Command::new(path);
        command
            .arg(mode)
            .current_dir(path.parent().ok_or("Ruta del motor inválida")?)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);
        #[cfg(windows)]
        command.creation_flags(0x08000000);
        let mut child = command
            .spawn()
            .map_err(|e| format!("No se pudo iniciar OCR: {e}"))?;
        let input = child.stdin.take().ok_or("OCR sin entrada")?;
        let output = BufReader::new(child.stdout.take().ok_or("OCR sin salida")?);
        Ok(Self {
            child,
            input,
            output,
        })
    }
    async fn read(&mut self) -> Result<Value, String> {
        let mut line = String::new();
        self.output
            .read_line(&mut line)
            .await
            .map_err(|e| e.to_string())?;
        if line.is_empty() {
            return Err("El motor OCR se cerró inesperadamente".into());
        }
        if line.len() > 65536 {
            return Err("Respuesta OCR demasiado grande".into());
        }
        let value: Value = serde_json::from_str(line.trim_start_matches('\u{feff}'))
            .map_err(|_| "El motor OCR devolvió una respuesta inválida")?;
        if let Some(error) = value.get("error").and_then(Value::as_str) {
            return Err(error.to_owned());
        }
        Ok(value)
    }
    async fn capture(&mut self, settings: &Settings) -> Result<Frame, String> {
        let line = format!(
            "{}\n",
            json!({"region": settings.region, "source": settings.source, "engine": settings.engine,
                "scale":settings.scale,"threshold":settings.threshold,"invert":settings.invert,
                "windowId":settings.window_id,"windowX":settings.window_x,"windowY":settings.window_y,
                "followMouse":settings.follow_mouse,"exclusions":settings.exclusions,"clipboardInput":settings.input_mode=="clipboard",
                "brightness":settings.brightness,"contrast":settings.contrast,"grayscale":settings.grayscale,
                "tesseractEnglishFilter":settings.tesseract_english_filter,
                "verticalText":settings.vertical_text, "autoColors":settings.auto_colors,
                "colorFilter":settings.color_filter,"filterColor":settings.filter_color,
                "colorTolerance":settings.color_tolerance,"erode":settings.erode})
        );
        self.input
            .write_all(line.as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        self.input.flush().await.map_err(|e| e.to_string())?;
        let value = tokio::time::timeout(Duration::from_secs(30), self.read())
            .await
            .map_err(|_| "OCR agotó el tiempo de espera")??;
        let frame: Frame = serde_json::from_value(value).map_err(|_| "Respuesta OCR inválida")?;
        if frame.text.chars().count() > 6000
            || frame.blocks.len() > 100
            || frame.width == 0
            || frame.height == 0
        {
            return Err("Seleccioná una región con menos texto".into());
        }
        for block in &frame.blocks {
            if ![block.x, block.y, block.width, block.height]
                .iter()
                .all(|v| v.is_finite())
                || block.x < 0.
                || block.y < 0.
                || block.width <= 0.
                || block.height <= 0.
                || block.x + block.width > f64::from(frame.width) + 1.
                || block.y + block.height > f64::from(frame.height) + 1.
            {
                return Err("Coordenadas OCR inválidas".into());
            }
        }
        Ok(frame)
    }
    async fn close(&mut self) {
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
    }
}

fn ensure_overlay(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(OVERLAY) {
        return window.show().map_err(|e| e.to_string());
    }
    WebviewWindowBuilder::new(
        app,
        OVERLAY,
        WebviewUrl::App("screen-translate-overlay.html".into()),
    )
    .title("Neeko · Traducción en pantalla")
    .inner_size(660., 240.)
    .min_inner_size(300., 130.)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(true)
    .focused(false)
    .transparent(true)
    .shadow(false)
    .content_protected(true)
    .build()
    .map_err(|e| e.to_string())?;
    if let (Some(window),Some(bounds))=(app.get_webview_window(OVERLAY),snapshot().settings.overlay_bounds) {
        if snapshot().settings.mode!="over" {
            let _=window.set_position(tauri::PhysicalPosition::new(bounds.x,bounds.y));
            let _=window.set_size(tauri::PhysicalSize::new(bounds.width,bounds.height));
        }
    }
    Ok(())
}

fn ensure_remote(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(REMOTE) {
        return window.show().map_err(|e| e.to_string());
    }
    WebviewWindowBuilder::new(
        app,
        REMOTE,
        WebviewUrl::App("remote-controller.html".into()),
    )
    .title("Neeko · Control")
    .inner_size(220., 40.)
    .min_inner_size(180., 36.)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .focused(false)
    .transparent(true)
    .shadow(false)
    .build()
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn hide_remote(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(REMOTE) {
        let _ = window.hide();
    }
}
fn hide_overlay(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(OVERLAY) {
        let _ = window.hide();
    }
}

async fn cancel_job(slot: &mut Option<Job>) {
    if let Some(mut job) = slot.take() {
        let _ = job.cancel.send(true);
        if tokio::time::timeout(Duration::from_secs(3), &mut job.task)
            .await
            .is_err()
        {
            job.task.abort();
            let _ = job.task.await;
        }
    }
}
fn next_generation(app: &AppHandle) -> u64 {
    update(app, None, |s| {
        s.generation += 1;
        s.active = false;
    });
    snapshot().generation
}
pub async fn stop(app: &AppHandle) {
    let mut slot = CONTROL.lock().await;
    next_generation(app);
    cancel_job(&mut slot).await;
    if snapshot().settings.mode!="over" {
        if let Some(window)=app.get_webview_window(OVERLAY) {
            if let (Ok(position),Ok(size))=(window.inner_position(),window.inner_size()) {
                let bounds=Region{x:position.x,y:position.y,width:size.width,height:size.height};
                if bounds.validate().is_ok() {update(app,None,|s|{s.settings.overlay_bounds=Some(bounds);let _=save_config(&s.settings,&s.profiles);});}
            }
        }
    }
    let _ = pipeline::lock(app, false);
    hide_overlay(app);
    update(app, None, |s| {
        s.phase = "idle".into();
        s.message = "Detenido. No se está capturando la pantalla.".into();
    });
}

fn normalized(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
mod google;
mod papago;
mod pipeline;
mod stability;
mod files;

async fn run_capture(
    app: AppHandle,
    generation: u64,
    path: PathBuf,
    settings: Settings,
    once: bool,
    mut cancel: watch::Receiver<bool>,
) {
    let mut helper = match Helper::spawn(&path, "--serve") {
        Ok(h) => h,
        Err(e) => {
            fail(&app, generation, e);
            return;
        }
    };
    let result = tokio::select! {
        _ = cancel.changed() => Ok(()),
        result = pipeline::run(&app, generation, &mut helper, &settings, once) => result,
    };
    helper.close().await;
    if let Err(error) = result {
        fail(&app, generation, error);
    } else {
        update(&app, Some(generation), |s| {
            s.active = false;
            s.phase = "idle".into();
        });
    }
}
fn fail(app: &AppHandle, generation: u64, error: String) {
    update(app, Some(generation), |s| {
        s.active = false;
        s.phase = "error".into();
        s.message = error;
    });
}
async fn run_utility(
    app: AppHandle,
    generation: u64,
    path: PathBuf,
    mode: &'static str,
    mut cancel: watch::Receiver<bool>,
) {
    let native_mode = if mode.starts_with("--select") {
        "--select"
    } else {
        mode
    };
    let mut helper = match Helper::spawn(&path, native_mode) {
        Ok(h) => h,
        Err(e) => {
            fail(&app, generation, e);
            return;
        }
    };
    if mode == "--install" {
        let request = format!("{}\n", json!({"source":snapshot().settings.source}));
        if helper.input.write_all(request.as_bytes()).await.is_err() {
            fail(&app, generation, "No se pudo preparar la descarga".into());
            helper.close().await;
            return;
        }
        let _ = helper.input.flush().await;
    }
    let timeout = if mode.starts_with("--select") || mode == "--install" {
        240
    } else {
        30
    };
    let result = tokio::select! {
        _ = cancel.changed() => None,
        result = tokio::time::timeout(Duration::from_secs(timeout), helper.read()) => Some(result.map_err(|_| "Se agotó el tiempo de espera del motor OCR".to_string()).and_then(|r| r)),
    };
    helper.close().await;
    match result {
        None => (),
        Some(Err(error)) => fail(&app, generation, error),
        Some(Ok(value)) => {
            if mode == "--languages" || mode == "--install" {
                let languages = serde_json::from_value::<Vec<Language>>(value["languages"].clone());
                match languages {
                    Ok(languages) => update(&app, Some(generation), |s| {
                        s.prepared = true;
                        s.phase = "idle".into();
                        s.message = if languages.is_empty() { "No hay idiomas OCR instalados. Abrí Idioma y región de Windows, agregá OCR y comprobá otra vez." } else { "Motor OCR listo. Seleccioná un idioma y una región." }.into();
                        if !languages
                            .iter()
                            .any(|l| l.code == s.settings.source && l.engine == s.settings.engine)
                        {
                            s.settings.source = languages
                                .iter()
                                .find(|l| l.engine == s.settings.engine)
                                .map(|l| l.code.clone())
                                .unwrap_or_default();
                        }
                        s.languages = languages;
                        s.windows = value["windows"].as_array().cloned().unwrap_or_default();
                    }),
                    Err(_) => fail(
                        &app,
                        generation,
                        "El motor no devolvió una lista de idiomas válida".into(),
                    ),
                }
            } else if value["cancelled"] == true {
                update(&app, Some(generation), |s| {
                    s.phase = "idle".into();
                    s.message = "Selección cancelada; se conserva la región anterior.".into();
                });
            } else {
                let region = serde_json::from_value::<Region>(value["region"].clone())
                    .map_err(|e| e.to_string())
                    .and_then(|r| {
                        r.validate()?;
                        Ok(r)
                    });
                match region {
                    Ok(region) => update(&app, Some(generation), |s| {
                        if mode == "--select-add" {
                            if s.settings.regions.len() < 8 {
                                s.settings.regions.push(region);
                            }
                        } else if mode == "--select-exclude" {
                            if s.settings.exclusions.len() < 20 {
                                s.settings.exclusions.push(region);
                            }
                        } else {
                            s.settings.region = Some(region);
                        }
                        s.phase = "idle".into();
                        s.original.clear();
                        s.translation.clear();
                        s.message = match save_config(&s.settings, &s.profiles) {
                            Ok(()) => {
                                "Región guardada. Pulsá Traducir una vez o Iniciar continuo.".into()
                            }
                            Err(e) => format!("Región seleccionada; no se pudo guardar: {e}"),
                        };
                    }),
                    Err(e) => fail(&app, generation, e),
                }
            }
        }
    }
}

#[tauri::command]
pub async fn screen_translate(
    app: AppHandle,
    window: WebviewWindow,
    action: String,
    settings: Option<Settings>,
    profiles: Option<Vec<Profile>>,
) -> Result<Status, String> {
    let overlay = window.label() == OVERLAY;
    let remote = window.label() == REMOTE;
    if !matches!(
        window.label(),
        "main" | "settings" | "screen-translate-overlay" | "screen-translate-remote"
    ) {
        return Err("Ventana no autorizada".into());
    }
    if overlay
        && !matches!(
            action.as_str(),
            "status" | "stop" | "pause" | "lock" | "hide"
        )
    {
        return Err("El overlay solo puede consultar y detener su sesión".into());
    }
    if remote
        && !matches!(
            action.as_str(),
            "status" | "stop" | "pause" | "lock" | "hide" | "start" | "once" | "toggle-overlay"
        )
    {
        return Err("El control remoto solo puede iniciar, pausar, bloquear y detener".into());
    }
    if action == "status" {
        return Ok(snapshot());
    }
    if action == "stop" {
        stop(&app).await;
        return Ok(snapshot());
    }
    let mut slot = CONTROL.lock().await;
    if !enabled() {
        return Err("Activá Screen Translate en Addons primero".into());
    }
    pipeline::ensure_hotkeys(&app);
    match action.as_str() {
        "clear-cache" => {
            if snapshot().active {return Err("Detené la sesión antes de borrar la caché.".into());}
            pipeline::clear_cache()?;
            update(&app,None,|s|s.message="Caché de traducciones borrada.".into());
        }
        "toggle-overlay" => {
            if app.get_webview_window(OVERLAY).is_some_and(|w|w.is_visible().unwrap_or(false)) {hide_overlay(&app);} else {ensure_overlay(&app)?;}
        }
        "toggle-remote" => {
            if app.get_webview_window(REMOTE).is_some_and(|w|w.is_visible().unwrap_or(false)) {hide_remote(&app);} else {ensure_remote(&app)?;}
        }
        "export" | "import" => {
            if snapshot().active || matches!(snapshot().phase.as_str(), "selecting" | "preparing") {
                return Err("Detene la sesion antes de importar o exportar.".into());
            }
            if action == "export" { files::export(&app).await?; }
            else { files::import(&app).await?; }
        }
        "pause" => {
            update(&app, None, |s| {
                s.paused = !s.paused;
                s.message = if s.paused { "Pausado" } else { "Reanudando" }.into();
            });
        }
        "lock" => {
            pipeline::lock(&app, !snapshot().locked)?;
        }
        "hide" => {
            hide_overlay(&app);
        }
        "defaults" | "settings" => {
            if snapshot().active || matches!(snapshot().phase.as_str(), "selecting" | "preparing") {
                return Err("Detené la sesión antes de cambiar su configuración".into());
            }
            let settings = if action == "defaults" {
                let current = snapshot().settings;
                Settings { region: current.region, regions: current.regions,
                    exclusions: current.exclusions, target: current.target,
                    window_id: current.window_id, window_x: current.window_x, window_y: current.window_y,
                    corrections: current.corrections, glossary: current.glossary,
                    ..Settings::default() }
            } else { settings.ok_or("Falta configuración")? };
            settings.validate()?;
            let profiles = profiles.unwrap_or_else(|| snapshot().profiles);
            validate_profiles(&profiles)?;
            save_config(&settings, &profiles)?;
            update(&app, None, |s| {
                s.settings = settings;
                s.profiles = profiles;
                s.message = if action == "defaults" {
                    "Valores de MORT 1.291 aplicados. Se conservaron destino, áreas y diccionario. Comprobá Tesseract e instalá inglés si falta."
                } else { "Configuración guardada." }.into();
            });
        }
        "clear" => {
            update(&app, None, |s| {
                s.original.clear();
                s.translation.clear();
                s.blocks.clear();
            });
        }
        "overlay" => {
            ensure_overlay(&app)?;
        }
        "language-settings" => {
            open::that("ms-settings:regionlanguage").map_err(|e| e.to_string())?;
        }
        "prepare" | "install" | "select" | "select-add" | "select-exclude" | "start" | "once" => {
            if snapshot().active || matches!(snapshot().phase.as_str(), "selecting" | "preparing") {
                return Err("Ya hay una operación en curso. Detenela primero.".into());
            }
            let path = helper_path(&app)?;
            let settings = snapshot().settings;
            if matches!(action.as_str(), "start" | "once") {
                settings.validate()?;
                if settings.input_mode == "screen" && settings.region.is_none() {
                    return Err("Seleccioná una región primero".into());
                }
                if settings.input_mode == "screen" && (!snapshot().prepared
                    || !snapshot().languages.iter().any(|l| {
                        l.code == settings.source && l.engine == settings.engine && l.installed
                    }))
                {
                    return Err("Comprobá OCR y seleccioná un idioma instalado".into());
                }
                ensure_overlay(&app)?;
            }
            if action == "install" && settings.engine != "tesseract" {
                return Err("La descarga integrada corresponde a Tesseract".into());
            }
            let generation = next_generation(&app);
            cancel_job(&mut slot).await;
            let (cancel, receiver) = watch::channel(false);
            let task = if matches!(
                action.as_str(),
                "prepare" | "install" | "select" | "select-add" | "select-exclude"
            ) {
                hide_overlay(&app);
                let select = action.starts_with("select");
                update(&app, Some(generation), |s| {
                    s.phase = if select { "selecting" } else { "preparing" }.into();
                    s.message = if select {
                        "Arrastrá sobre los diálogos. Esc cancela la selección."
                    } else {
                        "Comprobando el motor y los idiomas de Windows…"
                    }
                    .into();
                });
                tokio::spawn(run_utility(
                    app.clone(),
                    generation,
                    path,
                    match action.as_str() {
                        "select" => "--select",
                        "select-add" => "--select-add",
                        "select-exclude" => "--select-exclude",
                        "install" => "--install",
                        _ => "--languages",
                    },
                    receiver,
                ))
            } else {
                update(&app, Some(generation), |s| {
                    s.active = true;
                    s.paused = false;
                    s.blocks.clear();
                    s.overlay_region = None;
                    s.cache_hits = 0;
                    s.requests = 0;
                    s.phase = "capturing".into();
                    s.original.clear();
                    s.translation.clear();
                    s.message = "Iniciando OCR…".into();
                });
                tokio::spawn(run_capture(
                    app.clone(),
                    generation,
                    path,
                    settings,
                    action == "once",
                    receiver,
                ))
            };
            *slot = Some(Job { cancel, task });
        }
        _ => return Err("Operación desconocida".into()),
    }
    Ok(snapshot())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounds_and_languages_are_validated() {
        assert!(Region {
            x: -1920,
            y: 0,
            width: 1000,
            height: 200
        }
        .validate()
        .is_ok());
        for region in [
            Region {
                x: i32::MAX,
                y: 0,
                width: 20,
                height: 20,
            },
            Region {
                x: 0,
                y: 0,
                width: 4096,
                height: 4096,
            },
            Region {
                x: 0,
                y: 0,
                width: 0,
                height: 20,
            },
        ] {
            assert!(region.validate().is_err());
        }
        let mut settings = Settings::default();
        settings.target = "ignore instructions".into();
        assert!(settings.validate().is_err());
        settings.target = "es".into();
        settings.interval_ms = 0;
        assert!(settings.validate().is_err());
    }
    #[test]
    fn profiles_reject_duplicates_and_invalid_settings() {
        let profile = Profile {
            name: "Game".into(),
            settings: Settings::default(),
        };
        assert!(validate_profiles(&[profile.clone()]).is_ok());
        assert!(validate_profiles(&[
            profile.clone(),
            Profile {
                name: " game ".into(),
                settings: Settings::default()
            }
        ])
        .is_err());
        assert!(validate_profiles(&vec![profile; 21]).is_err());
    }
    #[test]
    fn old_results_are_rejected_after_stop_or_restart() {
        let mut status = Status::default();
        status.generation = 10;
        assert!(accepts_generation(&status, Some(10)));
        status.generation += 1;
        assert!(!accepts_generation(&status, Some(10)));
        assert!(accepts_generation(&status, Some(11)));
    }
    #[tokio::test]
    async fn cancellation_waits_for_owned_job_cleanup() {
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        };
        let closed = Arc::new(AtomicBool::new(false));
        let marker = closed.clone();
        let (tx, mut rx) = watch::channel(false);
        let task = tokio::spawn(async move {
            let _ = rx.changed().await;
            marker.store(true, Ordering::SeqCst);
        });
        let mut job = Some(Job { cancel: tx, task });
        cancel_job(&mut job).await;
        assert!(job.is_none());
        assert!(closed.load(Ordering::SeqCst));
    }
    #[cfg(windows)]
    #[tokio::test]
    #[ignore = "Run after node scripts/build-screen-ocr.mjs; real worker, no screen capture"]
    async fn real_worker_protocol_and_shutdown() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("binaries/screen-translate/neeko-screen-ocr.exe");
        assert!(path.is_file(), "Build the OCR worker first");
        let mut helper = Helper::spawn(&path, "--languages").unwrap();
        let value = tokio::time::timeout(Duration::from_secs(30), helper.read())
            .await
            .unwrap()
            .unwrap();
        assert!(value["languages"].is_array());
        helper.close().await;
        assert!(helper.child.try_wait().unwrap().is_some());
        let mut helper = Helper::spawn(&path, "--serve").unwrap();
        // Invalid geometry is rejected before CopyFromScreen, so no user data is read.
        let invalid = Settings {
            region: Some(Region {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            }),
            source: "en-US".into(),
            ..Settings::default()
        };
        assert!(helper
            .capture(&invalid)
            .await
            .unwrap_err()
            .contains("región"));
        helper.close().await;
        assert!(helper.child.try_wait().unwrap().is_some());
    }
}
