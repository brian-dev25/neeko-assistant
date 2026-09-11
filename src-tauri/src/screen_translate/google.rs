//! Google Web protocol adapted from MORT GoogleBasicTranslateAPI.cs (MIT).
//! Copyright (c) 2024 몽키해드. See native/screen-translate/LICENSE-MORT.txt.
use serde_json::{json, Value};

pub(super) fn language(tag: &str) -> &str {
    match tag {
        "zh-Hant" | "zh-TW" | "zh-HK" => "zh-TW",
        "zh" | "zh-Hans" | "zh-CN" => "zh-CN",
        _ => tag.split('-').next().unwrap_or(tag),
    }
}

fn segments(value: &Value) -> Option<String> {
    let text: String = value
        .as_array()?
        .iter()
        .filter_map(|v| v[0].as_str())
        .collect();
    let text = text.trim();
    (!text.is_empty() && text.chars().count() <= 12000).then(|| text.to_owned())
}

fn parse_batch(body: &str) -> Option<String> {
    for line in body
        .lines()
        .filter(|line| line.trim_start().starts_with('['))
    {
        let Ok(outer) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        for frame in outer.as_array().into_iter().flatten() {
            if frame[1] != "MkEWBc" {
                continue;
            }
            let Some(inner) = frame[2]
                .as_str()
                .and_then(|v| serde_json::from_str::<Value>(v).ok())
            else {
                continue;
            };
            if let Some(text) = segments(&inner[1][0][0][5]) {
                return Some(text);
            }
        }
    }
    None
}

async fn response(request: reqwest::RequestBuilder) -> Result<String, bool> {
    let mut response = request.send().await.map_err(|_| false)?;
    if response.status().as_u16() == 429 {
        return Err(true);
    }
    if !response.status().is_success() {
        return Err(false);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| false)? {
        if bytes.len() + chunk.len() > 262144 {
            return Err(false);
        }
        bytes.extend_from_slice(&chunk);
    }
    String::from_utf8(bytes).map_err(|_| false)
}

pub(super) async fn translate(
    client: &reqwest::Client,
    text: &str,
    source: &str,
    target: &str,
) -> Result<String, String> {
    let source = language(source);
    let target = language(target);
    if source == target {
        return Ok(text.to_owned());
    }
    let argument = json!([[text, source, target, true], [null]]).to_string();
    let payload = json!([[["MkEWBc", argument, null, "generic"]]]).to_string();
    let requests = [
        client.post("https://translate.google.com/_/TranslateWebserverUi/data/batchexecute?rpcids=MkEWBc&rt=c")
            .header("origin", "https://translate.google.com").header("referer", "https://translate.google.com/")
            .form(&[("f.req", payload.as_str())]),
        client.get("https://translate.googleapis.com/translate_a/single")
            .query(&[("client", "gtx"), ("sl", source), ("tl", target), ("dt", "t"), ("q", text)]),
        client.get("https://clients5.google.com/translate_a/t")
            .query(&[("client", "dict-chrome-ex"), ("sl", source), ("tl", target), ("q", text)]),
    ];
    for (index, request) in requests.into_iter().enumerate() {
        let body = match response(request).await {
            Ok(body) => body,
            Err(true) => {
                return Err(
                    "Google limitó las traducciones. Esperá unos minutos antes de reiniciar."
                        .into(),
                )
            }
            Err(false) => continue,
        };
        let parsed = match index {
            0 => parse_batch(&body),
            1 => serde_json::from_str::<Value>(&body)
                .ok()
                .and_then(|v| segments(&v[0])),
            _ => serde_json::from_str::<Vec<String>>(&body)
                .ok()
                .map(|v| v.concat()),
        };
        if let Some(text) = parsed.filter(|s| !s.trim().is_empty() && s.chars().count() <= 12000) {
            return Ok(text);
        }
    }
    Err("Google Web no respondió correctamente. Revisá Internet y volvé a intentar; el servicio puede cambiar o limitar solicitudes.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_framed_batch_and_rejects_unexpected_data() {
        let inner = json!([
            null,
            [[[null, null, null, null, null, [["Hola "], ["mundo"]]]]]
        ]);
        let body = format!(
            ")]}}'\n\n123\n{}\n",
            json!([["wrb.fr", "MkEWBc", inner.to_string(), null]])
        );
        assert_eq!(parse_batch(&body).as_deref(), Some("Hola mundo"));
        assert!(parse_batch("<html>captcha</html>").is_none());
        assert!(parse_batch("[[null]]").is_none());
        assert_eq!(
            segments(&json!([["Hola"], [" mundo"]])).as_deref(),
            Some("Hola mundo")
        );
        assert!(segments(&json!([[null]])).is_none());
    }
    #[test]
    fn maps_ocr_locales_to_translation_languages() {
        assert_eq!(language("es-MX"), "es");
        assert_eq!(language("en-US"), "en");
        assert_eq!(language("zh-Hant"), "zh-TW");
        assert_eq!(language("zh-Hans"), "zh-CN");
    }
    #[tokio::test]
    async fn rate_limit_stops_and_response_size_is_bounded() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        for (status, body, limited) in
            [(429, "".to_owned(), true), (200, "x".repeat(262145), false)]
        {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = [0; 4096];
                socket.read(&mut request).await.unwrap();
                let _ = socket.write_all(format!("HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await;
            });
            let client = reqwest::Client::builder()
                .no_proxy()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .unwrap();
            assert_eq!(
                response(client.get(format!("http://{address}"))).await,
                Err(limited)
            );
            server.await.unwrap();
        }
    }
    #[tokio::test]
    #[ignore = "Uses Google Web with synthetic text only; requires Internet"]
    async fn live_google_translation() {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap();
        let translated = translate(&client, "Hello world", "en-US", "es")
            .await
            .unwrap();
        assert!(translated.to_lowercase().contains("hola"), "{translated}");
        let translated = translate(&client,
            "las habilidades de programacion que debi aprender antes", "es-ES", "en")
            .await.unwrap();
        assert!(translated.to_lowercase().contains("programming"), "{translated}");
        assert!(translated.to_lowercase().contains("skills"), "{translated}");
    }
}
