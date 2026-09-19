use crate::research::types::{
    Document, DocumentStatus, DiscoveredLink, LinkId, SourceId,
};
use serde_json::Value;
use std::time::Duration;

const API_BASE: &str = "https://api.github.com";

/// Resultado de una operación GitHub.
pub enum GitHubResult {
    Repository {
        doc: Document,
        readme: Option<Document>,
        releases: Vec<Document>,
        issues: Vec<Document>,
    },
    Error(String),
}

/// Lee un repositorio GitHub con su contenido principal.
pub async fn read_repository(
    source_id: &SourceId,
    url: &str,
    resource: &str,
    filters: Option<&str>,
) -> GitHubResult {
    let client = match reqwest::Client::builder()
        .user_agent("NeekoAssistant/1.0")
        .timeout(Duration::from_secs(12))
        .build()
    {
        Ok(c) => c,
        Err(e) => return GitHubResult::Error(e.to_string()),
    };

    let owner_repo = extract_owner_repo(url);
    let (owner, repo) = match owner_repo {
        Some((o, r)) => (o, r),
        None => return GitHubResult::Error("URL de GitHub inválida".into()),
    };

    // Fetch repo metadata
    let repo_doc = match fetch_repo_meta(&client, &owner, &repo, source_id).await {
        Ok(doc) => doc,
        Err(e) => return GitHubResult::Error(e),
    };

    let mut readme_doc = None;
    let mut releases = Vec::new();
    let mut issues = Vec::new();

    let fetch_all = matches!(resource, "repository");
    if fetch_all || resource == "readme" {
        if let Ok(doc) = fetch_readme(&client, &owner, &repo, source_id).await {
            readme_doc = Some(doc);
        }
    }
    if fetch_all || resource == "releases" {
        if let Ok(mut docs) = fetch_releases(&client, &owner, &repo, source_id, filters).await {
            releases.append(&mut docs);
        }
    }
    if fetch_all || resource == "issues" {
        if let Ok(mut docs) = fetch_issues(&client, &owner, &repo, source_id, filters).await {
            issues.append(&mut docs);
        }
    }

    GitHubResult::Repository {
        doc: repo_doc,
        readme: readme_doc,
        releases,
        issues,
    }
}

fn extract_owner_repo(url: &str) -> Option<(String, String)> {
    let url = reqwest::Url::parse(url).ok()?;
    let mut segments: Vec<&str> = url.path_segments()?.collect();
    segments.retain(|s| !s.is_empty());
    if segments.len() >= 2 {
        Some((segments[0].to_string(), segments[1].to_string()))
    } else {
        None
    }
}

async fn fetch_repo_meta(
    client: &reqwest::Client,
    owner: &str,
    repo: &str,
    source_id: &SourceId,
) -> Result<Document, String> {
    let data: Value = client
        .get(format!("{API_BASE}/repos/{owner}/{repo}"))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let name = data["full_name"].as_str().unwrap_or("unknown");
    let description = data["description"].as_str().unwrap_or("");
    let html_url = data["html_url"].as_str().unwrap_or("");
    let homepage = data["homepage"].as_str().unwrap_or("");
    let updated_at = data["updated_at"].as_str().map(str::to_owned);
    let _pushed_at = data["pushed_at"].as_str().map(str::to_owned);
    let stars = data["stargazers_count"].as_u64().unwrap_or(0);
    let forks = data["forks_count"].as_u64().unwrap_or(0);
    let topics: Vec<String> = data["topics"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
        .unwrap_or_default();

    let mut links = Vec::new();
    if !homepage.is_empty() {
        links.push(DiscoveredLink {
            link_id: LinkId::new(source_id, 0),
            source_id: source_id.clone(),
            url: homepage.to_string(),
            anchor_text: "Website".to_string(),
        });
    }

    let readme_url = format!("https://github.com/{owner}/{repo}#readme");
    links.push(DiscoveredLink {
        link_id: LinkId::new(source_id, links.len()),
        source_id: source_id.clone(),
        url: readme_url,
        anchor_text: "README".to_string(),
    });

    let releases_url = format!("https://github.com/{owner}/{repo}/releases");
    links.push(DiscoveredLink {
        link_id: LinkId::new(source_id, links.len()),
        source_id: source_id.clone(),
        url: releases_url,
        anchor_text: "Releases".to_string(),
    });

    let body = format!(
        "{description}\n\nStars: {stars} | Forks: {forks}\nTopics: {}",
        if topics.is_empty() {
            "none".to_string()
        } else {
            topics.join(", ")
        }
    );

    Ok(Document {
        source_id: source_id.clone(),
        title: name.to_string(),
        url: html_url.to_string(),
        final_url: html_url.to_string(),
        domain: "github.com".to_string(),
        provider: "github".to_string(),
        status: DocumentStatus::ApiMetadata,
        http_status: Some(200),
        retrieved_at: crate::research::google::now_millis_pub(),
        published_at: None,
        modified_at: updated_at,
        read_scope: Default::default(),
        bytes_downloaded: 0,
        text_extracted_chars: body.len(),
        error: None,
        headers: vec![crate::research::types::Fragment {
            fragment_id: crate::research::types::FragmentId::new(source_id, 0),
            source_id: source_id.clone(),
            text: body,
            section: Some("metadata".to_string()),
        }],
        tables: Vec::new(),
        links,
    })
}

async fn fetch_readme(
    client: &reqwest::Client,
    owner: &str,
    repo: &str,
    source_id: &SourceId,
) -> Result<Document, String> {
    let data: Value = client
        .get(format!("{API_BASE}/repos/{owner}/{repo}/readme"))
        .header("accept", "application/vnd.github.v3.raw")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    // The raw readme comes as a JSON string or raw text
    let content = if let Some(s) = data.as_str() {
        s.to_string()
    } else {
        data.to_string()
    };

    let truncated = content.chars().take(6000).collect::<String>();

    Ok(Document {
        source_id: source_id.clone(),
        title: format!("{owner}/{repo} README"),
        url: format!("https://github.com/{owner}/{repo}#readme"),
        final_url: format!("https://github.com/{owner}/{repo}#readme"),
        domain: "github.com".to_string(),
        provider: "github".to_string(),
        status: DocumentStatus::ContentRead,
        http_status: Some(200),
        retrieved_at: crate::research::google::now_millis_pub(),
        published_at: None,
        modified_at: None,
        read_scope: Default::default(),
        bytes_downloaded: content.len(),
        text_extracted_chars: truncated.chars().count(),
        error: None,
        headers: vec![crate::research::types::Fragment {
            fragment_id: crate::research::types::FragmentId::new(source_id, 0),
            source_id: source_id.clone(),
            text: truncated,
            section: Some("readme".to_string()),
        }],
        tables: Vec::new(),
        links: Vec::new(),
    })
}

async fn fetch_releases(
    client: &reqwest::Client,
    owner: &str,
    repo: &str,
    source_id: &SourceId,
    filters: Option<&str>,
) -> Result<Vec<Document>, String> {
    let mut url = format!("{API_BASE}/repos/{owner}/{repo}/releases?per_page=5");
    if let Some(f) = filters {
        if f == "stable" {
            url.push_str("&prerelease=false");
        }
    }

    let data: Value = client
        .get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let items = data
        .as_array()
        .ok_or_else(|| "Respuesta GitHub inválida".to_string())?;

    let mut docs = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let tag = item["tag_name"].as_str().unwrap_or("unknown");
        let name = item["name"].as_str().unwrap_or(tag);
        let html_url = item["html_url"].as_str().unwrap_or("");
        let published = item["published_at"].as_str().map(str::to_owned);
        let created = item["created_at"].as_str().map(str::to_owned);
        let prerelease = item["prerelease"].as_bool().unwrap_or(false);
        let draft = item["draft"].as_bool().unwrap_or(false);
        let body = item["body"].as_str().unwrap_or("");

        let mut links = Vec::new();
        if let Some(download_url) = item["zipball_url"].as_str() {
            links.push(DiscoveredLink {
                link_id: LinkId::new(source_id, i),
                source_id: source_id.clone(),
                url: download_url.to_string(),
                anchor_text: format!("{tag} ZIP"),
            });
        }

        let status_text = format!(
            "Tag: {tag} | Prerelease: {prerelease} | Draft: {draft}\n{}",
            body.chars().take(3000).collect::<String>()
        );

        docs.push(Document {
            source_id: source_id.clone(),
            title: name.to_string(),
            url: html_url.to_string(),
            final_url: html_url.to_string(),
            domain: "github.com".to_string(),
            provider: "github".to_string(),
            status: DocumentStatus::ApiMetadata,
            http_status: Some(200),
            retrieved_at: crate::research::google::now_millis_pub(),
            published_at: published,
            modified_at: created,
            read_scope: Default::default(),
            bytes_downloaded: 0,
            text_extracted_chars: status_text.len(),
            error: None,
            headers: vec![crate::research::types::Fragment {
                fragment_id: crate::research::types::FragmentId::new(source_id, i),
                source_id: source_id.clone(),
                text: status_text,
                section: Some(format!("release {tag}")),
            }],
            tables: Vec::new(),
            links,
        });
    }

    Ok(docs)
}

async fn fetch_issues(
    client: &reqwest::Client,
    owner: &str,
    repo: &str,
    source_id: &SourceId,
    filters: Option<&str>,
) -> Result<Vec<Document>, String> {
    let state = if let Some(f) = filters {
        if f.contains("closed") {
            "closed"
        } else {
            "open"
        }
    } else {
        "open"
    };

    let data: Value = client
        .get(format!(
            "{API_BASE}/repos/{owner}/{repo}/issues?state={state}&per_page=5&sort=updated"
        ))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let items = data
        .as_array()
        .ok_or_else(|| "Respuesta GitHub inválida".to_string())?;

    let mut docs = Vec::new();
    for (i, item) in items.iter().enumerate() {
        // Skip pull requests (they appear in issues endpoint too)
        if item.get("pull_request").is_some() {
            continue;
        }
        let number = item["number"].as_u64().unwrap_or(0);
        let title = item["title"].as_str().unwrap_or("untitled");
        let html_url = item["html_url"].as_str().unwrap_or("");
        let state_str = item["state"].as_str().unwrap_or("unknown");
        let created = item["created_at"].as_str().map(str::to_owned);
        let updated = item["updated_at"].as_str().map(str::to_owned);
        let body = item["body"].as_str().unwrap_or("");
        let labels: Vec<String> = item["labels"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v["name"].as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();

        let issue_text = format!(
            "#{number} [{state_str}] {title}\nLabels: {}\n\n{}",
            if labels.is_empty() {
                "none".to_string()
            } else {
                labels.join(", ")
            },
            body.chars().take(3000).collect::<String>()
        );

        docs.push(Document {
            source_id: source_id.clone(),
            title: format!("#{number} {title}"),
            url: html_url.to_string(),
            final_url: html_url.to_string(),
            domain: "github.com".to_string(),
            provider: "github".to_string(),
            status: DocumentStatus::ApiMetadata,
            http_status: Some(200),
            retrieved_at: crate::research::google::now_millis_pub(),
            published_at: created,
            modified_at: updated,
            read_scope: Default::default(),
            bytes_downloaded: 0,
            text_extracted_chars: issue_text.len(),
            error: None,
            headers: vec![crate::research::types::Fragment {
                fragment_id: crate::research::types::FragmentId::new(source_id, i),
                source_id: source_id.clone(),
                text: issue_text,
                section: Some(format!("issue #{number}")),
            }],
            tables: Vec::new(),
            links: Vec::new(),
        });
    }

    Ok(docs)
}
