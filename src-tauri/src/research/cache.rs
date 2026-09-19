use super::{ResearchSearch, Source};
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

struct Entry {
    sources: Vec<Source>,
    expires: Instant,
}

static CACHE: OnceLock<Mutex<HashMap<String, Entry>>> = OnceLock::new();

fn cache() -> &'static Mutex<HashMap<String, Entry>> {
    CACHE.get_or_init(Default::default)
}

pub fn key(searches: &[ResearchSearch], intent: &str, english: bool) -> String {
    let mut parts = vec![format!("intent={intent}"), format!("lang={english}")];
    for search in searches.iter().take(3) {
        parts.push(format!(
            "{}:{}",
            search.scope.trim().to_lowercase(),
            search.query.trim().to_lowercase()
        ));
    }
    parts.join("|")
}

pub fn get(key: &str) -> Option<Vec<Source>> {
    let mut cache = cache().lock().ok()?;
    cache.retain(|_, entry| entry.expires > Instant::now());
    cache.get(key).map(|entry| entry.sources.clone())
}

pub fn put(key: String, sources: Vec<Source>, intent: &str) {
    let ttl = match intent {
        "news" | "current" => Duration::from_secs(15 * 60),
        "software" | "bug" | "product" | "hardware" => Duration::from_secs(60 * 60),
        "opinions" => Duration::from_secs(2 * 60 * 60),
        _ => Duration::from_secs(12 * 60 * 60),
    };
    let mut cache = match cache().lock() {
        Ok(cache) => cache,
        Err(_) => return,
    };
    cache.retain(|_, entry| entry.expires > Instant::now());
    if cache.len() > 64 {
        cache.clear();
    }
    cache.insert(
        key,
        Entry {
            sources,
            expires: Instant::now() + ttl,
        },
    );
}

