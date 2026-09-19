use crate::research::types::*;
use crate::research::{reader, ResearchSearch};
use std::time::Instant;

/// Error estructurado de una herramienta.
#[derive(Debug)]
#[allow(dead_code)]
pub struct ToolError {
    pub code: ErrorCode,
    pub message: String,
    pub provider: Option<String>,
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{:?}] {}", self.code, self.message)
    }
}

/// Resultado de ejecutar una herramienta.
pub enum ToolOutput {
    Success {
        documents: Vec<Document>,
        fragments: Vec<Fragment>,
        links: Vec<DiscoveredLink>,
    },
    Error(ToolError),
}

/// Valida un `ModelDecision` contra el estado actual y el presupuesto.
pub fn validate_decision(
    decision: &ModelDecision,
    state: &ResearchState,
) -> Result<(), ToolError> {
    match decision {
        ModelDecision::Search { query, .. } => {
            if query.len() < 2 || query.len() > 500 {
                return Err(ToolError {
                    code: ErrorCode::InvalidResponse,
                    message: "Consulta de búsqueda inválida (2-500 caracteres)".into(),
                    provider: None,
                });
            }
            if !state.has_search_budget() {
                return Err(ToolError {
                    code: ErrorCode::BudgetExhausted,
                    message: "Presupuesto de búsquedas agotado".into(),
                    provider: None,
                });
            }
        }
        ModelDecision::Read { source_id, .. } => {
            let id = SourceId::parse_id(source_id).ok_or_else(|| ToolError {
                code: ErrorCode::InvalidReference,
                message: format!("ID de fuente inválido: {source_id}"),
                provider: None,
            })?;
            if state.find_document(&id).is_none() {
                return Err(ToolError {
                    code: ErrorCode::InvalidReference,
                    message: format!("Fuente {source_id} no encontrada"),
                    provider: None,
                });
            }
            if !state.has_read_budget() {
                return Err(ToolError {
                    code: ErrorCode::BudgetExhausted,
                    message: "Presupuesto de lecturas agotado".into(),
                    provider: None,
                });
            }
        }
        ModelDecision::FollowLink { source_id, link_id, .. } => {
            let sid = SourceId::parse_id(source_id).ok_or_else(|| ToolError {
                code: ErrorCode::InvalidReference,
                message: format!("ID de fuente inválido: {source_id}"),
                provider: None,
            })?;
            let lid = LinkId(link_id.clone());
            if state.find_link(&sid, &lid).is_none() {
                return Err(ToolError {
                    code: ErrorCode::InvalidReference,
                    message: format!("Enlace {link_id} no encontrado en {source_id}"),
                    provider: None,
                });
            }
            if !state.has_read_budget() {
                return Err(ToolError {
                    code: ErrorCode::BudgetExhausted,
                    message: "Presupuesto de lecturas agotado".into(),
                    provider: None,
                });
            }
        }
        ModelDecision::GitHubRead { source_id, .. } => {
            let id = SourceId::parse_id(source_id).ok_or_else(|| ToolError {
                code: ErrorCode::InvalidReference,
                message: format!("ID de fuente inválido: {source_id}"),
                provider: None,
            })?;
            if state.find_document(&id).is_none() {
                return Err(ToolError {
                    code: ErrorCode::InvalidReference,
                    message: format!("Fuente {source_id} no encontrada"),
                    provider: None,
                });
            }
            if !state.has_read_budget() {
                return Err(ToolError {
                    code: ErrorCode::BudgetExhausted,
                    message: "Presupuesto de lecturas agotado".into(),
                    provider: None,
                });
            }
        }
        ModelDecision::RedditRead { source_id, .. } => {
            let id = SourceId::parse_id(source_id).ok_or_else(|| ToolError {
                code: ErrorCode::InvalidReference,
                message: format!("ID de fuente inválido: {source_id}"),
                provider: None,
            })?;
            if state.find_document(&id).is_none() {
                return Err(ToolError {
                    code: ErrorCode::InvalidReference,
                    message: format!("Fuente {source_id} no encontrada"),
                    provider: None,
                });
            }
            if !state.has_read_budget() {
                return Err(ToolError {
                    code: ErrorCode::BudgetExhausted,
                    message: "Presupuesto de lecturas agotado".into(),
                    provider: None,
                });
            }
        }
        ModelDecision::Finish { .. } => {}
    }
    Ok(())
}

/// Ejecuta una herramienta contra el estado.
pub async fn execute(
    decision: &ModelDecision,
    state: &mut ResearchState,
    english: bool,
) -> ToolOutput {
    match decision {
        ModelDecision::Search {
            query,
            strategy,
            domains,
            freshness,
        } => {
            execute_search(state, query, strategy, domains, freshness.as_deref(), english).await
        }
        ModelDecision::Read { source_id, focus } => {
            let id = SourceId::parse_id(source_id).unwrap_or_else(|| SourceId::new(0));
            execute_read(state, &id, focus.as_deref()).await
        }
        ModelDecision::FollowLink {
            source_id,
            link_id,
            focus,
        } => {
            let sid = SourceId::parse_id(source_id).unwrap_or_else(|| SourceId::new(0));
            let lid = LinkId(link_id.clone());
            execute_follow_link(state, &sid, &lid, focus.as_deref()).await
        }
        ModelDecision::GitHubRead {
            source_id,
            resource,
            filters,
        } => {
            let id = SourceId::parse_id(source_id).unwrap_or_else(|| SourceId::new(0));
            execute_github_read(state, &id, resource, filters.as_deref()).await
        }
        ModelDecision::RedditRead {
            source_id,
            max_comments,
            sort,
        } => {
            let id = SourceId::parse_id(source_id).unwrap_or_else(|| SourceId::new(0));
            execute_reddit_read(state, &id, max_comments.unwrap_or(5), sort.as_deref()).await
        }
        ModelDecision::Finish { .. } => ToolOutput::Success {
            documents: Vec::new(),
            fragments: Vec::new(),
            links: Vec::new(),
        },
    }
}

async fn execute_search(
    state: &mut ResearchState,
    query: &str,
    strategy: &str,
    domains: &[String],
    _freshness: Option<&str>,
    _english: bool,
) -> ToolOutput {
    state.counters.record_search();
    state.counters.record_operation();

    // A guessed brand domain can hide all valid results, especially with typos.
    let verified_domains: Vec<String> = domains.iter().filter(|domain|
        state.documents.iter().any(|d| d.domain.eq_ignore_ascii_case(domain))
    ).cloned().collect();
    let scope = strategy_to_scope(strategy, &verified_domains);
    let scoped_query = if verified_domains.is_empty() { query.to_string() }
        else { format!("site:{} {query}", verified_domains[0]) };

    let search = ResearchSearch {
        query: scoped_query,
        scope: scope.clone(),
    };

    let result = crate::research::search_sources(&[search], &state.intent.to_string(), _english).await;

    match result {
        Ok(sources) => {
            let new_count = sources.len();
            if !state.counters.can_add_candidates(&state.budget, new_count) {
                return ToolOutput::Error(ToolError {
                    code: ErrorCode::BudgetExhausted,
                    message: "Demasiados candidatos acumulados".into(),
                    provider: Some("search".into()),
                });
            }

            let mut documents = Vec::new();
            let fragments = Vec::new();
            let links = Vec::new();

            for source in sources {
                if state.documents.iter().any(|d| d.url == source.url) { continue; }
                let id = SourceId::new(state.documents.len() + documents.len() + 1);
                let doc = Document {
                    source_id: id.clone(),
                    title: source.title,
                    url: source.url.clone(),
                    final_url: source.url.clone(),
                    domain: source.domain,
                    provider: source.provider,
                    status: DocumentStatus::SearchSnippet,
                    http_status: None,
                    retrieved_at: source.retrieved_at,
                    published_at: source.published_at,
                    modified_at: None,
                    read_scope: ReadScope::Snippet,
                    bytes_downloaded: 0,
                    text_extracted_chars: source.snippet.len(),
                    error: None,
                    headers: vec![Fragment {
                        fragment_id: FragmentId::new(&id, 0),
                        source_id: id.clone(),
                        text: source.snippet,
                        section: None,
                    }],
                    tables: Vec::new(),
                    links: Vec::new(),
                };
                documents.push(doc);
            }
            state.counters.candidates += documents.len();

            ToolOutput::Success {
                documents,
                fragments,
                links,
            }
        }
        Err(e) => {
            if e.contains("No se encontraron") {
                state.counters.record_unproductive();
            }
            ToolOutput::Error(ToolError {
                code: ErrorCode::NoResults,
                message: e,
                provider: Some("search".into()),
            })
        }
    }
}

fn strategy_to_scope(strategy: &str, domains: &[String]) -> String {
    if !domains.is_empty() {
        return format!("domain:{}", domains[0]);
    }
    match strategy {
        "wikipedia" | "encyclopedia" => "wikipedia".into(),
        "github" | "repository" => "github".into(),
        "reddit" | "community" => "reddit".into(),
        "official" | "docs" | "documentation" => "official".into(),
        "news" => "news".into(),
        "reviews" => "reviews".into(),
        "stackoverflow" | "forum" => "stackoverflow".into(),
        "youtube" | "video" => "youtube".into(),
        _ => "web".into(),
    }
}

async fn execute_read(
    state: &mut ResearchState,
    source_id: &SourceId,
    focus: Option<&str>,
) -> ToolOutput {
    state.counters.record_read();
    state.counters.record_operation();

    let doc = match state.find_document(source_id) {
        Some(d) => d.clone(),
        None => {
            return ToolOutput::Error(ToolError {
                code: ErrorCode::InvalidReference,
                message: format!("Fuente {} no encontrada", source_id),
                provider: None,
            });
        }
    };

    let timeout = std::cmp::min(
        state.budget.http_timeout(),
        state.deadline.saturating_duration_since(Instant::now()),
    );

    match tokio::time::timeout(timeout, reader::read(&doc.url, focus.unwrap_or(""))).await {
        Ok(Ok(result)) => {
            let fragment_texts: Vec<String> = result
                .headers
                .iter()
                .chain(result.tables.iter())
                .chain(result.body_blocks.iter())
                .cloned()
                .collect();

            let discovered_links: Vec<DiscoveredLink> = result
                .links
                .iter()
                .enumerate()
                .filter_map(|(i, link)| {
                    let resolved = reader::resolve_link(&result.final_url, &link.href)?;
                    Some(DiscoveredLink {
                        link_id: LinkId::new(source_id, i),
                        source_id: source_id.clone(),
                        url: resolved,
                        anchor_text: link.anchor_text.clone(),
                    })
                })
                .collect();

            let fragments = state.add_fragments(source_id, fragment_texts, None);

            ToolOutput::Success {
                documents: vec![Document {
                    source_id: source_id.clone(),
                    title: doc.title,
                    url: doc.url,
                    final_url: result.final_url.clone(),
                    domain: doc.domain,
                    provider: doc.provider,
                    status: DocumentStatus::ContentRead,
                    http_status: Some(200),
                    retrieved_at: crate::research::google::now_millis_pub(),
                    published_at: result.published_at,
                    modified_at: result.modified_at,
                    read_scope: ReadScope::Full,
                    bytes_downloaded: result.bytes_downloaded,
                    text_extracted_chars: result.text_extracted_chars,
                    error: None,
                    headers: result.headers.into_iter().enumerate().map(|(i, text)| {
                        Fragment {
                            fragment_id: FragmentId::new(source_id, i),
                            source_id: source_id.clone(),
                            text,
                            section: Some("header".into()),
                        }
                    }).collect(),
                    tables: result.tables.into_iter().enumerate().map(|(i, text)| {
                        Fragment {
                            fragment_id: FragmentId::new(source_id, i),
                            source_id: source_id.clone(),
                            text,
                            section: Some("table".into()),
                        }
                    }).collect(),
                    links: discovered_links,
                }],
                fragments: fragments.into_iter().map(|fid| {
                    Fragment {
                        fragment_id: fid,
                        source_id: source_id.clone(),
                        text: String::new(),
                        section: None,
                    }
                }).collect(),
                links: Vec::new(),
            }
        }
        Ok(Err(e)) => ToolOutput::Error(ToolError {
            code: ErrorCode::NetworkError,
            message: e,
            provider: Some(doc.provider),
        }),
        Err(_) => ToolOutput::Error(ToolError {
            code: ErrorCode::Timeout,
            message: "Tiempo de lectura agotado".into(),
            provider: Some(doc.provider),
        }),
    }
}

async fn execute_follow_link(
    state: &mut ResearchState,
    source_id: &SourceId,
    link_id: &LinkId,
    focus: Option<&str>,
) -> ToolOutput {
    state.counters.record_read();
    state.counters.record_follow();
    state.counters.record_operation();

    let link = match state.find_link(source_id, link_id) {
        Some(l) => l.clone(),
        None => {
            return ToolOutput::Error(ToolError {
                code: ErrorCode::InvalidReference,
                message: format!("Enlace {} no encontrado", link_id),
                provider: None,
            });
        }
    };

    let timeout = std::cmp::min(
        state.budget.http_timeout(),
        state.deadline.saturating_duration_since(Instant::now()),
    );

    match tokio::time::timeout(timeout, reader::read(&link.url, focus.unwrap_or(""))).await {
        Ok(Ok(result)) => {
            let new_id = SourceId::new(state.documents.len() + 1);
            let link_url = link.url.clone();
            let link_text = link.anchor_text.clone();
            let fragment_texts: Vec<String> = result
                .headers
                .iter()
                .chain(result.tables.iter())
                .chain(result.body_blocks.iter())
                .cloned()
                .collect();

            let discovered_links: Vec<DiscoveredLink> = result
                .links
                .iter()
                .enumerate()
                .filter_map(|(i, l)| {
                    let resolved = reader::resolve_link(&result.final_url, &l.href)?;
                    Some(DiscoveredLink {
                        link_id: LinkId::new(&new_id, i),
                        source_id: new_id.clone(),
                        url: resolved,
                        anchor_text: l.anchor_text.clone(),
                    })
                })
                .collect();

            let _fragments = state.add_fragments(&new_id, fragment_texts, None);

            ToolOutput::Success {
                documents: vec![Document {
                    source_id: new_id.clone(),
                    title: link_text,
                    url: link_url.clone(),
                    final_url: result.final_url.clone(),
                    domain: reqwest::Url::parse(&link_url)
                        .ok()
                        .and_then(|u| u.host_str().map(str::to_owned))
                        .unwrap_or_default(),
                    provider: "web".to_string(),
                    status: DocumentStatus::ContentRead,
                    http_status: Some(200),
                    retrieved_at: crate::research::google::now_millis_pub(),
                    published_at: result.published_at,
                    modified_at: result.modified_at,
                    read_scope: ReadScope::Full,
                    bytes_downloaded: result.bytes_downloaded,
                    text_extracted_chars: result.text_extracted_chars,
                    error: None,
                    headers: result.headers.into_iter().enumerate().map(|(i, text)| {
                        Fragment {
                            fragment_id: FragmentId::new(&new_id, i),
                            source_id: new_id.clone(),
                            text,
                            section: Some("header".into()),
                        }
                    }).collect(),
                    tables: result.tables.into_iter().enumerate().map(|(i, text)| {
                        Fragment {
                            fragment_id: FragmentId::new(&new_id, i),
                            source_id: new_id.clone(),
                            text,
                            section: Some("table".into()),
                        }
                    }).collect(),
                    links: discovered_links,
                }],
                fragments: Vec::new(),
                links: Vec::new(),
            }
        }
        Ok(Err(e)) => ToolOutput::Error(ToolError {
            code: ErrorCode::NetworkError,
            message: e,
            provider: Some("web".into()),
        }),
        Err(_) => ToolOutput::Error(ToolError {
            code: ErrorCode::Timeout,
            message: "Tiempo de lectura agotado".into(),
            provider: Some("web".into()),
        }),
    }
}

async fn execute_github_read(
    state: &mut ResearchState,
    source_id: &SourceId,
    resource: &str,
    filters: Option<&str>,
) -> ToolOutput {
    state.counters.record_read();
    state.counters.record_operation();

    let doc = match state.find_document(source_id) {
        Some(d) => d.clone(),
        None => {
            return ToolOutput::Error(ToolError {
                code: ErrorCode::InvalidReference,
                message: format!("Fuente {} no encontrada", source_id),
                provider: None,
            });
        }
    };

    match crate::research::providers::github::read_repository(
        source_id,
        &doc.url,
        resource,
        filters,
    )
    .await
    {
        crate::research::providers::github::GitHubResult::Repository {
            doc: main_doc,
            readme,
            releases,
            issues,
        } => {
            let mut documents = vec![main_doc];
            if let Some(r) = readme {
                documents.push(r);
            }
            documents.extend(releases);
            documents.extend(issues);
            assign_provider_ids(state, &mut documents);
            ToolOutput::Success {
                documents,
                fragments: Vec::new(),
                links: Vec::new(),
            }
        }
        crate::research::providers::github::GitHubResult::Error(e) => ToolOutput::Error(ToolError {
            code: ErrorCode::NetworkError,
            message: e,
            provider: Some("github".into()),
        }),
    }
}

async fn execute_reddit_read(
    state: &mut ResearchState,
    source_id: &SourceId,
    max_comments: usize,
    sort: Option<&str>,
) -> ToolOutput {
    state.counters.record_read();
    state.counters.record_operation();

    let doc = match state.find_document(source_id) {
        Some(d) => d.clone(),
        None => {
            return ToolOutput::Error(ToolError {
                code: ErrorCode::InvalidReference,
                message: format!("Fuente {} no encontrada", source_id),
                provider: None,
            });
        }
    };

    match crate::research::providers::reddit::read_post(
        source_id,
        &doc.url,
        max_comments,
        sort.unwrap_or("best"),
    )
    .await
    {
        crate::research::providers::reddit::RedditResult::Post {
            doc: post_doc,
            comments,
        } => {
            let mut documents = vec![post_doc];
            documents.extend(comments);
            assign_provider_ids(state, &mut documents);
            ToolOutput::Success {
                documents,
                fragments: Vec::new(),
                links: Vec::new(),
            }
        }
        crate::research::providers::reddit::RedditResult::Error(e) => ToolOutput::Error(ToolError {
            code: ErrorCode::NetworkError,
            message: e,
            provider: Some("reddit".into()),
        }),
    }
}

// Provider-local numbering must never overwrite another source in the session.
fn assign_provider_ids(state: &ResearchState, documents: &mut [Document]) {
    let mut next = state.documents.len() + 1;
    for doc in documents {
        let id = if let Some(existing) = state.documents.iter().find(|d| d.url == doc.url) {
            existing.source_id.clone()
        } else { let id = SourceId::new(next); next += 1; id };
        doc.source_id = id.clone();
        for (i, fragment) in doc.headers.iter_mut().chain(doc.tables.iter_mut()).enumerate() {
            fragment.source_id = id.clone(); fragment.fragment_id = FragmentId::new(&id, i);
        }
        for (i, link) in doc.links.iter_mut().enumerate() {
            link.source_id = id.clone(); link.link_id = LinkId::new(&id, i);
        }
    }
}
