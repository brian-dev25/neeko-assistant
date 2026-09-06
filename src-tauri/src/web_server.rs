use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, Query, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{
        sse::{Event, KeepAlive, Sse},
        Html, IntoResponse, Response,
    },
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;
use tokio::sync::broadcast;
use tokio::sync::Mutex as AsyncMutex;
use tower_http::cors::{Any, CorsLayer};

const PORT: u16 = 1414;
const PASSWORD_CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
const WEB_USER_AGENT: &str = "Mozilla/5.0 NeekoAssistant/1.0";
static WEB_PASSWORD: OnceLock<String> = OnceLock::new();

pub(crate) fn web_password() -> &'static str {
    WEB_PASSWORD.get_or_init(generate_web_password).as_str()
}

fn generate_web_password() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0);
    let mut seed = nanos ^ ((std::process::id() as u64) << 32);

    (0..4)
        .map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            PASSWORD_CHARS[(seed as usize) % PASSWORD_CHARS.len()] as char
        })
        .collect()
}

fn files_dir() -> std::path::PathBuf {
    std::env::temp_dir().join("neeko-files")
}

#[derive(Clone)]
struct AppState {
    app_handle: AppHandle,
    client: reqwest::Client,
    file_events: broadcast::Sender<FileEvent>,
    free_games_cache: Arc<AsyncMutex<FreeGamesCache>>,
    free_games_seen: Arc<AsyncMutex<HashSet<String>>>,
    localsend_sessions: Arc<AsyncMutex<HashMap<String, LocalSendSession>>>,
}

#[derive(Clone)]
struct LocalSendFile {
    filename: String,
    token: String,
}

#[derive(Clone, Default)]
struct LocalSendSession {
    files: HashMap<String, LocalSendFile>,
}

#[derive(Deserialize)]
struct LocalSendPrepareRequest {
    files: HashMap<String, LocalSendFileMetadata>,
}

#[derive(Deserialize)]
struct LocalSendFileMetadata {
    #[serde(rename = "fileName")]
    filename: String,
}

#[derive(Serialize)]
struct LocalSendPrepareResponse {
    #[serde(rename = "sessionId")]
    session_id: String,
    files: HashMap<String, String>,
}

#[derive(Deserialize)]
struct LocalSendUploadQuery {
    #[serde(rename = "sessionId")]
    session_id: String,
    #[serde(rename = "fileId")]
    file_id: String,
    token: String,
}

#[derive(Deserialize)]
struct LocalSendCancelQuery {
    #[serde(rename = "sessionId")]
    session_id: String,
}

#[derive(Deserialize)]
struct ChatRequest {
    session: String,
    messages: Vec<crate::assistant::Message>,
}

#[derive(Deserialize)]
struct ChatDecision {
    session: String,
    proposal_id: String,
    approved: bool,
    save_account: Option<bool>,
}

#[derive(Deserialize)]
struct ChatCancel {
    session: String,
}

#[derive(Deserialize)]
struct OpenAppRequest {
    app: String,
}

#[derive(Deserialize)]
struct SearchRequest {
    query: String,
}

#[derive(Deserialize)]
struct UrlRequest {
    url: String,
}

#[derive(Deserialize)]
struct LoginRequest {
    password: String,
}

#[derive(Deserialize)]
struct InstallModelRequest {
    model_url: String,
}

#[derive(Deserialize)]
struct CancelDownloadRequest {
    id: String,
}

#[derive(Deserialize)]
struct WebConfigRequest {
    language: Option<String>,
    start_with_windows: Option<bool>,
}

#[derive(Serialize)]
struct WebConfigResponse {
    language: String,
    start_with_windows: bool,
}

#[derive(Serialize)]
struct LoginResponse {
    success: bool,
    token: String,
}

#[derive(Serialize)]
struct ApiResponse {
    ok: bool,
    message: String,
}

#[derive(Serialize)]
struct FileInfo {
    name: String,
    size: u64,
    url: String,
}

#[derive(Clone, Serialize)]
struct FileEvent {
    action: String,
    name: String,
}

#[derive(Serialize)]
struct LlamaStatus {
    running: bool,
    model_available: bool,
}

#[derive(Default)]
struct FreeGamesCache {
    fetched_at: Option<Instant>,
    offers: Vec<FreeGameOffer>,
}

#[derive(Clone, Serialize)]
struct FreeGameOffer {
    id: String,
    title: String,
    store: String,
    url: String,
    expires_at: Option<String>,
    image_url: Option<String>,
    original_price: Option<String>,
}

fn free_games_seen_path() -> Option<PathBuf> {
    let dir = dirs::config_dir()
        .or_else(|| dirs::data_dir())
        .or_else(|| dirs::home_dir())?;
    Some(dir.join("neeko-assistant").join("free-games-seen.json"))
}

fn load_seen_free_games() -> HashSet<String> {
    let Some(path) = free_games_seen_path() else {
        return HashSet::new();
    };
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Vec<String>>(&text).ok())
        .unwrap_or_default()
        .into_iter()
        .collect()
}

fn save_seen_free_games(seen: &HashSet<String>) {
    let Some(path) = free_games_seen_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let mut ids: Vec<String> = seen.iter().cloned().collect();
    ids.sort();
    let _ = std::fs::write(
        path,
        serde_json::to_string_pretty(&ids).unwrap_or_else(|_| "[]".to_string()),
    );
}

fn notify_free_game(app: &AppHandle, offer: &FreeGameOffer) {
    let body = match &offer.expires_at {
        Some(expires) => format!("{} en {}. Termina: {}", offer.title, offer.store, expires),
        None => format!("{} en {}", offer.title, offer.store),
    };

    let _ = app
        .notification()
        .builder()
        .title("Neeko encontro un juego gratis")
        .body(body)
        .show();
}

async fn auth_middleware(
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let path = request.uri().path().to_string();
    if request.method() == axum::http::Method::OPTIONS
        || path == "/api/login"
        || path == "/"
        || path.starts_with("/static")
        || path == "/favicon.ico"
        || path.starts_with("/shared/")
        || path.starts_with("/api/localsend/")
        || path == "/api/upload"
        || path == "/api/files"
        || path == "/api/files/events"
        || path == "/api/install/events"
        || path == "/api/free-games"
        || path == "/api/llama/status"
    {
        return Ok(next.run(request).await);
    }

    if path.starts_with("/api/files/") && request.method() == "GET" {
        return Ok(next.run(request).await);
    }

    if let Some(auth) = headers.get("Authorization") {
        if let Ok(auth_str) = auth.to_str() {
            if auth_str == format!("Bearer {}", web_password()) {
                return Ok(next.run(request).await);
            }
        }
    }

    Err(StatusCode::UNAUTHORIZED)
}

async fn login_handler(
    State(_state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, StatusCode> {
    if payload.password.trim() == web_password() {
        Ok(Json(LoginResponse {
            success: true,
            token: web_password().to_string(),
        }))
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

fn safe_filename(filename: &str) -> String {
    std::path::Path::new(filename)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty() && *name != "." && *name != "..")
        .unwrap_or("unknown")
        .to_string()
}

fn localsend_id(prefix: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("{}-{}-{}", prefix, std::process::id(), nanos)
}

async fn localsend_prepare_handler(
    State(state): State<AppState>,
    Json(payload): Json<LocalSendPrepareRequest>,
) -> Result<Json<LocalSendPrepareResponse>, StatusCode> {
    if payload.files.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let session_id = localsend_id("session");
    let mut session = LocalSendSession::default();
    let mut tokens = HashMap::new();

    for (file_id, metadata) in payload.files {
        let token = localsend_id("token");
        session.files.insert(
            file_id.clone(),
            LocalSendFile {
                filename: safe_filename(&metadata.filename),
                token: token.clone(),
            },
        );
        tokens.insert(file_id, token);
    }

    state
        .localsend_sessions
        .lock()
        .await
        .insert(session_id.clone(), session);

    Ok(Json(LocalSendPrepareResponse {
        session_id,
        files: tokens,
    }))
}

async fn localsend_upload_handler(
    State(state): State<AppState>,
    Query(query): Query<LocalSendUploadQuery>,
    body: Body,
) -> Result<StatusCode, StatusCode> {
    let file = {
        let sessions = state.localsend_sessions.lock().await;
        sessions
            .get(&query.session_id)
            .and_then(|session| session.files.get(&query.file_id))
            .filter(|file| file.token == query.token)
            .cloned()
            .ok_or(StatusCode::FORBIDDEN)?
    };

    let dir = files_dir();
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let filepath = dir.join(&file.filename);
    let mut output = tokio::fs::File::create(&filepath)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut stream = body.into_data_stream();
    let mut total: u64 = 0;
    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        output
            .write_all(&chunk)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        total += chunk.len() as u64;
    }
    output
        .flush()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut sessions = state.localsend_sessions.lock().await;
    if let Some(session) = sessions.get_mut(&query.session_id) {
        session.files.remove(&query.file_id);
        if session.files.is_empty() {
            sessions.remove(&query.session_id);
        }
    }
    drop(sessions);

    let _ = state.file_events.send(FileEvent {
        action: "uploaded".to_string(),
        name: file.filename.clone(),
    });
    crate::notify_system(
        &state.app_handle,
        "Archivo recibido",
        &format!(
            "{} ({:.1} MB)",
            file.filename,
            total as f64 / (1024.0 * 1024.0)
        ),
    );

    Ok(StatusCode::OK)
}

async fn localsend_cancel_handler(
    State(state): State<AppState>,
    Query(query): Query<LocalSendCancelQuery>,
) -> StatusCode {
    state
        .localsend_sessions
        .lock()
        .await
        .remove(&query.session_id);
    StatusCode::OK
}

// ─── FILE SHARING (temp dir, like LocalSend) ───

async fn upload_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Body,
) -> Result<Json<ApiResponse>, StatusCode> {
    let dir = files_dir();
    std::fs::create_dir_all(&dir).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let filename = headers
        .get("X-Filename")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    let filename = urlencoding::decode(&filename)
        .unwrap_or(std::borrow::Cow::Borrowed(&filename))
        .into_owned();

    let filename = safe_filename(&filename);
    let filepath = dir.join(&filename);

    let mut file = tokio::fs::File::create(&filepath)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut body = body.into_data_stream();
    let mut total: u64 = 0;

    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;

    while let Some(chunk) = body.next().await {
        let chunk = chunk.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        file.write_all(&chunk)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        total += chunk.len() as u64;
    }

    file.flush()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    eprintln!(
        "[NEEKO] Upload OK: {} ({:.1} MB)",
        filename,
        total as f64 / (1024.0 * 1024.0)
    );

    let _ = state.file_events.send(FileEvent {
        action: "uploaded".to_string(),
        name: filename.clone(),
    });

    crate::notify_system(
        &state.app_handle,
        "Archivo recibido",
        &format!("{} ({:.1} MB)", filename, total as f64 / (1024.0 * 1024.0)),
    );

    Ok(Json(ApiResponse {
        ok: true,
        message: format!(
            "Subido: {} ({:.1} MB)",
            filename,
            total as f64 / (1024.0 * 1024.0)
        ),
    }))
}

async fn list_files_handler() -> Result<impl IntoResponse, StatusCode> {
    let dir = files_dir();
    if !dir.exists() {
        let _ = tokio::fs::create_dir_all(&dir).await;
    }

    let mut files = Vec::new();
    if let Ok(mut entries) = tokio::fs::read_dir(&dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.is_file() {
                let name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let size = tokio::fs::metadata(&path)
                    .await
                    .map(|m| m.len())
                    .unwrap_or(0);
                files.push(FileInfo {
                    name: name.clone(),
                    size,
                    url: format!("/shared/{}", name),
                });
            }
        }
    }

    files.sort_by(|a, b| b.size.cmp(&a.size));

    let json = serde_json::to_string(&files).unwrap_or_else(|_| "[]".to_string());
    let mut headers = HeaderMap::new();
    headers.insert("content-type", HeaderValue::from_static("application/json"));
    headers.insert(
        "cache-control",
        HeaderValue::from_static("no-store, no-cache, must-revalidate"),
    );
    headers.insert("pragma", HeaderValue::from_static("no-cache"));

    Ok((headers, json))
}

async fn file_events_handler(
    State(state): State<AppState>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    let receiver = state.file_events.subscribe();

    let stream = futures_util::stream::unfold(receiver, |mut receiver| async move {
        loop {
            match receiver.recv().await {
                Ok(event) => {
                    let data = serde_json::to_string(&event)
                        .unwrap_or_else(|_| "{\"action\":\"changed\"}".to_string());
                    return Some((
                        Ok(Event::default().event("files-changed").data(data)),
                        receiver,
                    ));
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn download_shared_handler(Path(filename): Path<String>) -> Result<Response, StatusCode> {
    let dir = files_dir();
    let filepath = dir.join(&filename);

    if !filepath.exists() || !filepath.starts_with(&dir) {
        return Err(StatusCode::NOT_FOUND);
    }

    let mime = mime_guess::from_path(&filepath)
        .first_or_octet_stream()
        .to_string();

    let data = std::fs::read(&filepath).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut headers = HeaderMap::new();
    headers.insert("content-type", HeaderValue::from_str(&mime).unwrap());
    headers.insert(
        "content-disposition",
        HeaderValue::from_str(&format!("attachment; filename=\"{}\"", filename)).unwrap(),
    );

    Ok((headers, data).into_response())
}

async fn download_handler(Path(filename): Path<String>) -> Result<Response, StatusCode> {
    let dir = files_dir();
    let filepath = dir.join(&filename);

    if !filepath.exists() || !filepath.starts_with(&dir) {
        return Err(StatusCode::NOT_FOUND);
    }

    let mime = mime_guess::from_path(&filepath)
        .first_or_octet_stream()
        .to_string();

    let data = std::fs::read(&filepath).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut headers = HeaderMap::new();
    headers.insert("content-type", HeaderValue::from_str(&mime).unwrap());
    headers.insert(
        "content-disposition",
        HeaderValue::from_str(&format!("attachment; filename=\"{}\"", filename)).unwrap(),
    );

    Ok((headers, data).into_response())
}

async fn delete_handler(
    State(state): State<AppState>,
    Path(filename): Path<String>,
) -> Result<Json<ApiResponse>, StatusCode> {
    let dir = files_dir();
    let filepath = dir.join(&filename);

    if !filepath.exists() || !filepath.starts_with(&dir) {
        return Ok(Json(ApiResponse {
            ok: false,
            message: "Archivo no encontrado".to_string(),
        }));
    }

    std::fs::remove_file(&filepath).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let _ = state.file_events.send(FileEvent {
        action: "deleted".to_string(),
        name: filename.clone(),
    });

    crate::notify_system(&state.app_handle, "Archivo eliminado", &filename);

    Ok(Json(ApiResponse {
        ok: true,
        message: format!("Eliminado: {}", filename),
    }))
}

// ─── LLAMA CONTROL ───

async fn llama_status_handler() -> Result<Json<LlamaStatus>, StatusCode> {
    let model_available = !crate::get_model_path().is_empty();
    let running = crate::is_llama_server_running() && model_available;
    Ok(Json(LlamaStatus {
        running,
        model_available,
    }))
}

async fn get_config_handler(State(state): State<AppState>) -> Json<WebConfigResponse> {
    let config = crate::config::AppConfig::load();
    Json(WebConfigResponse {
        language: crate::config::normalize_language(&config.language)
            .unwrap_or("es")
            .to_string(),
        start_with_windows: crate::get_start_with_windows_state(&state.app_handle)
            .unwrap_or(config.start_with_windows),
    })
}

async fn save_config_handler(
    State(state): State<AppState>,
    Json(payload): Json<WebConfigRequest>,
) -> Json<ApiResponse> {
    let mut config = crate::config::AppConfig::load();
    if let Some(language) = payload.language {
        let Some(language) = crate::config::normalize_language(&language) else {
            return Json(ApiResponse {
                ok: false,
                message: "Idioma no valido".to_string(),
            });
        };
        config.language = language.to_string();
    }

    let start_with_windows = payload.start_with_windows;
    if let Some(enabled) = start_with_windows {
        if let Err(error) = crate::apply_start_with_windows(&state.app_handle, enabled) {
            return Json(ApiResponse {
                ok: false,
                message: error,
            });
        }
        config.start_with_windows = enabled;
    }

    match config.save() {
        Ok(()) => Json(ApiResponse {
            ok: true,
            message: "Configuracion guardada".to_string(),
        }),
        Err(error) => Json(ApiResponse {
            ok: false,
            message: error,
        }),
    }
}

async fn llama_start_handler(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse>, StatusCode> {
    if crate::get_model_path().is_empty() {
        let _ = crate::stop_llama_server().await;
        return Ok(Json(ApiResponse {
            ok: false,
            message: "No encontre el modelo GGUF. Instala la IA primero.".to_string(),
        }));
    }

    let running = crate::is_llama_server_running();
    if running {
        return Ok(Json(ApiResponse {
            ok: true,
            message: "LLaMA ya está corriendo 🦎".to_string(),
        }));
    }

    let app_handle = state.app_handle.clone();
    tokio::spawn(async move {
        let result = crate::start_llama_server().await;
        match &result {
            Ok(msg) => crate::notify_system(&app_handle, "LLaMA iniciado", msg),
            Err(err) => crate::notify_system(&app_handle, "No pude iniciar LLaMA", err),
        }
    });

    Ok(Json(ApiResponse {
        ok: true,
        message: "Iniciando LLaMA... 🦎".to_string(),
    }))
}

async fn llama_stop_handler(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse>, StatusCode> {
    let running = crate::is_llama_server_running();
    if !running {
        return Ok(Json(ApiResponse {
            ok: true,
            message: "LLaMA ya estaba apagado".to_string(),
        }));
    }

    let app_handle = state.app_handle.clone();
    tokio::spawn(async move {
        let result = crate::stop_llama_server().await;
        match &result {
            Ok(msg) => crate::notify_system(&app_handle, "LLaMA apagado", msg),
            Err(err) => crate::notify_system(&app_handle, "No pude apagar LLaMA", err),
        }
    });

    Ok(Json(ApiResponse {
        ok: true,
        message: "Apagando LLaMA... 🦎".to_string(),
    }))
}

// ─── CHAT ───

fn epic_store_url(element: &serde_json::Value) -> String {
    let slug = element["productSlug"]
        .as_str()
        .or_else(|| element["catalogNs"]["mappings"][0]["pageSlug"].as_str())
        .or_else(|| element["urlSlug"].as_str())
        .unwrap_or("");

    if slug.is_empty() {
        "https://store.epicgames.com/free-games".to_string()
    } else {
        format!("https://store.epicgames.com/p/{}", slug)
    }
}

fn epic_image_url(element: &serde_json::Value) -> Option<String> {
    element["keyImages"]
        .as_array()
        .and_then(|images| {
            images
                .iter()
                .find(|img| img["type"].as_str() == Some("OfferImageWide"))
                .or_else(|| {
                    images
                        .iter()
                        .find(|img| img["type"].as_str() == Some("DieselStoreFrontWide"))
                })
                .or_else(|| images.first())
        })
        .and_then(|img| img["url"].as_str())
        .map(|url| url.to_string())
}

fn epic_current_promo(element: &serde_json::Value) -> Option<&serde_json::Value> {
    element["promotions"]["promotionalOffers"]
        .as_array()?
        .iter()
        .flat_map(|group| group["promotionalOffers"].as_array().into_iter().flatten())
        .find(|promo| {
            promo["discountSetting"]["discountPercentage"]
                .as_i64()
                .map(|discount| discount == 0)
                .unwrap_or(false)
        })
}

async fn fetch_epic_free_games(client: &reqwest::Client) -> Vec<FreeGameOffer> {
    let url = "https://store-site-backend-static.ak.epicgames.com/freeGamesPromotions?locale=es-ES&country=AR&allowCountries=AR";
    let resp = match client
        .get(url)
        .header("User-Agent", WEB_USER_AGENT)
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(error) => {
            eprintln!("[NEEKO] No pude consultar Epic gratis: {}", error);
            return Vec::new();
        }
    };
    if !resp.status().is_success() {
        eprintln!(
            "[NEEKO] Epic gratis respondio HTTP {}",
            resp.status().as_u16()
        );
        return Vec::new();
    }
    let data = match resp.json::<serde_json::Value>().await {
        Ok(data) => data,
        Err(error) => {
            eprintln!("[NEEKO] No pude parsear Epic gratis: {}", error);
            return Vec::new();
        }
    };

    data["data"]["Catalog"]["searchStore"]["elements"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|element| {
            let promo = epic_current_promo(element)?;
            let title = element["title"].as_str()?.to_string();
            let id = element["id"]
                .as_str()
                .or_else(|| element["namespace"].as_str())
                .unwrap_or(title.as_str())
                .to_string();
            let original_price = element["price"]["totalPrice"]["fmtPrice"]["originalPrice"]
                .as_str()
                .map(|price| price.to_string());

            Some(FreeGameOffer {
                id: format!("epic:{}", id),
                title,
                store: "Epic Games".to_string(),
                url: epic_store_url(element),
                expires_at: promo["endDate"].as_str().map(|date| date.to_string()),
                image_url: epic_image_url(element),
                original_price,
            })
        })
        .collect()
}

fn html_attr(html: &str, name: &str) -> Option<String> {
    let pattern = format!(r#"{}\s*=\s*"([^"]+)""#, regex::escape(name));
    regex::Regex::new(&pattern)
        .ok()?
        .captures(html)?
        .get(1)
        .map(|m| m.as_str().to_string())
}

fn html_text(html: &str, class_name: &str) -> Option<String> {
    let pattern = format!(
        r#"<[^>]*class="[^"]*{}[^"]*"[^>]*>(.*?)</[^>]+>"#,
        regex::escape(class_name)
    );
    let raw = regex::Regex::new(&pattern)
        .ok()?
        .captures(html)?
        .get(1)?
        .as_str();
    let without_tags = regex::Regex::new(r"<[^>]+>").ok()?.replace_all(raw, "");
    Some(
        without_tags
            .replace("&amp;", "&")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .trim()
            .to_string(),
    )
}

async fn fetch_steam_free_games(client: &reqwest::Client) -> Vec<FreeGameOffer> {
    let url = "https://store.steampowered.com/search/?maxprice=free&category1=998&specials=1&ndl=1";
    let resp = match client
        .get(url)
        .header("User-Agent", WEB_USER_AGENT)
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(error) => {
            eprintln!("[NEEKO] No pude consultar Steam gratis: {}", error);
            return Vec::new();
        }
    };
    if !resp.status().is_success() {
        eprintln!(
            "[NEEKO] Steam gratis respondio HTTP {}",
            resp.status().as_u16()
        );
        return Vec::new();
    }
    let html = match resp.text().await {
        Ok(html) => html,
        Err(error) => {
            eprintln!("[NEEKO] No pude leer Steam gratis: {}", error);
            return Vec::new();
        }
    };

    let row_re =
        match regex::Regex::new(r#"(?s)<a[^>]*class="[^"]*search_result_row[^"]*"[^>]*>.*?</a>"#) {
            Ok(re) => re,
            Err(_) => return Vec::new(),
        };

    row_re
        .find_iter(&html)
        .filter_map(|row| {
            let row = row.as_str();
            if !row.contains("search_discount") || !row.contains("-100%") {
                return None;
            }

            let title = html_text(row, "title")?;
            let app_id = html_attr(row, "data-ds-appid").unwrap_or_else(|| title.clone());
            let href = html_attr(row, "href").unwrap_or_else(|| {
                "https://store.steampowered.com/search/?maxprice=free&category1=998&specials=1"
                    .to_string()
            });
            let image_url = html_attr(row, "src");
            let original_price = html_text(row, "discount_original_price");

            Some(FreeGameOffer {
                id: format!("steam:{}", app_id),
                title,
                store: "Steam".to_string(),
                url: href,
                expires_at: None,
                image_url,
                original_price,
            })
        })
        .take(20)
        .collect()
}

async fn free_games_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<FreeGameOffer>>, StatusCode> {
    Ok(Json(refresh_free_games(state, true).await))
}

async fn test_free_games_notification_handler(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse>, StatusCode> {
    let offer = FreeGameOffer {
        id: "test".to_string(),
        title: "Aviso de prueba".to_string(),
        store: "Neeko".to_string(),
        url: "http://localhost:1414".to_string(),
        expires_at: None,
        image_url: None,
        original_price: None,
    };

    notify_free_game(&state.app_handle, &offer);
    Ok(Json(ApiResponse {
        ok: true,
        message: "Notificacion enviada".to_string(),
    }))
}

async fn refresh_free_games(state: AppState, notify: bool) -> Vec<FreeGameOffer> {
    {
        let cache = state.free_games_cache.lock().await;
        if cache
            .fetched_at
            .map(|at| at.elapsed() < Duration::from_secs(15 * 60))
            .unwrap_or(false)
        {
            return cache.offers.clone();
        }
    }

    let (epic, steam) = tokio::join!(
        fetch_epic_free_games(&state.client),
        fetch_steam_free_games(&state.client)
    );

    let mut offers = epic;
    offers.extend(steam);
    offers.sort_by(|a, b| a.store.cmp(&b.store).then_with(|| a.title.cmp(&b.title)));

    {
        let mut seen = state.free_games_seen.lock().await;
        let first_run = seen.is_empty();
        let mut changed = false;

        for offer in &offers {
            if seen.insert(offer.id.clone()) {
                changed = true;
                if notify && !first_run {
                    notify_free_game(&state.app_handle, offer);
                }
            }
        }

        if changed {
            save_seen_free_games(&seen);
        }
    }

    let mut cache = state.free_games_cache.lock().await;
    cache.fetched_at = Some(Instant::now());
    cache.offers = offers.clone();

    offers
}

async fn chat_handler(
    Json(payload): Json<ChatRequest>,
) -> Result<Json<crate::assistant::Reply>, (StatusCode, String)> {
    crate::assistant::assistant_chat(format!("web:{}", payload.session), payload.messages)
        .await
        .map(Json)
        .map_err(|error| (StatusCode::BAD_REQUEST, error))
}

async fn chat_decide_handler(
    State(state): State<AppState>,
    Json(payload): Json<ChatDecision>,
) -> Result<Json<ApiResponse>, (StatusCode, String)> {
    crate::assistant::assistant_decide(
        state.app_handle,
        format!("web:{}", payload.session),
        payload.proposal_id,
        payload.approved,
        payload.save_account,
    )
    .await
    .map(|message| Json(ApiResponse { ok: true, message }))
    .map_err(|error| (StatusCode::BAD_REQUEST, error))
}

async fn chat_cancel_handler(Json(payload): Json<ChatCancel>) -> Json<ApiResponse> {
    crate::assistant::assistant_cancel(format!("web:{}", payload.session));
    Json(ApiResponse {
        ok: true,
        message: "cancelado".into(),
    })
}

async fn open_app_handler(
    State(state): State<AppState>,
    Json(payload): Json<OpenAppRequest>,
) -> Result<Json<ApiResponse>, StatusCode> {
    let result =
        crate::open_any_app_with_notify(Some(&state.app_handle), payload.app.clone()).await;
    match result {
        Ok(msg) => Ok(Json(ApiResponse {
            ok: true,
            message: msg,
        })),
        Err(e) => Ok(Json(ApiResponse {
            ok: false,
            message: e,
        })),
    }
}

async fn search_handler(
    Json(payload): Json<SearchRequest>,
) -> Result<Json<ApiResponse>, StatusCode> {
    let url = format!(
        "https://www.google.com/search?q={}",
        payload.query.replace(' ', "+")
    );
    match open::that(&url) {
        Ok(_) => Ok(Json(ApiResponse {
            ok: true,
            message: if crate::language_is_english() {
                format!("Searched: {}", payload.query)
            } else {
                format!("Busque: {}", payload.query)
            },
        })),
        Err(e) => Ok(Json(ApiResponse {
            ok: false,
            message: format!("Error: {}", e),
        })),
    }
}

async fn open_url_handler(
    Json(payload): Json<UrlRequest>,
) -> Result<Json<ApiResponse>, StatusCode> {
    let full_url = if payload.url.starts_with("http") {
        payload.url
    } else {
        format!("https://{}", payload.url)
    };
    match open::that(&full_url) {
        Ok(_) => Ok(Json(ApiResponse {
            ok: true,
            message: if crate::language_is_english() {
                format!("Opened {}", full_url)
            } else {
                format!("Abri {}", full_url)
            },
        })),
        Err(e) => Ok(Json(ApiResponse {
            ok: false,
            message: format!("Error: {}", e),
        })),
    }
}

async fn install_ffmpeg_handler(State(state): State<AppState>) -> Json<ApiResponse> {
    match crate::install_ffmpeg_impl(state.app_handle.clone()).await {
        Ok(message) => Json(ApiResponse { ok: true, message }),
        Err(error) => Json(ApiResponse {
            ok: false,
            message: error,
        }),
    }
}

async fn install_git_handler(State(state): State<AppState>) -> Json<ApiResponse> {
    match crate::install_git_impl(state.app_handle.clone()).await {
        Ok(message) => Json(ApiResponse { ok: true, message }),
        Err(error) => Json(ApiResponse {
            ok: false,
            message: error,
        }),
    }
}

async fn install_model_handler(
    State(state): State<AppState>,
    Json(payload): Json<InstallModelRequest>,
) -> Json<ApiResponse> {
    match crate::install_model_impl(state.app_handle.clone(), payload.model_url).await {
        Ok(message) => Json(ApiResponse { ok: true, message }),
        Err(error) => Json(ApiResponse {
            ok: false,
            message: error,
        }),
    }
}

async fn cancel_download_handler(Json(payload): Json<CancelDownloadRequest>) -> Json<ApiResponse> {
    match crate::cancel_download(payload.id) {
        Ok(message) => Json(ApiResponse { ok: true, message }),
        Err(error) => Json(ApiResponse {
            ok: false,
            message: error,
        }),
    }
}

async fn check_update_handler(State(state): State<AppState>) -> Json<ApiResponse> {
    match crate::check_updates(state.app_handle.clone()).await {
        Ok(value) => Json(ApiResponse {
            ok: true,
            message: value.to_string(),
        }),
        Err(error) => Json(ApiResponse {
            ok: false,
            message: error,
        }),
    }
}

async fn install_model_file_handler(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Json<ApiResponse> {
    use tokio::io::AsyncWriteExt;

    while let Ok(Some(mut field)) = multipart.next_field().await {
        let Some(file_name) = field.file_name().map(|name| name.to_string()) else {
            continue;
        };

        let file_name = match crate::sanitize_model_file_name(&file_name) {
            Ok(name) => name,
            Err(error) => {
                return Json(ApiResponse {
                    ok: false,
                    message: error,
                });
            }
        };

        let target_dir = crate::installed_models_dir();
        if let Err(error) = tokio::fs::create_dir_all(&target_dir).await {
            return Json(ApiResponse {
                ok: false,
                message: format!("No pude crear la carpeta IA: {}", error),
            });
        }

        let target = target_dir.join(file_name);
        let mut output = match tokio::fs::File::create(&target).await {
            Ok(file) => file,
            Err(error) => {
                return Json(ApiResponse {
                    ok: false,
                    message: format!("No pude crear el modelo instalado: {}", error),
                });
            }
        };

        let mut written = 0_u64;
        loop {
            match field.chunk().await {
                Ok(Some(chunk)) => {
                    written += chunk.len() as u64;
                    if let Err(error) = output.write_all(&chunk).await {
                        return Json(ApiResponse {
                            ok: false,
                            message: format!("No pude guardar el modelo: {}", error),
                        });
                    }
                }
                Ok(None) => break,
                Err(error) => {
                    return Json(ApiResponse {
                        ok: false,
                        message: format!("No pude leer el archivo subido: {}", error),
                    });
                }
            }
        }

        if written == 0 {
            let _ = tokio::fs::remove_file(&target).await;
            return Json(ApiResponse {
                ok: false,
                message: "El archivo del modelo esta vacio".to_string(),
            });
        }

        let message = format!("Modelo instalado en {}", target.display());
        crate::notify_system(&state.app_handle, "Modelo instalado", &message);
        return Json(ApiResponse { ok: true, message });
    }

    Json(ApiResponse {
        ok: false,
        message: "Selecciona un archivo .gguf para instalar".to_string(),
    })
}

async fn uninstall_ffmpeg_handler() -> Json<ApiResponse> {
    match crate::uninstall_ffmpeg_impl() {
        Ok(message) => Json(ApiResponse { ok: true, message }),
        Err(error) => Json(ApiResponse {
            ok: false,
            message: error,
        }),
    }
}

async fn uninstall_git_handler() -> Json<ApiResponse> {
    match crate::uninstall_git_impl() {
        Ok(message) => Json(ApiResponse { ok: true, message }),
        Err(error) => Json(ApiResponse {
            ok: false,
            message: error,
        }),
    }
}

async fn uninstall_model_handler() -> Json<ApiResponse> {
    match crate::uninstall_model_impl() {
        Ok(message) => Json(ApiResponse { ok: true, message }),
        Err(error) => Json(ApiResponse {
            ok: false,
            message: error,
        }),
    }
}

async fn install_events_handler() -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>>
{
    let rx = crate::subscribe_download_progress();
    let stream = futures_util::stream::unfold(rx, |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(progress) => {
                    let data =
                        serde_json::to_string(&progress).unwrap_or_else(|_| "{}".to_string());
                    return Some((Ok(Event::default().event("progress").data(data)), rx));
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn web_ui() -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert("cache-control", HeaderValue::from_static("no-store"));
    headers.insert("pragma", HeaderValue::from_static("no-cache"));

    (headers, Html(include_str!("../../web/index.html")))
}

pub async fn start_web_server(app_handle: AppHandle) {
    let temp = files_dir();
    let _ = std::fs::create_dir_all(&temp);
    eprintln!("[NEEKO] 📁 Temp files dir: {}", temp.display());

    let state = AppState {
        app_handle,
        client: reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .unwrap(),
        file_events: broadcast::channel(100).0,
        free_games_cache: Arc::new(AsyncMutex::new(FreeGamesCache::default())),
        free_games_seen: Arc::new(AsyncMutex::new(load_seen_free_games())),
        localsend_sessions: Arc::new(AsyncMutex::new(HashMap::new())),
    };

    let games_state = state.clone();
    tokio::spawn(async move {
        let _ = refresh_free_games(games_state.clone(), true).await;
        let mut interval = tokio::time::interval(Duration::from_secs(30 * 60));
        loop {
            interval.tick().await;
            let _ = refresh_free_games(games_state.clone(), true).await;
        }
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let api_routes = Router::new()
        .route("/login", post(login_handler))
        .route("/chat", post(chat_handler))
        .route("/chat/decide", post(chat_decide_handler))
        .route("/chat/cancel", post(chat_cancel_handler))
        .route("/open", post(open_app_handler))
        .route("/search", post(search_handler))
        .route("/url", post(open_url_handler))
        .route("/upload", post(upload_handler))
        .route(
            "/localsend/v2/prepare-upload",
            post(localsend_prepare_handler),
        )
        .route("/localsend/v2/upload", post(localsend_upload_handler))
        .route("/localsend/v2/cancel", post(localsend_cancel_handler))
        .route("/files", get(list_files_handler))
        .route("/files/events", get(file_events_handler))
        .route("/free-games", get(free_games_handler))
        .route("/config", get(get_config_handler).post(save_config_handler))
        .route(
            "/free-games/test-notification",
            post(test_free_games_notification_handler),
        )
        .route(
            "/files/{name}",
            get(download_handler).delete(delete_handler),
        )
        .route("/llama/status", get(llama_status_handler))
        .route("/llama/start", post(llama_start_handler))
        .route("/llama/stop", post(llama_stop_handler))
        .route("/install/events", get(install_events_handler))
        .route("/install/ffmpeg", post(install_ffmpeg_handler))
        .route("/install/git", post(install_git_handler))
        .route("/install/model", post(install_model_handler))
        .route("/install/model-file", post(install_model_file_handler))
        .route("/install/cancel", post(cancel_download_handler))
        .route("/uninstall/ffmpeg", post(uninstall_ffmpeg_handler))
        .route("/uninstall/git", post(uninstall_git_handler))
        .route("/uninstall/model", post(uninstall_model_handler))
        .route("/check-update", post(check_update_handler));

    let shared_routes = Router::new().route("/{name}", get(download_shared_handler));

    let app = Router::new()
        .route("/", get(web_ui))
        .route(
            "/static/chat-client.mjs",
            get(|| async {
                (
                    [(
                        axum::http::header::CONTENT_TYPE,
                        "text/javascript; charset=utf-8",
                    )],
                    include_str!("../../src/chat-client.mjs"),
                )
            }),
        )
        .nest("/api", api_routes)
        .nest("/shared", shared_routes)
        .layer(cors)
        .layer(DefaultBodyLimit::max(8 * 1024 * 1024 * 1024usize))
        .layer(middleware::from_fn(auth_middleware))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], PORT));
    eprintln!(
        "[NEEKO] 🌐 Web server running on http://{}:{}",
        get_local_ip(),
        PORT
    );

    eprintln!("[NEEKO] Web password: {}", web_password());

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

fn get_local_ip() -> String {
    if let Ok(socket) = std::net::UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = socket.local_addr() {
                return addr.ip().to_string();
            }
        }
    }
    "localhost".to_string()
}
