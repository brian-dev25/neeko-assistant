//! Read-only encyclopedia answers. Source URLs come from the fetched page, never the model.
use serde_json::Value;
use std::time::Duration;

pub struct Query {
    term: String,
    birth: bool,
}

pub struct Answer {
    pub text: String,
    pub url: String,
}

pub fn query(text: &str) -> Option<Query> {
    let text = text
        .trim()
        .trim_matches(|c| matches!(c, '?' | '¿' | '!' | '¡'))
        .trim();
    let pattern = regex::Regex::new(r"(?i)^(?:cu[aá]ndo naci[oó]|cu[aá]ndo muri[oó]|qui[eé]n (?:es|fue)|qu[eé] (?:es|son)|d[oó]nde naci[oó]|who (?:is|was)|what (?:is|are))\s+(.+)$").ok()?;
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
        birth: text.to_lowercase().starts_with("cuando naci")
            || text.to_lowercase().starts_with("cuándo naci"),
    })
}

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
            Some(captures) => format!("{title} nació el {}.", &captures[1]),
            None => first_sentence(extract),
        }
    } else {
        first_sentence(extract)
    };
    Some(Answer {
        text,
        url: url.into(),
    })
}

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

pub async fn lookup(query: &Query, english: bool) -> Result<Answer, String> {
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
        assert_eq!(query("¿Cuándo nació Perón?").unwrap().term, "Perón");
        assert!(query("que es la fotosintesis").is_some());
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
    fn birth_answer_uses_fetched_content_and_canonical_source() {
        let query = query("cuando nacio peron").unwrap();
        let mut data = json!({"query":{"pages":{"1":{"title":"Juan Domingo Perón", "extract":"Juan Domingo Perón (Lobos, 8 de octubre de 1895 ‑ Olivos, 1 de julio de 1974) fue un político argentino.", "fullurl":"https://es.wikipedia.org/wiki/Juan_Domingo_Per%C3%B3n"}}}});
        let result = answer(&data, &query, "es").unwrap();
        assert_eq!(
            result.text,
            "Juan Domingo Perón nació el 8 de octubre de 1895."
        );
        assert!(result.url.starts_with("https://es.wikipedia.org/wiki/"));
        data["query"]["pages"]["1"]["fullurl"] = json!("https://example.com/wiki/fake");
        assert!(answer(&data, &query, "es").is_none());
        assert!(answer(&json!({}), &query, "es").is_none());
    }

    #[tokio::test]
    #[ignore = "Requires Wikipedia network access"]
    async fn live_birth_lookup() {
        let result = lookup(&query("cuando nacio peron").unwrap(), false)
            .await
            .unwrap();
        assert!(
            result.text.contains("8 de octubre de 1895"),
            "{}",
            result.text
        );
        assert!(result.url.starts_with("https://es.wikipedia.org/wiki/"));
    }
}
