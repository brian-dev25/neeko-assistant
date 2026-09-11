//! Papago Web, checked against the public web client on 2026-09-10.
//! MORT 1.291 uses the older /apis/n2mt/translate + PPG protocol.
//! The current web client uses /api/text/translation without that signature.
use reqwest::Client;
use serde_json::Value;
use std::time::{Duration, Instant};

#[derive(Default)]
pub(super) struct Papago {
    next_request: Option<Instant>,
}

impl Papago {
    pub(super) async fn translate(
        &mut self,
        client: &Client,
        text: &str,
        source: &str,
        target: &str,
    ) -> Result<String, String> {
        let source = super::google::language(source);
        let target = super::google::language(target);
        if source == target {
            return Ok(text.to_owned());
        }
        if text.chars().count() > 5000 {
            return Err(
                "Papago admite hasta 5000 caracteres por lectura. Reducí la región.".into(),
            );
        }
        if let Some(next) = self.next_request {
            tokio::time::sleep(next.saturating_duration_since(Instant::now())).await;
        }
        let response = client
            .post("https://papago.naver.com/api/text/translation")
            .header("referer", "https://papago.naver.com/")
            .header("origin", "https://papago.naver.com")
            .header("accept-language", target)
            .form(&[
                ("source", source),
                ("target", target),
                ("text", text),
                ("honorific", "false"),
                ("dict", "false"),
                ("useGlossary", "false"),
            ])
            .send()
            .await;
        // MORT spaces requests by up to 650 ms. Use a fixed upper bound between blocks.
        self.next_request = Some(Instant::now() + Duration::from_millis(650));
        let mut response = response.map_err(|_| "Papago no respondió. Revisá la conexión.")?;
        let status = response.status();
        if status.as_u16() == 429 {
            return Err(
                "Papago limitó las consultas. Esperá unos minutos antes de reiniciar.".into(),
            );
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| "Respuesta Papago interrumpida")?
        {
            if bytes.len() + chunk.len() > 262144 {
                return Err("La respuesta de Papago supera el tamaño permitido.".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|_| format!("Papago devolvió una respuesta inválida (HTTP {status})."))?;
        if !status.is_success() || value.get("errorCode").is_some_and(|v| !v.is_null()) {
            let reason = match value["errorCode"].as_str().unwrap_or("") {
                "60102" => "El idioma de origen no está admitido",
                "60103" => "El idioma de destino no está admitido",
                "60109" => "No se pudo detectar el idioma; elegí el idioma del texto",
                _ => "El servicio rechazó la consulta",
            };
            return Err(format!(
                "Papago: {reason} ({source} → {target}, HTTP {status})."
            ));
        }
        parse_translation(&value, source, target)
    }
}

fn parse_translation(value: &Value, source: &str, target: &str) -> Result<String, String> {
    let actual_target = value["tarLangType"]
        .as_str()
        .ok_or("Papago no indicó el idioma de destino.")?;
    if super::google::language(actual_target) != target {
        return Err(
            "Papago devolvió otro idioma de destino; no se mostrará como traducción válida.".into(),
        );
    }
    let actual_source = value["srcLangType"]
        .as_str()
        .ok_or("Papago no indicó el idioma de origen.")?;
    if source != "auto" && super::google::language(actual_source) != source {
        return Err("Papago interpretó otro idioma de origen. Revisá el idioma del texto.".into());
    }
    value["translatedText"]
        .as_str()
        .filter(|text| !text.trim().is_empty() && text.chars().count() <= 12000)
        .map(str::to_owned)
        .ok_or_else(|| "Papago no devolvió una traducción válida.".into())
}
