//! Read-only web answers with source URLs from actual search results.
pub mod agent;
mod cache;
pub mod google;
pub mod progress;
pub mod prompts;
pub mod providers;
pub mod reader;
pub mod tools;
pub mod types;
pub mod verification;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::time::Duration;

#[cfg(test)]
pub struct Query {
    term: String,
    birth: bool,
    death_age: bool,
    search: String,
}

#[cfg(test)]
pub struct Answer {
    pub text: String,
    pub url: String,
    pub sources: Vec<Source>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Source {
    pub source_id: String,
    pub title: String,
    pub url: String,
    pub domain: String,
    pub source_type: String,
    pub snippet: String,
    pub provider: String,
    pub retrieved_at: String,
    pub read_status: String,
    #[serde(default)]
    pub published_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ResearchSearch {
    pub query: String,
    #[serde(default = "default_scope")]
    pub scope: String,
}

fn default_scope() -> String {
    "web".into()
}

#[cfg(test)]
pub fn query(text: &str) -> Option<Query> {
    let text = text
        .trim()
        .trim_matches(|c| matches!(c, '?' | '¿' | '!' | '¡'))
        .trim();
    if text.chars().count() > 500 {
        return None;
    }
    if let Some(action) = crate::local_commands::scoped_search(text) {
        if crate::local_commands::normalize_site(action["site"].as_str()?).ok()? != "google" {
            return None;
        }
    }
    let explicit = regex::Regex::new(r"(?i)^(?:busc[aá]|buscar|buscame|búscame|search|look\s+up|investiga)\s+(?:(?:en|on|in)\s+(?:google|internet|la web|the web)\s+)?(.+)$").ok()?;
    if let Some(capture) = explicit.captures(text) {
        let term = capture[1].trim();
        if term.len() < 2 || term.chars().any(char::is_control) {
            return None;
        }
        if let Some(mut query) = query(term) {
            query.search = term.into();
            return Some(query);
        }
        return Some(Query {
            term: term.into(),
            birth: false,
            death_age: false,
            search: term.into(),
        });
    }
    let pattern = regex::Regex::new(r"(?i)^(?:cu[aá]ndo naci[oó]|cu[aá]ndo muri[oó]|a qu[eé] edad muri[oó]|qu[eé] edad ten[ií]a cuando muri[oó]|cu[aá]ndo se (?:cre[oó]|invent[oó]|descubri[oó])|qu[eé] d[ií]a muri[oó]|en qu[eé] fecha muri[oó]|qui[eé]n (?:es|fue)|qu[eé] (?:es|son)|d[oó]nde naci[oó]|who (?:is|was)|what (?:is|are)|when was .+ (?:created|invented|discovered))\s+(.+)$").ok()?;
    let captures = pattern.captures(text)?;
    let term = captures[1].trim().trim_end_matches(['?', '.', '!']).trim();
    if term.len() < 2 || term.len() > 200 || term.contains(['\n', '#', '@', ':']) {
        return None;
    }
    let first = term.split_whitespace().next()?.to_lowercase();
    if ["mi", "mis", "tu", "tus", "my", "your"].contains(&first.as_str()) {
        return None;
    }
    Some(Query {
        term: term.into(),
        search: text.into(),
        birth: text.to_lowercase().starts_with("cuando naci")
            || text.to_lowercase().starts_with("cuándo naci"),
        death_age: text.to_lowercase().starts_with("a que edad murio")
            || text.to_lowercase().starts_with("a qué edad murió")
            || text.to_lowercase().starts_with("que edad tenia cuando murio")
            || text.to_lowercase().starts_with("qué edad tenía cuando murió"),
    })
}

pub fn scoped_terms(query: &str, scope: &str) -> String {
    let query = query.trim();
    match scope.trim().to_lowercase().as_str() {
        "wikipedia" => format!("site:wikipedia.org {query}"),
        "wikidata" => format!("site:wikidata.org {query}"),
        "reddit" => format!("site:reddit.com {query}"),
        "github" => format!("site:github.com {query}"),
        "stackoverflow" => format!("site:stackoverflow.com OR site:stackexchange.com {query}"),
        "youtube" => format!("site:youtube.com OR site:youtu.be {query}"),
        "news" => format!("{query} Reuters OR AP OR BBC"),
        "docs" | "official" => format!("{query} official documentation"),
        "reviews" => format!("{query} review OR benchmark OR user experience"),
        _ => query.into(),
    }
}

pub async fn search_sources(
    searches: &[ResearchSearch],
    intent: &str,
    english: bool,
) -> Result<Vec<Source>, String> {
    let cache_key = cache::key(searches, intent, english);
    if let Some(sources) = cache::get(&cache_key) {
        return Ok(sources);
    }
    let mut all = Vec::new();
    let mut seen = HashSet::new();
    for search in searches.iter().take(3) {
        let query = search.query.trim();
        if query.len() < 2 || query.chars().count() > 500 || query.chars().any(char::is_control) {
            continue;
        }
        let scope = search.scope.trim().to_lowercase();
        let results = match scope.as_str() {
            "wikipedia" => match wikipedia_sources(query, english).await {
                Ok(sources) => sources,
                Err(_) => google::search(query, &scope, english, 5).await?,
            },
            "github" => match github_sources(query).await {
                Ok(sources) => sources,
                Err(_) => google::search(query, &scope, english, 5).await?,
            },
            "reddit" => match reddit_sources(query).await {
                Ok(sources) => sources,
                Err(_) => google::search(query, &scope, english, 5).await?,
            },
            _ => google::search(query, &scope, english, 5).await?,
        };
        for mut source in results {
            if !seen.insert(source.url.clone()) {
                continue;
            }
            source.source_id = format!("s{}", all.len() + 1);
            all.push(source);
            if all.len() >= 8 {
                cache::put(cache_key, all.clone(), intent);
                return Ok(all);
            }
        }
    }
    if all.is_empty() {
        Err("No se encontraron fuentes verificables.".into())
    } else {
        cache::put(cache_key, all.clone(), intent);
        Ok(all)
    }
}

pub async fn read_sources(sources: &mut [Source], question: &str) {
    for source in sources.iter_mut().take(3) {
        if matches!(source.source_type.as_str(), "Video") || source.read_status == "api_extract" {
            continue;
        }
        if let Ok(read) = reader::read(&source.url, question).await {
            let excerpt = read
                .headers
                .into_iter()
                .chain(read.tables)
                .chain(read.body_blocks)
                .collect::<Vec<_>>()
                .join("\n");
            if !excerpt.trim().is_empty() {
                source.snippet = excerpt;
                source.read_status = read.status;
                source.published_at = read.published_at;
            }
        }
    }
}

async fn wikipedia_sources(query: &str, english: bool) -> Result<Vec<Source>, String> {
    let language = if english { "en" } else { "es" };
    let client = reqwest::Client::builder()
        .user_agent("NeekoAssistant/1.0")
        .timeout(Duration::from_secs(12))
        .build()
        .map_err(|e| e.to_string())?;
    let data: Value = client
        .get(format!("https://{language}.wikipedia.org/w/api.php"))
        .query(&[
            ("action", "query"),
            ("format", "json"),
            ("generator", "search"),
            ("gsrsearch", query),
            ("gsrlimit", "3"),
            ("gsrnamespace", "0"),
            ("prop", "extracts|info|pageprops"),
            ("exintro", "1"),
            ("explaintext", "1"),
            ("exchars", "900"),
            ("inprop", "url"),
            ("ppprop", "disambiguation"),
            ("redirects", "1"),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let pages = data["query"]["pages"]
        .as_object()
        .ok_or_else(|| "Wikipedia no devolvio resultados.".to_string())?;
    let mut sources = Vec::new();
    for page in pages.values() {
        if page.get("missing").is_some() || page["pageprops"].get("disambiguation").is_some() {
            continue;
        }
        let Some(title) = page["title"].as_str() else { continue };
        let Some(url) = page["fullurl"].as_str() else { continue };
        let Some(extract) = page["extract"].as_str() else { continue };
        sources.push(Source {
            source_id: format!("s{}", sources.len() + 1),
            title: title.into(),
            url: url.into(),
            domain: format!("{language}.wikipedia.org"),
            source_type: "Encyclopedia".into(),
            snippet: extract.to_owned(),
            provider: "wikipedia".into(),
            retrieved_at: now_millis(),
            read_status: "api_extract".into(),
            published_at: None,
        });
    }
    if sources.is_empty() {
        Err("Wikipedia no devolvio resultados verificables.".into())
    } else {
        Ok(sources)
    }
}

async fn github_sources(query: &str) -> Result<Vec<Source>, String> {
    let client = reqwest::Client::builder()
        .user_agent("NeekoAssistant/1.0")
        .timeout(Duration::from_secs(12))
        .build()
        .map_err(|e| e.to_string())?;
    let data: Value = client
        .get("https://api.github.com/search/repositories")
        .query(&[("q", query), ("per_page", "3")])
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let items = data["items"]
        .as_array()
        .ok_or_else(|| "GitHub no devolvio resultados.".to_string())?;
    let mut sources = Vec::new();
    for item in items {
        let Some(name) = item["full_name"].as_str() else { continue };
        let Some(url) = item["html_url"].as_str() else { continue };
        let description = item["description"].as_str().unwrap_or("");
        let updated = item["updated_at"].as_str().map(str::to_owned);
        sources.push(Source {
            source_id: format!("s{}", sources.len() + 1),
            title: name.into(),
            url: url.into(),
            domain: "github.com".into(),
            source_type: "Repository".into(),
            snippet: description.chars().take(900).collect(),
            provider: "github".into(),
            retrieved_at: now_millis(),
            read_status: "api_extract".into(),
            published_at: updated,
        });
    }
    if sources.is_empty() {
        Err("GitHub no devolvio resultados verificables.".into())
    } else {
        Ok(sources)
    }
}

async fn reddit_sources(query: &str) -> Result<Vec<Source>, String> {
    let client = reqwest::Client::builder()
        .user_agent("NeekoAssistant/1.0")
        .timeout(Duration::from_secs(12))
        .build()
        .map_err(|e| e.to_string())?;
    let data: Value = client
        .get("https://www.reddit.com/search.json")
        .query(&[("q", query), ("limit", "3"), ("sort", "relevance")])
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let children = data["data"]["children"]
        .as_array()
        .ok_or_else(|| "Reddit no devolvio resultados.".to_string())?;
    let mut sources = Vec::new();
    for item in children {
        let data = &item["data"];
        let Some(title) = data["title"].as_str() else { continue };
        let Some(permalink) = data["permalink"].as_str() else { continue };
        let body = data["selftext"].as_str().unwrap_or("");
        let subreddit = data["subreddit_name_prefixed"].as_str().unwrap_or("reddit");
        sources.push(Source {
            source_id: format!("s{}", sources.len() + 1),
            title: format!("{subreddit}: {title}"),
            url: format!("https://www.reddit.com{permalink}"),
            domain: "reddit.com".into(),
            source_type: "Community".into(),
            snippet: if body.trim().is_empty() {
                title.chars().take(900).collect()
            } else {
                body.chars().take(900).collect()
            },
            provider: "reddit".into(),
            retrieved_at: now_millis(),
            read_status: "api_extract".into(),
            published_at: data["created_utc"].as_f64().map(|value| value.to_string()),
        });
    }
    if sources.is_empty() {
        Err("Reddit no devolvio resultados verificables.".into())
    } else {
        Ok(sources)
    }
}

fn now_millis() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis().to_string())
        .unwrap_or_else(|_| "0".into())
}

#[cfg(test)]
fn answer(data: &Value, query: &Query, language: &str) -> Option<Answer> {
    let page = data["query"]["pages"].as_object()?.values().next()?;
    if page.get("missing").is_some() || page["pageprops"].get("disambiguation").is_some() {
        return None;
    }
    let title = page["title"].as_str()?;
    let extract = page["extract"].as_str()?.trim();
    if extract.is_empty() {
        return None;
    }
    let url = page["fullurl"].as_str()?;
    let parsed = reqwest::Url::parse(url).ok()?;
    let host = format!("{language}.wikipedia.org");
    if parsed.scheme() != "https"
        || parsed.host_str() != Some(host.as_str())
        || !parsed.path().starts_with("/wiki/")
    {
        return None;
    }
    let text = if query.birth {
        // Only take a birth date from the biographical parenthesis at the start.
        let date =
            regex::Regex::new(r"^[^(]{1,160}\([^)]*?(\d{1,2} de [a-záéíóú]+ de \d{4})").ok()?;
        match date.captures(extract) {
            Some(captures) => format!("{title} nacio el {}.", &captures[1]),
            None => first_sentence(extract),
        }
    } else if query.death_age {
        match explicit_death_age(extract) {
            Some(age_text) => format!("{title} murio {age_text}."),
            None => first_sentence(extract),
        }
    } else {
        first_sentence(extract)
    };
    let source = Source {
        source_id: "s1".into(),
        title: title.into(),
        url: url.into(),
        domain: format!("{language}.wikipedia.org"),
        source_type: "Encyclopedia".into(),
        snippet: extract.to_owned(),
        provider: "wikipedia".into(),
        retrieved_at: now_millis(),
        read_status: "api_extract".into(),
        published_at: None,
    };
    Some(Answer {
        text,
        url: source.url.clone(),
        sources: vec![source],
    })
}

#[cfg(test)]
fn explicit_death_age(extract: &str) -> Option<String> {
    let spanish = regex::Regex::new(r"(?i)\b(a los \d{1,3} a(?:n|ñ)os|a la edad de \d{1,3} a(?:n|ñ)os)\b").ok()?;
    if let Some(captures) = spanish.captures(extract) {
        return Some(captures.get(1)?.as_str().replace('ñ', "n"));
    }
    let english = regex::Regex::new(r"(?i)\baged (\d{1,3})\b").ok()?;
    english
        .captures(extract)
        .and_then(|captures| captures.get(1).map(|age| format!("a los {} anos", age.as_str())))
}

#[cfg(test)]
fn first_sentence(extract: &str) -> String {
    let paragraph = extract.lines().next().unwrap_or(extract);
    let sentence = paragraph
        .split_once(". ")
        .map(|(s, _)| format!("{s}."))
        .unwrap_or_else(|| paragraph.into());
    if sentence.chars().count() > 600 {
        format!("{}…", sentence.chars().take(600).collect::<String>())
    } else {
        sentence
    }
}

#[cfg(test)]
async fn wikipedia_lookup(query: &Query, english: bool) -> Result<Answer, String> {
    let language = if english { "en" } else { "es" };
    let client = reqwest::Client::builder()
        .user_agent("NeekoAssistant/1.0 (https://github.com/brian-dev25/neeko-assistant)")
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;
    let data: Value = client
        .get(format!("https://{language}.wikipedia.org/w/api.php"))
        .query(&[
            ("action", "query"),
            ("format", "json"),
            ("generator", "search"),
            ("gsrsearch", query.term.as_str()),
            ("gsrlimit", "1"),
            ("gsrnamespace", "0"),
            ("prop", "extracts|info|pageprops"),
            ("exintro", "1"),
            ("explaintext", "1"),
            ("exchars", "2000"),
            ("inprop", "url"),
            ("ppprop", "disambiguation"),
            ("redirects", "1"),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    answer(&data, query, language)
        .ok_or_else(|| "No se encontró un artículo inequívoco con contenido.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn factual_queries_exclude_chat_and_private_questions() {
        assert_eq!(query("¿Cuándo nació Ada Lovelace?").unwrap().term, "Ada Lovelace");
        assert!(query("que es la fotosintesis").is_some());
        assert_eq!(query("cuando se creo la polvora").unwrap().term, "la polvora");
        assert_eq!(query("que dia murio Marie Curie?").unwrap().term, "Marie Curie");
        assert_eq!(
            query("busca en google que dia murio Marie Curie")
                .unwrap()
                .search,
            "que dia murio Marie Curie"
        );
        assert_eq!(
            query("busca en google telescopios espaciales")
                .unwrap()
                .search,
            "telescopios espaciales"
        );
        assert!(query("busca en youtube musica").is_none());
        for text in [
            "hola",
            "abrí Google",
            "quien es mi hermano",
            "que es mi token",
            "cuando nacio usuario#ABC",
        ] {
            assert!(query(text).is_none(), "{text}");
        }
    }

    #[test]
    fn death_age_answer_only_uses_explicit_source_text() {
        let query = query("a que edad murio Ada Lovelace").unwrap();
        let data = json!({"query":{"pages":{"1":{"title":"Ada Lovelace", "extract":"Ada Lovelace fue una matematica britanica; murio a los 36 anos segun esta fuente.", "fullurl":"https://es.wikipedia.org/wiki/Ada_Lovelace"}}}});
        let result = answer(&data, &query, "es").unwrap();
        assert_eq!(result.text, "Ada Lovelace murio a los 36 anos.");
        assert_eq!(result.sources[0].read_status, "api_extract");

        let data_without_age = json!({"query":{"pages":{"1":{"title":"Ada Lovelace", "extract":"Ada Lovelace (Londres, 10 de diciembre de 1815 - Marylebone, 27 de noviembre de 1852) fue una matematica y escritora britanica.", "fullurl":"https://es.wikipedia.org/wiki/Ada_Lovelace"}}}});
        let fallback = answer(&data_without_age, &query, "es").unwrap();
        assert_ne!(fallback.text, "Ada Lovelace murio a los 36 anos.");
    }

    #[test]
    fn birth_answer_uses_fetched_content_and_canonical_source() {
        let query = query("cuando nacio Ada Lovelace").unwrap();
        let mut data = json!({"query":{"pages":{"1":{"title":"Ada Lovelace", "extract":"Ada Lovelace (Londres, 10 de diciembre de 1815 ‑ Marylebone, 27 de noviembre de 1852) fue una matemática y escritora británica.", "fullurl":"https://es.wikipedia.org/wiki/Ada_Lovelace"}}}});
        let result = answer(&data, &query, "es").unwrap();
        assert_eq!(
            result.text,
            "Ada Lovelace nacio el 10 de diciembre de 1815."
        );
        assert!(result.url.starts_with("https://es.wikipedia.org/wiki/"));
        data["query"]["pages"]["1"]["fullurl"] = json!("https://example.com/wiki/fake");
        assert!(answer(&data, &query, "es").is_none());
        assert!(answer(&json!({}), &query, "es").is_none());
    }

    #[tokio::test]
    #[ignore = "Requires Wikipedia network access"]
    async fn live_birth_lookup() {
        let result = wikipedia_lookup(&query("cuando nacio Ada Lovelace").unwrap(), false)
            .await
            .unwrap();
        assert!(
            result.text.contains("10 de diciembre de 1815"),
            "{}",
            result.text
        );
        assert!(result.url.starts_with("https://es.wikipedia.org/wiki/"));
    }
}

