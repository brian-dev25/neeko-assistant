use crate::research::types::{
    Document, DocumentStatus, DiscoveredLink, Fragment, FragmentId, LinkId, SourceId,
};
use serde_json::Value;
use std::time::Duration;

/// Resultado de una operación Reddit.
pub enum RedditResult {
    Post {
        doc: Document,
        comments: Vec<Document>,
    },
    Error(String),
}

/// Lee un post de Reddit con sus comentarios.
pub async fn read_post(
    source_id: &SourceId,
    url: &str,
    max_comments: usize,
    sort: &str,
) -> RedditResult {
    let client = match reqwest::Client::builder()
        .user_agent("NeekoAssistant/1.0")
        .timeout(Duration::from_secs(12))
        .build()
    {
        Ok(c) => c,
        Err(e) => return RedditResult::Error(e.to_string()),
    };

    let json_url = format_json_url(url);
    let data: Value = match client
        .get(&json_url)
        .query(&[("limit", "1"), ("sort", sort)])
        .send()
        .await
    {
        Ok(resp) => match resp.error_for_status() {
            Ok(r) => match r.json().await {
                Ok(v) => v,
                Err(e) => return RedditResult::Error(format!("JSON inválido: {e}")),
            },
            Err(e) => return RedditResult::Error(format!("HTTP: {e}")),
        },
        Err(e) => return RedditResult::Error(format!("Error de red: {e}")),
    };

    let listing = data
        .as_array()
        .and_then(|a| a.get(0))
        .and_then(|l| l["data"]["children"].as_array());
    let children = match listing {
        Some(c) => c,
        None => return RedditResult::Error("Reddit no devolvió datos".into()),
    };

    let post_data = children
        .first()
        .and_then(|c| c["data"].as_object());
    let post = match post_data {
        Some(p) => p,
        None => return RedditResult::Error("Post no encontrado".into()),
    };

    let title = post["title"].as_str().unwrap_or("untitled");
    let selftext = post["selftext"].as_str().unwrap_or("");
    let author = post["author"].as_str().unwrap_or("[deleted]");
    let subreddit = post["subreddit_name_prefixed"].as_str().unwrap_or("reddit");
    let permalink = post["permalink"].as_str().unwrap_or("");
    let score = post["score"].as_u64().unwrap_or(0);
    let num_comments = post["num_comments"].as_u64().unwrap_or(0);
    let created_utc = post["created_utc"].as_f64().map(|v| {
        let secs = v as u64;
        chrono::DateTime::from_timestamp(secs as i64, 0)
            .map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
            .unwrap_or_default()
    });

    let post_url = format!("https://www.reddit.com{permalink}");
    let body_text = format!(
        "r/{subreddit} | u/{author} | Score: {score} | Comments: {num_comments}\n\n{selftext}"
    );

    let mut links = Vec::new();
    links.push(DiscoveredLink {
        link_id: LinkId::new(source_id, 0),
        source_id: source_id.clone(),
        url: post_url.clone(),
        anchor_text: format!("{subreddit}: {title}"),
    });

    // Extract external links from selftext if any
    let url_regex = regex::Regex::new(r"https?://[^\s\)\]]+").ok();
    if let Some(re) = url_regex {
        for (_i, m) in re.find_iter(selftext).enumerate().take(5) {
            let found_url = m.as_str().to_string();
            if found_url != post_url && !found_url.contains("reddit.com") {
                links.push(DiscoveredLink {
                    link_id: LinkId::new(source_id, links.len()),
                    source_id: source_id.clone(),
                    url: found_url,
                    anchor_text: format!("Link from post"),
                });
            }
        }
    }

    let post_doc = Document {
        source_id: source_id.clone(),
        title: format!("{subreddit}: {title}"),
        url: post_url.clone(),
        final_url: post_url,
        domain: "reddit.com".to_string(),
        provider: "reddit".to_string(),
        status: DocumentStatus::ApiMetadata,
        http_status: Some(200),
        retrieved_at: crate::research::google::now_millis_pub(),
        published_at: created_utc,
        modified_at: None,
        read_scope: Default::default(),
        bytes_downloaded: 0,
        text_extracted_chars: body_text.len(),
        error: None,
        headers: vec![Fragment {
            fragment_id: FragmentId::new(source_id, 0),
            source_id: source_id.clone(),
            text: body_text,
            section: Some("post".to_string()),
        }],
        tables: Vec::new(),
        links,
    };

    // Fetch comments
    let comments = match fetch_comments(&client, source_id, permalink, max_comments).await {
        Ok(c) => c,
        Err(_) => Vec::new(),
    };

    RedditResult::Post {
        doc: post_doc,
        comments,
    }
}

async fn fetch_comments(
    client: &reqwest::Client,
    source_id: &SourceId,
    permalink: &str,
    max: usize,
) -> Result<Vec<Document>, String> {
    let url = format!("https://www.reddit.com{permalink}.json?limit={max}&sort=best");
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

    let listing = data
        .as_array()
        .and_then(|a| a.get(1))
        .and_then(|l| l["data"]["children"].as_array());

    let children = match listing {
        Some(c) => c,
        None => return Ok(Vec::new()),
    };

    let mut docs = Vec::new();
    for (i, child) in children.iter().enumerate().take(max) {
        if child["kind"].as_str() != Some("t1") {
            continue;
        }
        let comment = &child["data"];
        let author = comment["author"].as_str().unwrap_or("[deleted]");
        let body = comment["body"].as_str().unwrap_or("");
        let score = comment["score"].as_u64().unwrap_or(0);
        let created = comment["created_utc"].as_f64().map(|v| {
            let secs = v as u64;
            chrono::DateTime::from_timestamp(secs as i64, 0)
                .map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
                .unwrap_or_default()
        });
        let comment_permalink = comment["permalink"].as_str().unwrap_or("");

        let comment_url = format!("https://www.reddit.com{comment_permalink}");
        let comment_text = format!("u/{author} | Score: {score}\n\n{body}");

        docs.push(Document {
            source_id: source_id.clone(),
            title: format!("Comment by u/{author}"),
            url: comment_url.clone(),
            final_url: comment_url,
            domain: "reddit.com".to_string(),
            provider: "reddit".to_string(),
            status: DocumentStatus::ApiMetadata,
            http_status: Some(200),
            retrieved_at: crate::research::google::now_millis_pub(),
            published_at: created,
            modified_at: None,
            read_scope: Default::default(),
            bytes_downloaded: 0,
            text_extracted_chars: comment_text.len(),
            error: None,
            headers: vec![Fragment {
                fragment_id: FragmentId::new(source_id, i + 1),
                source_id: source_id.clone(),
                text: comment_text,
                section: Some(format!("comment by u/{author}")),
            }],
            tables: Vec::new(),
            links: Vec::new(),
        });
    }

    Ok(docs)
}

fn format_json_url(url: &str) -> String {
    if url.contains(".json") {
        url.to_string()
    } else if url.ends_with('/') {
        format!("{url}.json")
    } else {
        format!("{url}/.json")
    }
}
