use crate::research::types::*;
use crate::research::Source;

/// Resultado de la verificación de una respuesta.
#[allow(dead_code)]
pub struct VerificationOutput {
    pub verified_claims: Vec<VerifiedClaim>,
    pub message: String,
    pub can_finalize: bool,
}

/// Valida determinísticamente las citas de una respuesta.
pub fn validate_citations(message: &str, state: &ResearchState) -> Result<(), String> {
    let refs = regex::Regex::new("\\[([sS]\\d+(?::p\\d+)?)\\]").unwrap();
    for capture in refs.captures_iter(message) {
        let ref_str = &capture[1];
        if let Some(colon_pos) = ref_str.find(':') {
            let source_part = &ref_str[..colon_pos];
            let source_id = SourceId::new(source_part[1..].parse().unwrap_or(0));
            if state.find_document(&source_id).is_none() {
                return Err(format!("Cita a fuente inexistente: [{ref_str}]"));
            }
            if !state.fragments.iter().any(|f| f.fragment_id.as_str().eq_ignore_ascii_case(ref_str) && !f.text.is_empty()) {
                return Err(format!("Fragmento inexistente: [{ref_str}]"));
            }
        } else {
            let num: usize = ref_str[1..].parse().unwrap_or(0);
            let source_id = SourceId::new(num);
            if state.find_document(&source_id).is_none() {
                return Err(format!("Cita a fuente inexistente: [{ref_str}]"));
            }
        }
    }
    Ok(())
}

/// Valida que el modelo no inventó URLs o fuentes.
pub fn validate_urls(message: &str, state: &ResearchState) -> Result<(), String> {
    let url_re = regex::Regex::new("https?://[^\\s)>\"]+").unwrap();
    for url_match in url_re.find_iter(message) {
        let url = url_match.as_str();
        let exists = state.documents.iter().any(|d| {
            d.url == url || d.final_url == url || d.links.iter().any(|l| l.url == url)
        });
        if !exists {
            return Err(format!("URL en respuesta no encontrada en evidencia: {url}"));
        }
    }
    Ok(())
}

/// Reconstruye el mapeo final de IDs de fuente para el mensaje y la lista.
pub fn remap_source_ids(
    message: &str,
    sources: &[Source],
) -> (String, Vec<Source>) {
    let refs = regex::Regex::new(r"\[[sS](\d+)(?::p\d+)?\]").unwrap();
    let mut picked: Vec<Source> = Vec::new();
    let result = refs.replace_all(message, |caps: &regex::Captures| {
        let id = format!("s{}", &caps[1]);
        if let Some(source) = sources.iter().find(|s| s.source_id == id) {
            let index = match picked.iter().position(|s| s.source_id == id) {
                Some(i) => i,
                None => { picked.push(source.clone()); picked.len() - 1 }
            };
            format!("[s{}]", index + 1)
        } else { caps[0].to_string() }
    }).into_owned();
    for (i, source) in picked.iter_mut().enumerate() { source.source_id = format!("s{}", i + 1); }
    (result, picked)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_citations_rejects_missing_source() {
        let state = ResearchState::new(
            "r1".into(),
            "session".into(),
            1,
            "test".into(),
            "es".into(),
            Default::default(),
        );
        assert!(validate_citations("This is [s1] but [s99] missing", &state).is_err());
        assert!(validate_citations("No citations here", &state).is_ok());
    }

    #[test]
    fn remap_source_ids_keeps_order() {
        let sources = vec![
            Source {
                source_id: "s1".into(),
                title: "First".into(),
                url: "https://a.com".into(),
                domain: "a.com".into(),
                source_type: "Web".into(),
                snippet: "aa".into(),
                provider: "google".into(),
                retrieved_at: "0".into(),
                read_status: "full".into(),
                published_at: None,
            },
            Source {
                source_id: "s2".into(),
                title: "Second".into(),
                url: "https://b.com".into(),
                domain: "b.com".into(),
                source_type: "Web".into(),
                snippet: "bb".into(),
                provider: "google".into(),
                retrieved_at: "0".into(),
                read_status: "full".into(),
                published_at: None,
            },
        ];
        let (msg, picked) = remap_source_ids("Answer [s2] from [s2]", &sources);
        assert!(msg.contains("[s1]"));
        assert!(!msg.contains("[s2]"));
        assert_eq!(picked.len(), 1);
        assert_eq!(picked[0].source_id, "s1");
        let (msg, picked) = remap_source_ids("Second [s2] then first [s1] and fragment [s2:p4]", &sources);
        assert_eq!(msg, "Second [s1] then first [s2] and fragment [s1]");
        assert_eq!(picked[0].url, "https://b.com");
        assert_eq!(picked[1].url, "https://a.com");
    }
}
