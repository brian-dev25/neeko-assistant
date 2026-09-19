//! Google results rendered with an isolated, temporary Edge profile. No user
//! cookies, browser session, local AI, or model-generated citations are used.
#[cfg(test)]
use super::Answer;
use super::Source;
use scraper::{ElementRef, Html, Selector};
use std::{path::PathBuf, time::Duration};
use tokio::io::AsyncReadExt;

const MAX_HTML: u64 = 2_000_000;
static BROWSER: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);

struct Profile(PathBuf);
impl Drop for Profile {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(windows)]
struct ProcessGroup(isize);
#[cfg(windows)]
impl ProcessGroup {
    fn attach(pid: u32) -> Result<Self, String> {
        use windows_sys::Win32::{
            Foundation::CloseHandle,
            System::{JobObjects::*, Threading::*},
        };
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return Err("No se pudo aislar el navegador de busqueda.".into());
            }
            let group = Self(job as isize);
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of_val(&info) as u32,
            ) == 0
            {
                return Err("No se pudo configurar el cierre del navegador.".into());
            }
            let process = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, pid);
            if process.is_null() {
                return Err("No se pudo controlar el navegador de busqueda.".into());
            }
            let assigned = AssignProcessToJobObject(job, process);
            CloseHandle(process);
            if assigned == 0 {
                return Err("No se pudo aislar el proceso de busqueda.".into());
            }
            Ok(group)
        }
    }
}
#[cfg(windows)]
impl Drop for ProcessGroup {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0 as _);
        }
    }
}

fn browser_path() -> Option<PathBuf> {
    ["PROGRAMFILES(X86)", "PROGRAMFILES", "LOCALAPPDATA"]
        .iter()
        .filter_map(std::env::var_os)
        .map(|root| PathBuf::from(root).join("Microsoft/Edge/Application/msedge.exe"))
        .find(|path| path.is_file())
}

async fn render(query: &str, english: bool) -> Result<String, String> {
    let _permit = tokio::time::timeout(Duration::from_secs(20), BROWSER.acquire())
        .await
        .map_err(|_| "Hay otra busqueda en curso. Intenta nuevamente.")?
        .map_err(|e| e.to_string())?;
    let browser = browser_path().ok_or("La busqueda Google necesita Microsoft Edge instalado.")?;
    let profile =
        Profile(std::env::temp_dir().join(format!("neeko-search-{}", crate::uuid_simple())));
    std::fs::create_dir(&profile.0).map_err(|e| e.to_string())?;
    let mut url = reqwest::Url::parse("https://www.google.com/search").unwrap();
    url.query_pairs_mut()
        .append_pair("q", query)
        .append_pair("hl", if english { "en" } else { "es" })
        .append_pair("udm", "14");
    let mut command = tokio::process::Command::new(browser);
    command
        .args([
            "--headless",
            "--disable-gpu",
            "--no-first-run",
            "--no-default-browser-check",
            "--disable-extensions",
            "--disable-background-networking",
            "--virtual-time-budget=5000",
            "--dump-dom",
        ])
        .arg(format!("--user-data-dir={}", profile.0.display()))
        .arg(url.as_str())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    let mut child = command
        .spawn()
        .map_err(|e| format!("No se pudo consultar Google: {e}"))?;
    #[cfg(windows)]
    let group = ProcessGroup::attach(child.id().ok_or("Navegador sin proceso")?)?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or("Navegador sin respuesta")?
        .take(MAX_HTML + 1);
    let mut data = Vec::new();
    let result = tokio::time::timeout(Duration::from_secs(20), async {
        let (status, _) = tokio::try_join!(child.wait(), stdout.read_to_end(&mut data))
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("Google no devolvio resultados legibles.".to_owned());
        }
        if data.len() as u64 > MAX_HTML {
            return Err("La respuesta de Google es demasiado grande.".to_owned());
        }
        Ok(String::from_utf8_lossy(&data).into_owned())
    })
    .await
    .map_err(|_| "Google agoto el tiempo de espera.".to_owned())
    .and_then(|v| v);
    #[cfg(windows)]
    drop(group);
    let _ = child.kill().await;
    let _ = child.wait().await;
    result
}

#[derive(Debug)]
struct ResultCard {
    title: String,
    snippet: String,
    href: String,
}

fn text(element: ElementRef<'_>) -> String {
    element
        .text()
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn cards(html: &str) -> Vec<ResultCard> {
    let document = Html::parse_document(html);
    let heading = Selector::parse("h3").unwrap();
    let snippets = Selector::parse(".VwiC3b, .IsZvec, [data-sncf], .aCOpRe, .st").unwrap();
    let mut results = Vec::new();
    for h in document.select(&heading) {
        let Some(link) = h
            .ancestors()
            .filter_map(ElementRef::wrap)
            .find(|e| e.value().name() == "a")
        else {
            continue;
        };
        let Some(href) = link.value().attr("href") else {
            continue;
        };
        if !(href.starts_with("https://")
            || href.starts_with("/url?")
            || href.starts_with("/goto?"))
        {
            continue;
        }
        // Stop before an ancestor spans multiple result titles. Never mix a
        // source URL with another result's text or with navigation/AI summaries.
        let snippet = link
            .ancestors()
            .filter_map(ElementRef::wrap)
            .take(9)
            .take_while(|e| e.select(&heading).count() <= 1)
            .find_map(|e| {
                e.select(&snippets)
                    .map(text)
                    .find(|s| s.chars().count() > 35)
            });
        if let Some(snippet) = snippet {
            results.push(ResultCard {
                title: text(h),
                snippet,
                href: href.into(),
            });
        }
        if results.len() >= 5 {
            break;
        }
    }
    results
}

fn public_source(raw: &str) -> Option<String> {
    let url = reqwest::Url::parse(raw).ok()?;
    let host = url.host_str()?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || host == "localhost"
        || host.ends_with(".local")
        || !host.contains('.')
        || host
            .trim_matches(['[', ']'])
            .parse::<std::net::IpAddr>()
            .is_ok()
        || host == "google.com"
        || host.ends_with(".google.com")
    {
        return None;
    }
    Some(url.into())
}

async fn source(client: &reqwest::Client, href: &str) -> Option<String> {
    if let Some(url) = public_source(href) {
        return Some(url);
    }
    if !(href.starts_with("/url?") || href.starts_with("/goto?")) {
        return None;
    }
    let url = reqwest::Url::parse(&format!("https://www.google.com{href}")).ok()?;
    for (name, value) in url.query_pairs() {
        if ["q", "url"].contains(&name.as_ref()) {
            if let Some(url) = public_source(&value) {
                return Some(url);
            }
        }
    }
    // Google may use opaque /goto tokens. Resolve only Google's first redirect;
    // do not fetch arbitrary destinations, local addresses, or page instructions.
    let response = client.get(url).send().await.ok()?;
    if !response.status().is_redirection() {
        return None;
    }
    public_source(
        response
            .headers()
            .get(reqwest::header::LOCATION)?
            .to_str()
            .ok()?,
    )
}

#[cfg(test)]
pub(super) async fn lookup(query: &str, english: bool) -> Result<Answer, String> {
    let sources = search(query, "web", english, 1).await?;
    let source = sources
        .first()
        .ok_or_else(|| "Google no devolvio resultados legibles.".to_string())?;
    let prefix = if english {
        "I searched Google. This source reports:"
    } else {
        "Busque en Google. Esta fuente indica:"
    };
    Ok(Answer {
        text: format!("{prefix}\n\n{}\n\n{}", source.snippet, source.title),
        url: source.url.clone(),
        sources,
    })
}
fn source_type(url: &str) -> String {
    let host = reqwest::Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .unwrap_or_default();
    let host = host.trim_start_matches("www.").to_lowercase();
    if host.ends_with("wikipedia.org") {
        "Encyclopedia"
    } else if host == "github.com" || host.ends_with(".github.com") {
        "Repository"
    } else if host == "reddit.com" || host.ends_with(".reddit.com") {
        "Community"
    } else if host.contains("stackoverflow.com") || host.ends_with("stackexchange.com") {
        "Forum"
    } else if host.contains("microsoft.com")
        || host.contains("docs.")
        || host.contains("developer.")
        || host.contains("learn.")
    {
        "Documentation"
    } else if ["reuters.com", "apnews.com", "bbc.com", "bbc.co.uk"]
        .iter()
        .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")))
    {
        "News"
    } else if [
        "techpowerup.com",
        "tomshardware.com",
        "anandtech.com",
        "rtings.com",
    ]
    .iter()
    .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")))
    {
        "Review"
    } else if host.ends_with("youtube.com") || host == "youtu.be" {
        "Video"
    } else {
        "Web"
    }
    .into()
}

fn domain(url: &str) -> String {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|url| {
            url.host_str()
                .map(|host| host.trim_start_matches("www.").to_string())
        })
        .unwrap_or_default()
}

pub fn now_millis() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis().to_string())
        .unwrap_or_else(|_| "0".into())
}

pub fn now_millis_pub() -> String {
    now_millis()
}

pub(super) async fn search(
    query: &str,
    scope: &str,
    english: bool,
    limit: usize,
) -> Result<Vec<Source>, String> {
    if query.trim().is_empty() || query.chars().count() > 500 || query.chars().any(char::is_control)
    {
        return Err("Busqueda invalida.".into());
    }
    let scoped = super::scoped_terms(query, scope);
    let html = render(&scoped, english).await?;
    let results = cards(&html);
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;
    let mut sources = Vec::new();
    for result in results {
        if let Some(url) = source(&client, &result.href).await {
            if sources.iter().any(|source: &Source| source.url == url) {
                continue;
            }
            let source_id = format!("s{}", sources.len() + 1);
            sources.push(Source {
                source_id,
                title: result.title.chars().take(180).collect(),
                url: url.clone(),
                domain: domain(&url),
                source_type: source_type(&url),
                snippet: result.snippet.chars().take(900).collect(),
                provider: "google".into(),
                retrieved_at: now_millis(),
                read_status: "snippet".into(),
                published_at: None,
            });
            if sources.len() >= limit {
                break;
            }
        }
    }
    if sources.is_empty() {
        Err("Google no devolvio resultados legibles o pidio una comprobacion. Proba de nuevo mas tarde.".into())
    } else {
        Ok(sources)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sources_stay_attached_to_their_own_snippet() {
        let html = r#"<div><div><a href="https://example.org/a"><h3>Article A</h3></a><div class="VwiC3b">This is the factual snippet belonging to article A.</div></div><div><a href="https://example.org/b"><h3>Article B</h3></a><div class="VwiC3b">This is the factual snippet belonging to article B.</div></div></div>"#;
        let results = cards(html);
        assert_eq!(results.len(), 2);
        assert!(results[0].snippet.ends_with("article A."));
        assert!(results[1].snippet.ends_with("article B."));
        assert_eq!(results[0].href, "https://example.org/a");
        for url in [
            "javascript:alert(1)",
            "https://user:pass@example.org",
            "http://127.0.0.1/",
            "https://www.google.com/search?q=x",
        ] {
            assert!(public_source(url).is_none());
        }
        assert!(cards("<h3>Consent</h3><script>invented answer</script>").is_empty());
    }
    #[tokio::test]
    #[ignore = "Uses Google and an isolated headless Edge profile"]
    async fn live_google_death_question() {
        let answer = lookup("que dia murio Marie Curie", false)
            .await
            .unwrap();
        assert!(answer.text.contains("1934"), "{}", answer.text);
        assert!(answer.text.starts_with("Busqué en Google"));
        assert!(public_source(&answer.url).is_some());
        eprintln!("{}\nSource: {}", answer.text, answer.url);
    }
}


