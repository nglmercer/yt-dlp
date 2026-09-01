fn medaltv_url(value: Option<&serde_json::Value>) -> Option<String> {
    let value = value.and_then(serde_json::Value::as_str)?.trim();
    if value.is_empty() || value.contains("video/privacy-protected-guest") {
        return None;
    }
    let parsed = url::Url::parse(value).ok()?;
    matches!(parsed.scheme(), "http" | "https").then(|| value.to_owned())
}

fn medaltv_formats(
    context: &ExtractionContext,
    content: &serde_json::Value,
    video_id: &str,
) -> Result<Vec<serde_json::Value>, ExtractorError> {
    let mut formats = Vec::new();
    if let Some(hls_url) = medaltv_url(content.get("contentUrlHls")) {
        formats.push(serde_json::json!({
            "url": hls_url,
            "format_id": "hls",
            "ext": "mp4",
            "protocol": "m3u8_native",
        }));
    }
    if let Some(http_url) = medaltv_url(content.get("contentUrl")) {
        formats.push(serde_json::json!({
            "url": http_url,
            "format_id": "http-source",
            "ext": "mp4",
            "quality": 1,
        }));
    }
    if formats.is_empty() {
        let social_url = medaltv_social_url(context, video_id)?;
        if social_url.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Medal.tv video {video_id} has no social-video URL"),
            ));
        }
        formats.push(serde_json::json!({
            "url": social_url,
            "format_id": "social-video",
            "ext": "mp4",
            "quality": -1,
        }));
    }
    Ok(formats)
}

fn medaltv_value_string(value: Option<&serde_json::Value>) -> Option<String> {
    let value = value?;
    match value {
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn medaltv_timestamp(value: Option<&serde_json::Value>) -> Option<i64> {
    let value = value?;
    let milliseconds = value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
        .or_else(|| value.as_str().and_then(|value| value.parse().ok()))?;
    Some(milliseconds / 1000)
}

fn medaltv_tags(content: &serde_json::Value) -> Option<serde_json::Value> {
    let tags = content
        .get("tags")
        .and_then(serde_json::Value::as_array)?
        .iter()
        .filter_map(|tag| tag.as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    (!tags.is_empty()).then(|| serde_json::json!(tags))
}
