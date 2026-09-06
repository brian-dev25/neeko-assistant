//! Deterministic offline commands. Produces proposals only; never executes them.
use serde_json::{json, Value};

fn capture<'a>(pattern: &str, text: &'a str) -> Option<regex::Captures<'a>> {
    regex::Regex::new(pattern).ok()?.captures(text)
}

pub fn normalize_site(site: &str) -> Result<String, String> {
    let site = site.trim().to_lowercase();
    let site = site
        .strip_prefix("https://")
        .or_else(|| site.strip_prefix("http://"))
        .unwrap_or(&site);
    let site = site
        .strip_prefix("www.")
        .unwrap_or(site)
        .trim_end_matches('/');
    let known = match site {
        "g" | "google" | "google.com" => "google",
        "yt" | "youtube" | "youtube.com" | "youtu.be" => "youtube",
        "gh" | "github" | "github.com" => "github",
        "reddit" | "reddit.com" => "reddit",
        "ml" | "mercado" | "mercadolibre" | "mercadolibre.com.ar" => "mercadolibre",
        "wiki" | "wikipedia" | "wikipedia.org" | "es.wikipedia.org" => "wikipedia",
        "spotify" | "open.spotify.com" => "spotify",
        "steam" | "store.steampowered.com" => "steam",
        "bing" | "bing.com" => "bing",
        "ddg" | "duckduckgo" | "duckduckgo.com" => "duckduckgo",
        _ => {
            if capture(
                r"^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]*[a-z0-9])?)+$",
                site,
            )
            .is_some()
            {
                return Ok(site.into());
            }
            return Err(format!("Sitio no reconocido: {site}. Usá un nombre como YouTube o un dominio como example.com."));
        }
    };
    Ok(known.into())
}

pub fn search_url(site: &str, query: &str) -> Result<String, String> {
    let site = normalize_site(site)?;
    let query = urlencoding::encode(query);
    Ok(match site.as_str() {
        "google" => format!("https://www.google.com/search?q={query}"),
        "youtube" => format!("https://www.youtube.com/results?search_query={query}"),
        "github" => format!("https://github.com/search?q={query}"),
        "reddit" => format!("https://www.reddit.com/search/?q={query}"),
        "mercadolibre" => format!("https://listado.mercadolibre.com.ar/{query}"),
        "wikipedia" => format!("https://es.wikipedia.org/w/index.php?search={query}"),
        "spotify" => format!("https://open.spotify.com/search/{query}"),
        "steam" => format!("https://store.steampowered.com/search/?term={query}"),
        "bing" => format!("https://www.bing.com/search?q={query}"),
        "duckduckgo" => format!("https://duckduckgo.com/?q={query}"),
        _ => format!(
            "https://www.google.com/search?q=site%3A{}%20{query}",
            urlencoding::encode(&site)
        ),
    })
}

pub fn scoped_search(text: &str) -> Option<Value> {
    let m = capture(
        r"(?i)^(?:busc[aá]|buscar|buscame|búscame|search|look\s+up)\s+(?:en|on|in)\s+(\S+)\s+(.+)$",
        text.trim(),
    )?;
    Some(json!({"action":"search","site":m[1].trim(),"query":m[2].trim()}))
}

pub fn detect(text: &str) -> Option<Value> {
    let text = text.trim();
    if let Some(action) = scoped_search(text) {
        return Some(action);
    }
    let lower = text.to_lowercase();
    let simple = match lower.as_str() {
        "ip" | "mi ip" | "my ip" | "local ip" | "cuál es mi ip" | "cual es mi ip" => "get_ip",
        "comprimir" | "comprimí" | "comprimi" | "comprimir video" | "comprimí video"
        | "compress" | "compress video" | "quiero comprimir" | "achicar video" => {
            "open_compressor_window"
        }
        "cancelar apagado" | "cancela el apagado" | "cancel shutdown" => "cancel_shutdown",
        "reiniciar explorer"
        | "reinicia explorer"
        | "reiniciar explorador"
        | "restart explorer" => "restart_explorer",
        "reiniciar wifi" | "reinicia wifi" | "reiniciar wi-fi" | "restart wifi" => "restart_wifi",
        "reiniciar bluetooth" | "reinicia bluetooth" | "restart bluetooth" => "restart_bluetooth",
        "memoria" | "qué sabes de mí" | "que sabes de mi" | "what do you know about me" => {
            "knowledge_list"
        }
        "borrar toda la memoria" | "limpiar toda la memoria" | "clear all memory" => {
            "knowledge_clear"
        }
        "mi rango" | "mi rango lol" | "mi elo" | "my lol rank" => "lol_rank",
        "mis partidas" | "mis partidas lol" | "my lol matches" => "lol_match_history",
        "apagar pc" | "apaga la pc" | "shutdown" | "shutdown pc" => {
            return Some(json!({"action":"shutdown","seconds":0}))
        }
        _ => "",
    };
    if !simple.is_empty() {
        return Some(json!({"action":simple}));
    }
    if let Some(m) = capture(
        r"(?i)^(?:apag[aá]|apagar|shutdown|shut\s+down)\s+(?:(?:la|the)\s+)?pc\s+(?:en|in)\s+(\d+)\s*(segundos?|seconds?|s|minutos?|minutes?|min|horas?|hours?|h)$",
        text,
    ) {
        let value: u64 = m[1].parse().ok()?;
        let unit = m[2].to_lowercase();
        let factor = if unit.starts_with('h') {
            3600
        } else if unit.starts_with('m') {
            60
        } else {
            1
        };
        return Some(json!({"action":"shutdown","seconds":value.checked_mul(factor)?}));
    }
    if let Some(m) = capture(
        r"(?i)^git\s+(init|status|log|branch|push|pull)(?:\s+(?:en|in)\s+(.+))?$",
        text,
    ) {
        let mut action = json!({"action":format!("git_{}",m[1].to_lowercase())});
        if let Some(path) = m.get(2) {
            action["path"] = json!(path.as_str());
        }
        return Some(action);
    }
    if let Some(m) = capture(r"(?i)^git\s+add(?:\s+(.+))?$", text) {
        return Some(json!({"action":"git_add","files":m.get(1).map_or(".",|m|m.as_str())}));
    }
    if let Some(m) = capture(r"(?i)^git\s+commit\s+(.+)$", text) {
        return Some(json!({"action":"git_commit","message":m[1].trim()}));
    }
    if let Some(m) = capture(r"(?i)^git\s+remote\s+add\s+(\S+)\s+(\S+)$", text) {
        return Some(json!({"action":"git_remote_add","name":m[1].trim(),"url":m[2].trim()}));
    }
    if let Some(m) = capture(
        r"(?i)^(?:record[aá]|recuerda|guardar|remember|save)\s+(?:que\s+)?(.+?)\s+(?:es|=)\s+(.+)$",
        text,
    ) {
        return Some(
            json!({"action":"knowledge_save_manual","key":m[1].trim(),"value":m[2].trim()}),
        );
    }
    if let Some(m) = capture(
        r"(?i)^(?:abr[ií]|abre|abrir|open)\s+(?:(?:la|the)\s+)?(?:carpeta\s+|folder\s+)?(escritorio|desktop|descargas|downloads|documentos|documents|inicio|home)$",
        text,
    ) {
        return Some(json!({"action":"open_folder","folder":m[1].to_lowercase()}));
    }
    if let Some(m) = capture(
        r"(?i)^(?:comprim[ií]|comprimir|compress)\s+(?:video\s*:\s*)?(.+\.(?:mp4|mkv|mov|avi|webm))(?:\s+(?:a|to)\s+(\d+)\s*mb)?$",
        text,
    ) {
        let mut action = json!({"action":"compress_for_discord","file":m[1].trim()});
        if let Some(size) = m.get(2) {
            action["targetSizeMb"] = json!(size.as_str().parse::<u64>().ok()?);
        }
        return Some(action);
    }
    for (pattern, name, field) in [
        (
            r"(?i)^(?:busc[aá]|buscar|buscame|búscame|search|look\s+up)\s+(.+)$",
            "search",
            "query",
        ),
        (
            r"(?i)^(?:pon[eé]|pon|poner|reproduce|play)\s+(.+)$",
            "play_music",
            "query",
        ),
        (
            r"(?i)^(?:abr[ií]|abre|abrir|open)\s+(https?://\S+)$",
            "open_url",
            "url",
        ),
        (
            r"(?i)^(?:abr[ií]|abre|abrir|open)\s+(.+)$",
            "open_app",
            "app",
        ),
        (
            r"(?i)^(?:olvida|olvid[aá]|forget)\s+(.+)$",
            "knowledge_delete_by_text",
            "text",
        ),
    ] {
        if let Some(m) = capture(pattern, text) {
            return Some(json!({"action":name,field:m[1].trim()}));
        }
    }
    // Explicit catalog syntax also covers commands without a natural-language alias.
    if text.starts_with('{') {
        return serde_json::from_str(text).ok();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn offline_commands_preserve_parameters_and_do_not_match_negations() {
        assert_eq!(detect("ip").unwrap()["action"], "get_ip");
        assert_eq!(detect("abrí Discord").unwrap()["app"], "Discord");
        assert_eq!(detect("git status").unwrap()["action"], "git_status");
        assert_eq!(detect("apaga la pc en 2 minutos").unwrap()["seconds"], 120);
        for text in ["no abras Discord", "qué significa apagar pc", "hola"] {
            assert!(detect(text).is_none());
        }
    }
    #[test]
    fn scoped_search_preserves_site_query_and_encoding() {
        let action = detect("busca en yt Rust & C++").unwrap();
        assert_eq!(action["site"], "yt");
        assert_eq!(action["query"], "Rust & C++");
        assert_eq!(
            search_url("yt", "Rust & C++").unwrap(),
            "https://www.youtube.com/results?search_query=Rust%20%26%20C%2B%2B"
        );
        assert!(search_url("github", "Rust")
            .unwrap()
            .starts_with("https://github.com/search"));
        assert!(search_url("example.com", "hi")
            .unwrap()
            .contains("site%3Aexample.com"));
        assert!(normalize_site("unknown-place").is_err());
        assert!(normalize_site("javascript:alert(1)").is_err());
    }
}
