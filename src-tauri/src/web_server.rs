use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, Request, State},
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
use std::collections::HashSet;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;
use tokio::sync::broadcast;
use tokio::sync::Mutex as AsyncMutex;
use tower_http::cors::{Any, CorsLayer};

const PASSWORD: &str = "Lorena25";
const PORT: u16 = 1414;

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
}

#[derive(Deserialize)]
struct ChatRequest {
    messages: Vec<ChatMessage>,
}

#[derive(Deserialize, Serialize, Clone)]
struct ChatMessage {
    role: String,
    content: String,
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
    if path == "/api/login"
        || path == "/"
        || path.starts_with("/static")
        || path == "/favicon.ico"
        || path.starts_with("/shared/")
        || path == "/api/files"
        || path == "/api/files/events"
        || path == "/api/install/events"
        || path == "/api/llama/status"
    {
        return Ok(next.run(request).await);
    }

    if path.starts_with("/api/files/") && request.method() == "GET" {
        return Ok(next.run(request).await);
    }

    if let Some(auth) = headers.get("Authorization") {
        if let Ok(auth_str) = auth.to_str() {
            if auth_str == format!("Bearer {}", PASSWORD) {
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
    if payload.password == PASSWORD {
        Ok(Json(LoginResponse {
            success: true,
            token: PASSWORD.to_string(),
        }))
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
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
    let Ok(resp) = client.get(url).send().await else {
        return Vec::new();
    };
    let Ok(data) = resp.json::<serde_json::Value>().await else {
        return Vec::new();
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
    let Ok(resp) = client
        .get(url)
        .header("User-Agent", "Mozilla/5.0 NeekoAssistant/1.0")
        .send()
        .await
    else {
        return Vec::new();
    };
    let Ok(html) = resp.text().await else {
        return Vec::new();
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

enum SystemChatAction {
    Shutdown(Option<u64>),
    CancelShutdown,
    RestartExplorer,
    RestartWifi,
    RestartBluetooth,
}

fn detect_system_chat_action(lower: &str) -> Option<SystemChatAction> {
    if regex::Regex::new(r"cancel(?:ar)?\s+(?:el\s+)?(?:apagado|shutdown|apaga)")
        .ok()?
        .is_match(lower)
    {
        return Some(SystemChatAction::CancelShutdown);
    }

    if let Ok(re) = regex::Regex::new(
        r"apag(?:a|ar|o)\s+(?:la\s+)?pc\s+en\s+(\d+)\s*(min(?:uto)?s?|horas?|h|s(?:egundo)?s?)",
    ) {
        if let Some(caps) = re.captures(lower) {
            let mut seconds = caps
                .get(1)
                .and_then(|m| m.as_str().parse::<u64>().ok())
                .unwrap_or(0);
            let unit = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            if unit.starts_with('h') || unit.starts_with("hora") {
                seconds *= 3600;
            } else if unit.starts_with("min") {
                seconds *= 60;
            }
            return Some(SystemChatAction::Shutdown(Some(seconds)));
        }
    }

    if regex::Regex::new(r"apag(?:a|ar|o)\s+(?:la\s+)?pc")
        .ok()?
        .is_match(lower)
    {
        return Some(SystemChatAction::Shutdown(Some(0)));
    }

    if regex::Regex::new(
        r"reinici(?:a|ar|o)\s+(?:el\s+)?(?:explorer|iconos?|barra|escritorio|windows\s*explorer)",
    )
    .ok()?
    .is_match(lower)
    {
        return Some(SystemChatAction::RestartExplorer);
    }

    if regex::Regex::new(
        r"reinici(?:a|ar|o)\s+(?:el\s+)?(?:wifi|wi-fi|internet|red|conexion|conexi[oó]n)",
    )
    .ok()?
    .is_match(lower)
    {
        return Some(SystemChatAction::RestartWifi);
    }

    if regex::Regex::new(r"reinici(?:a|ar|o)\s+(?:el\s+)?(?:bluetooth|blue\s*tooth)")
        .ok()?
        .is_match(lower)
    {
        return Some(SystemChatAction::RestartBluetooth);
    }

    None
}

fn execute_system_chat_action(action: SystemChatAction) -> String {
    match action {
        SystemChatAction::Shutdown(seconds) => crate::system_shutdown_impl(seconds),
        SystemChatAction::CancelShutdown => crate::system_cancel_shutdown_impl(),
        SystemChatAction::RestartExplorer => crate::system_restart_explorer_impl(),
        SystemChatAction::RestartWifi => crate::system_restart_wifi_impl(),
        SystemChatAction::RestartBluetooth => crate::system_restart_bluetooth_impl(),
    }
    .unwrap_or_else(|e| e)
}

async fn chat_handler(
    State(state): State<AppState>,
    Json(payload): Json<ChatRequest>,
) -> Result<Json<ApiResponse>, StatusCode> {
    let user_msg = payload
        .messages
        .last()
        .map(|m| m.content.as_str())
        .unwrap_or("")
        .to_string();
    let lower = user_msg.to_lowercase();

    // Check if llama is running before trying to use AI
    let llama_running = crate::is_llama_server_running();

    if lower == "ip"
        || lower == "mi ip"
        || lower == "la ip"
        || (lower.contains("ip")
            && (lower.contains("cuál")
                || lower.contains("cual")
                || lower.contains("que")
                || lower.contains("cómo")
                || lower.contains("como")
                || lower.contains("connect")
                || lower.contains("conect")
                || lower.contains("dirección")
                || lower.contains("direccion")))
    {
        let ip = get_local_ip();
        let msg = format!("La IP para conectarte es: http://{}:1414", ip);
        return Ok(Json(ApiResponse {
            ok: true,
            message: msg,
        }));
    }

    if let Some(action) = detect_system_chat_action(&lower) {
        return Ok(Json(ApiResponse {
            ok: true,
            message: execute_system_chat_action(action),
        }));
    }

    // Llama control from chat
    if lower.contains("cierra") && lower.contains("llama") {
        let result = crate::stop_llama_server().await;
        return Ok(Json(ApiResponse {
            ok: true,
            message: result.unwrap_or_else(|e| e),
        }));
    }
    if lower.contains("abre") && lower.contains("llama") {
        let result = crate::start_llama_server().await;
        return Ok(Json(ApiResponse {
            ok: true,
            message: result.unwrap_or_else(|e| e),
        }));
    }

    let known_apps = [
        "spotify",
        "discord",
        "steam",
        "chrome",
        "firefox",
        "edge",
        "notepad",
        "calculadora",
        "calculator",
        "explorer",
        "vscode",
        "code",
        "powershell",
        "terminal",
        "whatsapp",
        "telegram",
        "obs",
        "youtube",
        "league of legends",
        "lol",
        "riot client",
        "7-zip",
        "7zip",
        "winrar",
        "obsidian",
        "brave",
        "bluestacks",
        "roblox",
        "fightcade",
        "qbittorrent",
        "davinci",
        "filmora",
        "photoshop",
        "photoshop cs6",
        "virtualbox",
        "node",
        "python",
        "git",
    ];

    let open_patterns = [
        r"abr[ií]?\s+(.+)",
        r"abre\s+(.+)",
        r"abrir\s+(.+)",
        r"abrime\s+(.+)",
        r"pone[r]?\s+(.+)",
        r"iniciar\s+(.+)",
        r"ejecutar\s+(.+)",
    ];

    for app in &known_apps {
        if lower == *app
            || lower == format!("abri {}", app)
            || lower == format!("abre {}", app)
            || lower == format!("abrir {}", app)
        {
            let result = crate::open_any_app_with_notify(None, app.to_string()).await;
            return Ok(Json(ApiResponse {
                ok: true,
                message: result.unwrap_or_else(|e| e),
            }));
        }
    }

    for pattern in &open_patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            if let Some(caps) = re.captures(&lower) {
                if let Some(app_name) = caps.get(1) {
                    let app = app_name.as_str().trim();
                    let result =
                        crate::open_any_app_with_notify(Some(&state.app_handle), app.to_string())
                            .await;
                    return Ok(Json(ApiResponse {
                        ok: true,
                        message: result.unwrap_or_else(|e| e),
                    }));
                }
            }
        }
    }

    let search_in_patterns = [
        r"busca[r]?\s+en\s+([^:]+)\s*:\s*(.+)",
        r"buscar\s+en\s+([^:]+)\s*:\s*(.+)",
    ];
    for pattern in &search_in_patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            if let Some(caps) = re.captures(&lower) {
                if let (Some(site), Some(query)) = (caps.get(1), caps.get(2)) {
                    let site = site.as_str().trim();
                    let query = query.as_str().trim();
                    let url = search_url_for_site(site, query);
                    let _ = open::that(&url);
                    return Ok(Json(ApiResponse {
                        ok: true,
                        message: format!("Busque en {}: {}", site, query),
                    }));
                }
            }
        }
    }

    let search_patterns = [r"busca[r]?\s+(.+)", r"investigar\s+(.+)"];
    for pattern in &search_patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            if let Some(caps) = re.captures(&lower) {
                if let Some(query) = caps.get(1) {
                    let q = query.as_str().trim();
                    let url = format!("https://www.google.com/search?q={}", q.replace(' ', "+"));
                    let _ = open::that(&url);
                    return Ok(Json(ApiResponse {
                        ok: true,
                        message: format!("Busque: {}", q),
                    }));
                }
            }
        }
    }

    if !llama_running {
        return Ok(Json(ApiResponse {
            ok: true,
            message: "LLaMA está apagado. Decí 'abre llama' para activarlo.".to_string(),
        }));
    }

    let body = serde_json::json!({
        "model": "neeko",
        "messages": payload.messages,
        "stream": false
    });

    let resp = state
        .client
        .post("http://127.0.0.1:8080/v1/chat/completions")
        .json(&body)
        .send()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    if !resp.status().is_success() {
        return Err(StatusCode::BAD_GATEWAY);
    }

    let data: serde_json::Value = resp.json().await.map_err(|_| StatusCode::BAD_GATEWAY)?;

    let reply = data["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("No entendí")
        .to_string();

    if let Some(action) = extract_action(&reply) {
        let result = execute_web_action(&action).await;
        if let Some(msg) = result {
            return Ok(Json(ApiResponse {
                ok: true,
                message: msg,
            }));
        }
    }

    Ok(Json(ApiResponse {
        ok: true,
        message: reply,
    }))
}

fn extract_action(text: &str) -> Option<serde_json::Value> {
    if let Some(start) = text.find('{') {
        if let Some(end) = text[start..].find('}') {
            let json_str = &text[start..=start + end];
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
                if val.get("action").is_some() {
                    return Some(val);
                }
            }
        }
    }
    None
}

async fn execute_web_action(action: &serde_json::Value) -> Option<String> {
    let act = action["action"].as_str()?;
    match act {
        "open_app" => {
            let app = action["app"].as_str()?;
            let result = crate::open_any_app_with_notify(None, app.to_string()).await;
            Some(result.unwrap_or_else(|e| e))
        }
        "search" => {
            let query = action["query"].as_str()?;
            let url = format!(
                "https://www.google.com/search?q={}",
                query.replace(' ', "+")
            );
            let _ = open::that(&url);
            Some(format!("Busque: {}", query))
        }
        "open_url" => {
            let url = action["url"].as_str()?;
            let _ = open::that(url);
            Some(format!("Abrí: {}", url))
        }
        "play_music" => {
            let query = action["query"].as_str()?;
            let url = format!(
                "https://www.youtube.com/results?search_query={}",
                query.replace(' ', "+")
            );
            let _ = open::that(&url);
            Some(format!("Buscando música: {}", query))
        }
        "shutdown" => {
            let seconds = action["seconds"].as_u64();
            Some(crate::system_shutdown_impl(seconds).unwrap_or_else(|e| e))
        }
        "cancel_shutdown" => Some(crate::system_cancel_shutdown_impl().unwrap_or_else(|e| e)),
        "restart_explorer" => Some(crate::system_restart_explorer_impl().unwrap_or_else(|e| e)),
        "restart_wifi" => Some(crate::system_restart_wifi_impl().unwrap_or_else(|e| e)),
        "restart_bluetooth" => Some(crate::system_restart_bluetooth_impl().unwrap_or_else(|e| e)),
        _ => None,
    }
}

fn search_url_for_site(site: &str, query: &str) -> String {
    let site = site.trim().to_lowercase();
    let query = urlencoding::encode(query.trim());

    match site.as_str() {
        "google" | "g" => format!("https://www.google.com/search?q={}", query),
        "youtube" | "yt" => format!("https://www.youtube.com/results?search_query={}", query),
        "github" | "gh" => format!("https://github.com/search?q={}", query),
        "reddit" => format!("https://www.reddit.com/search/?q={}", query),
        "mercado" | "mercadolibre" | "ml" => {
            format!("https://listado.mercadolibre.com.ar/{}", query)
        }
        "wikipedia" | "wiki" => format!("https://es.wikipedia.org/w/index.php?search={}", query),
        "spotify" => format!("https://open.spotify.com/search/{}", query),
        "steam" => format!("https://store.steampowered.com/search/?term={}", query),
        _ => format!(
            "https://www.google.com/search?q=site%3A{}+{}",
            urlencoding::encode(site.as_str()),
            query
        ),
    }
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
            message: format!("Busque: {}", payload.query),
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
            message: format!("Abrí {}", full_url),
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
        .route("/open", post(open_app_handler))
        .route("/search", post(search_handler))
        .route("/url", post(open_url_handler))
        .route("/upload", post(upload_handler))
        .route("/files", get(list_files_handler))
        .route("/files/events", get(file_events_handler))
        .route("/free-games", get(free_games_handler))
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
