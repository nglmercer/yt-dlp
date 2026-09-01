fn lrt_json_request(
    context: &ExtractionContext,
    endpoint: &str,
    query: &[(&str, String)],
    description: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let mut request = Request::new(endpoint);
    let query = query
        .iter()
        .map(|(key, value)| ((*key).to_owned(), value.clone()))
        .collect::<Vec<_>>();
    request.update_query(&query);
    let response = context.request(&request)?;
    serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid LRT {description} JSON: {error}"),
        )
    })
}

fn lrt_unescape(value: &str) -> String {
    value
        .replace("\\/", "/")
        .replace("\\u0026", "&")
        .replace("\\u003d", "=")
        .replace("\\\"", "\"")
}

fn lrt_streams_url(webpage: &str, video_id: &str) -> Result<String, ExtractorError> {
    let matcher = Regex::new(r#"(?s)\\?"get_streams_url\\?"\s*:\s*\\?"([^"\\]+)"#)
        .map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid LRT stream URL matcher: {error}"),
            )
        })?;
    matcher
        .captures(webpage)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| lrt_unescape(value.as_str()))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("LRT stream page {video_id} has no get_streams_url value"),
            )
        })
}

fn lrt_canonical_url(webpage: &str) -> Option<String> {
    let patterns = [
        r#"(?is)<link\b[^>]*\brel\s*=\s*["']canonical["'][^>]*\bhref\s*=\s*["']([^"']+)"#,
        r#"(?is)<link\b[^>]*\bhref\s*=\s*["']([^"']+)["'][^>]*\brel\s*=\s*["']canonical["']"#,
        r#"(?is)\\?"(?:article|data)\\?"\s*:\s*\{[^}]*?\\?"url\\?"\s*:\s*\\?"([^"\\]+)"#,
    ];
    patterns.iter().find_map(|pattern| {
        Regex::new(pattern)
            .ok()
            .and_then(|matcher| matcher.captures(webpage).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| lrt_unescape(value.as_str()))
            .filter(|value| !value.is_empty())
    })
}

fn lrt_stream_data_urls(data: &serde_json::Value) -> Vec<String> {
    let mut urls = Vec::new();
    let Some(response) = data.get("response") else {
        return urls;
    };
    let Some(stream_data) = response.get("data") else {
        return urls;
    };
    match stream_data {
        serde_json::Value::Object(values) => {
            for (key, value) in values {
                if key.starts_with("content") {
                    if let Some(value) = value.as_str() {
                        if !value.is_empty() {
                            urls.push(lrt_unescape(value));
                        }
                    }
                }
            }
        }
        serde_json::Value::Array(values) => {
            urls.extend(
                values
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .filter(|value| !value.is_empty())
                    .map(lrt_unescape),
            );
        }
        _ => {}
    }
    urls
}

fn lrt_fetch_vod_media(
    context: &ExtractionContext,
    video_id: &str,
    canonical_url: &str,
) -> Result<serde_json::Value, ExtractorError> {
    lrt_json_request(
        context,
        "https://www.lrt.lt/servisai/stream_url/vod/media_info/",
        &[("url", canonical_url.to_owned())],
        &format!("VOD media info for {video_id}"),
    )
}

fn lrt_fetch_radio_media(
    context: &ExtractionContext,
    video_id: &str,
    path: &str,
) -> Result<serde_json::Value, ExtractorError> {
    lrt_json_request(
        context,
        "https://www.lrt.lt/rest-api/media",
        &[("url", format!("/mediateka/irasas/{video_id}/{path}"))],
        &format!("radio media info for {video_id}"),
    )
}
