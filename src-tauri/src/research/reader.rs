use scraper::{Html, Selector};
use std::{
    collections::HashSet,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    time::Duration,
};

const MAX_BYTES: u64 = 1_048_576;
const MAX_TEXT: usize = 6_000;

/// Resultado de leer una página. Incluye fragmentos, enlaces y metadatos separados.
pub struct ReadResult {
    pub final_url: String,
    pub headers: Vec<String>,
    pub tables: Vec<String>,
    pub body_blocks: Vec<String>,
    pub links: Vec<ExtractedLink>,
    pub status: String,
    pub published_at: Option<String>,
    pub modified_at: Option<String>,
    pub bytes_downloaded: usize,
    pub text_extracted_chars: usize,
}

#[derive(Clone, Debug)]
pub struct ExtractedLink {
    pub href: String,
    pub anchor_text: String,
}

/// Lee una URL de forma segura con descarga incremental, DNS validado y extracción completa.
pub async fn read(url: &str, _focus: &str) -> Result<ReadResult, String> {
    let url = public_url(url)?;
    let client = reqwest::Client::builder()
        .user_agent("NeekoAssistant/1.0")
        .timeout(Duration::from_secs(12))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())?;

    let mut current_url = url;
    let mut data = Vec::new();
    let mut content_type = String::new();

    for _ in 0..4 {
        validate_dns(&current_url).await?;
        let response = match client.get(current_url.clone()).send().await {
            Ok(r) => r,
            Err(e) => return Err(format!("Error de red: {e}")),
        };
        if response.status().is_redirection() {
            if let Some(location) = response.headers().get(reqwest::header::LOCATION) {
                let next = reqwest::Url::parse(location.to_str().unwrap_or(""))
                    .or_else(|_| current_url.join(location.to_str().unwrap_or("")))
                    .map_err(|_| "Redirección inválida".to_string())?;
                let next = public_url(next.as_str())?;
                validate_dns(&next).await?;
                current_url = next;
                continue;
            }
        }
        if !response.status().is_success() {
            return Err(format!("La fuente devolvió HTTP {}", response.status()));
        }
        content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let mut stream = response;
        let mut total = 0u64;
        while let Some(chunk) = stream.chunk().await.map_err(|e| e.to_string())? {
            total += chunk.len() as u64;
            if total > MAX_BYTES {
                return Err("La página supera el límite de lectura.".into());
            }
            data.extend_from_slice(&chunk);
        }
        break;
    }

    if !content_type.contains("text/html")
        && !content_type.contains("text/plain")
        && !content_type.contains("application/json")
    {
        return Err("Formato no legible para investigación.".into());
    }

    let raw = String::from_utf8_lossy(&data);
    let extracted = if content_type.contains("text/html") {
        extract_html(&raw)
    } else {
        let text = raw.to_string();
        let blocks: Vec<String> = text.lines().map(str::to_owned).collect();
        HtmlExtract {
            headers: Vec::new(),
            tables: Vec::new(),
            body_blocks: blocks,
            links: Vec::new(),
        }
    };

    let published = published_at(&raw);
    let modified = modified_at(&raw);

    let text_chars: usize = extracted
        .headers
        .iter()
        .chain(extracted.tables.iter())
        .chain(extracted.body_blocks.iter())
        .map(|b| b.chars().count())
        .sum();

    Ok(ReadResult {
        final_url: current_url.to_string(),
        headers: extracted.headers,
        tables: extracted.tables,
        body_blocks: extracted.body_blocks,
        links: extracted.links,
        status: if text_chars > MAX_TEXT { "partial" } else { "full" }.into(),
        published_at: published,
        modified_at: modified,
        bytes_downloaded: data.len(),
        text_extracted_chars: text_chars,
    })
}

struct HtmlExtract {
    headers: Vec<String>,
    tables: Vec<String>,
    body_blocks: Vec<String>,
    links: Vec<ExtractedLink>,
}

fn extract_html(html: &str) -> HtmlExtract {
    let document = Html::parse_document(html);
    let root_selector = Selector::parse("article, main, [role=main], .content, .post, .entry")
        .unwrap();
    let root = document
        .select(&root_selector)
        .next()
        .unwrap_or(document.root_element());

    let excluded = [
        "nav", "header", "footer", "aside", "script", "style", "noscript", "form", "iframe",
    ];

    let mut seen = HashSet::new();
    let mut headers = Vec::new();
    let mut tables = Vec::new();
    let mut body_blocks = Vec::new();
    let mut links = Vec::new();

    // Extract headings
    let heading_sel = Selector::parse("h1, h2, h3, h4").unwrap();
    for node in root.select(&heading_sel) {
        if node.ancestors().filter_map(scraper::ElementRef::wrap).any(|a| {
            excluded.contains(&a.value().name())
                || a.value().attr("role").is_some_and(|r| {
                    matches!(r, "navigation" | "banner" | "contentinfo")
                })
        }) {
            continue;
        }
        let text: Vec<&str> = node.text().collect();
        let text = clean_text(text);
        if !text.is_empty() && seen.insert(text.clone()) {
            headers.push(text);
        }
    }

    // Extract tables
    let table_sel = Selector::parse("table").unwrap();
    for table in root.select(&table_sel) {
        if table.ancestors().filter_map(scraper::ElementRef::wrap).any(|a| {
            excluded.contains(&a.value().name())
                || a.value().attr("role").is_some_and(|r| {
                    matches!(r, "navigation" | "banner" | "contentinfo")
                })
        }) {
            continue;
        }
        let rows: Vec<String> = table
            .select(&Selector::parse("tr").unwrap())
            .map(|row| {
                let cells: Vec<String> = row
                    .select(&Selector::parse("td, th").unwrap())
                    .map(|cell| {
                        let text: Vec<&str> = cell.text().collect();
                        clean_text(text)
                    })
                    .filter(|s| !s.is_empty())
                    .collect();
                cells.join(" | ")
            })
            .filter(|s| !s.is_empty())
            .collect();
        if !rows.is_empty() {
            let text = rows.join("\n");
            if seen.insert(text.clone()) {
                tables.push(text);
            }
        }
    }

    // Extract body text blocks
    let block_sel = Selector::parse("p, li, td:not(:has(table)), pre, blockquote, dd, dt")
        .unwrap();
    for node in root.select(&block_sel) {
        if node.ancestors().filter_map(scraper::ElementRef::wrap).any(|a| {
            excluded.contains(&a.value().name())
                || a.value().attr("role").is_some_and(|r| {
                    matches!(r, "navigation" | "banner" | "contentinfo")
                })
                || a.value().attr("aria-hidden") == Some("true")
                || a.value().name() == "table"
        }) {
            continue;
        }
        // Skip nested blocks inside tables (already captured)
        if node.ancestors().filter_map(scraper::ElementRef::wrap).any(|a| a.value().name() == "table") {
            continue;
        }
        let text: Vec<&str> = node.text().collect();
        let text = clean_text(text);
        if !text.is_empty() && text.chars().count() > 10 && seen.insert(text.clone()) {
            body_blocks.push(text);
        }
    }

    // Extract links
    let link_sel = Selector::parse("a[href]").unwrap();
    for node in root.select(&link_sel) {
        if let Some(href) = node.value().attr("href") {
            if href.is_empty() || href.starts_with('#') || href.starts_with("javascript:") {
                continue;
            }
            let anchor = clean_text(node.text());
            if !anchor.is_empty() && anchor.len() > 2 {
                links.push(ExtractedLink {
                    href: href.to_string(),
                    anchor_text: anchor,
                });
            }
        }
    }

    HtmlExtract {
        headers,
        tables,
        body_blocks,
        links,
    }
}

fn clean_text<'a>(texts: impl IntoIterator<Item = &'a str>) -> String {
    texts
        .into_iter()
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn public_url(raw: &str) -> Result<reqwest::Url, String> {
    let url = reqwest::Url::parse(raw).map_err(|_| "URL inválida".to_string())?;
    let host = url.host_str().ok_or_else(|| "URL sin host".to_string())?;
    if !matches!(url.scheme(), "https" | "http")
        || !url.username().is_empty()
        || url.password().is_some()
        || host == "localhost"
        || host.ends_with(".local")
        || !host.contains('.')
    {
        return Err("URL no permitida".into());
    }
    if let Ok(ip) = host.trim_matches(['[', ']']).parse::<IpAddr>() {
        if private_ip(ip) {
            return Err("URL privada no permitida".into());
        }
    }
    Ok(url)
}

async fn validate_dns(url: &reqwest::Url) -> Result<(), String> {
    let host = url.host_str().ok_or_else(|| "URL sin host".to_string())?;
    let port = url.port_or_known_default().unwrap_or(443);
    let addrs = tokio::net::lookup_host((host, port))
        .await
        .map_err(|_| "No se pudo resolver la fuente".to_string())?;
    for addr in addrs {
        if private_ip(addr.ip()) {
            return Err("La fuente resuelve a una red privada".into());
        }
    }
    Ok(())
}

fn private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip == Ipv4Addr::UNSPECIFIED
        }
        IpAddr::V6(ip) => {
            ip.is_loopback()
                || ip.is_unspecified()
                || matches!(ip.segments()[0] & 0xfe00, 0xfc00 | 0xfe80)
                || ip == Ipv6Addr::LOCALHOST
        }
    }
}

fn published_at(html: &str) -> Option<String> {
    let document = Html::parse_document(html);
    let selector = Selector::parse(
        r#"meta[property="article:published_time"], meta[name="date"], meta[name="pubdate"], time[datetime]"#,
    )
    .ok()?;
    for node in document.select(&selector) {
        if let Some(value) = node.value().attr("content").or_else(|| node.value().attr("datetime"))
        {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.chars().take(64).collect());
            }
        }
    }
    None
}

fn modified_at(html: &str) -> Option<String> {
    let document = Html::parse_document(html);
    let selector = Selector::parse(
        r#"meta[property="article:modified_time"], meta[name="last-modified"], meta[name="updated"]"#,
    )
    .ok()?;
    for node in document.select(&selector) {
        if let Some(value) = node.value().attr("content") {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.chars().take(64).collect());
            }
        }
    }
    None
}

/// Resuelve un enlace relativo contra una URL base.
pub fn resolve_link(base: &str, href: &str) -> Option<String> {
    if href.starts_with("http://") || href.starts_with("https://") {
        return Some(href.to_string());
    }
    let base_url = reqwest::Url::parse(base).ok()?;
    let resolved = base_url.join(href).ok()?;
    Some(resolved.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_relative_links() {
        assert_eq!(
            resolve_link("https://example.org/docs/page", "../other").unwrap(),
            "https://example.org/other"
        );
        assert_eq!(
            resolve_link("https://example.org/a", "https://other.org/b").unwrap(),
            "https://other.org/b"
        );
    }

    #[test]
    fn private_ip_blocks() {
        assert!(private_ip("127.0.0.1".parse().unwrap()));
        assert!(private_ip("192.168.1.1".parse().unwrap()));
        assert!(!private_ip("8.8.8.8".parse().unwrap()));
    }
}
