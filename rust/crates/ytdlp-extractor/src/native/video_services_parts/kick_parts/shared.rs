const KICK_API_BASE: &str = "https://kick.com/api";

fn kick_api_json(
    context: &ExtractionContext,
    path: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let endpoint = format!("{KICK_API_BASE}/{path}");
    let mut request = Request::new(&endpoint);
    if let Some(token) = kick_session_token(context)? {
        request
            .headers_mut()
            .set("Authorization", format!("Bearer {token}"));
    }
    let response = context.request(&request)?;
    serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Kick API JSON from {}: {error}", response.url()),
        )
    })
}

fn kick_session_token(context: &ExtractionContext) -> Result<Option<String>, ExtractorError> {
    let header = context
        .cookie_jar()
        .lock()
        .map_err(|_| ExtractorError::new(ExtractorErrorKind::Network, "Kick cookie jar poisoned"))?
        .cookie_header("https://kick.com/")
        .map_err(map_request_error)?;
    Ok(header.and_then(|cookies| {
        cookies.split(';').find_map(|cookie| {
            let (name, value) = cookie.trim().split_once('=')?;
            (name == "session_token").then(|| percent_decode(value))
        })
    }))
}

fn kick_valid_url(value: &str) -> Option<String> {
    let value = value.trim();
    (value.starts_with("http://") || value.starts_with("https://"))
        .then(|| value.to_owned())
}

fn kick_media_format(media_url: &str, format_id: &str) -> serde_json::Value {
    let extension = yt_dlp_core::determine_ext(Some(media_url), "mp4").to_ascii_lowercase();
    if extension == "m3u8" {
        serde_json::json!({
            "url": media_url,
            "format_id": format_id,
            "ext": "mp4",
            "protocol": "m3u8_native",
        })
    } else {
        serde_json::json!({
            "url": media_url,
            "format_id": format_id,
            "ext": extension,
            "protocol": "http",
        })
    }
}

fn kick_timestamp(value: Option<&str>) -> Option<i64> {
    value.and_then(|value| parse_timestamp(value.to_owned()))
}

fn kick_optional_url(value: Option<&serde_json::Value>) -> Option<String> {
    value
        .and_then(|value| value.as_str().and_then(kick_valid_url))
        .or_else(|| {
            value
                .and_then(|value| value.get("url"))
                .and_then(serde_json::Value::as_str)
                .and_then(kick_valid_url)
        })
}

fn kick_category_names(value: Option<&serde_json::Value>) -> Option<Vec<String>> {
    let mut names = Vec::new();
    match value {
        Some(serde_json::Value::Array(values)) => {
            for value in values {
                if let Some(name) = value
                    .as_str()
                    .map(str::to_owned)
                    .or_else(|| json_string(value, "name").map(str::to_owned))
                {
                    names.push(name);
                }
            }
        }
        Some(serde_json::Value::Object(values)) => {
            if let Some(name) = values.get("name").and_then(serde_json::Value::as_str) {
                names.push(name.to_owned());
            }
        }
        Some(serde_json::Value::String(value)) => names.push(value.clone()),
        _ => {}
    }
    (!names.is_empty()).then_some(names)
}

fn kick_age_limit(value: Option<&serde_json::Value>) -> Option<i64> {
    value
        .and_then(serde_json::Value::as_bool)
        .map(|is_mature| if is_mature { 18 } else { 0 })
}

fn kick_is_vod_url(url: &str) -> bool {
    Regex::new(
        r#"(?i)^https?://(?:www\.)?kick\.com/[\w-]+/videos/[\da-f]{8}-(?:[\da-f]{4}-){3}[\da-f]{12}"#,
    )
    .ok()
    .is_some_and(|matcher| matcher.is_match(url).unwrap_or(false))
}

fn kick_is_clip_url(url: &str) -> bool {
    Regex::new(
        r#"(?i)^https?://(?:www\.)?kick\.com/[\w-]+(?:/clips/|/?\?(?:[^#]+&)?clip=)clip_[\w-]+"#,
    )
    .ok()
    .is_some_and(|matcher| matcher.is_match(url).unwrap_or(false))
}
