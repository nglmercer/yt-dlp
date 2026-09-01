fn minds_integer(value: Option<&serde_json::Value>) -> Option<i64> {
    value.and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
            .or_else(|| value.as_f64().map(|value| value as i64))
            .or_else(|| value.as_str().and_then(|value| value.trim().parse().ok()))
    })
}

fn minds_text(value: Option<&serde_json::Value>) -> Option<String> {
    value
        .and_then(serde_json::Value::as_str)
        .map(html_text_fragment)
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn minds_formats(
    video: &serde_json::Value,
    video_id: &str,
) -> Result<Vec<serde_json::Value>, ExtractorError> {
    let sources = video
        .get("sources")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Minds video {video_id} has no sources"),
            )
        })?;
    let mut formats = Vec::new();
    for source in sources {
        let Some(media_url) = minds_valid_http_url(minds_value_string(source.get("src"))) else {
            continue;
        };
        let extension = yt_dlp_core::determine_ext(Some(&media_url), "mp4");
        let is_hls = extension.eq_ignore_ascii_case("m3u8");
        let mut format = serde_json::json!({
            "url": media_url,
            "ext": if is_hls { "mp4" } else { extension.as_str() },
        });
        if let Some(label) = json_string(source, "label").filter(|value| !value.is_empty()) {
            format["format_id"] = serde_json::json!(label);
        }
        if let Some(height) = minds_integer(source.get("size")) {
            format["height"] = serde_json::json!(height);
        }
        if is_hls {
            format["protocol"] = serde_json::json!("m3u8_native");
        }
        formats.push(format);
    }
    if formats.is_empty() {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Minds video {video_id} has no playable sources"),
        ));
    }
    Ok(formats)
}

fn minds_tags(value: Option<&serde_json::Value>) -> Option<serde_json::Value> {
    match value? {
        serde_json::Value::String(value) if !value.is_empty() => {
            Some(serde_json::json!([value]))
        }
        serde_json::Value::Array(values) if !values.is_empty() => {
            Some(serde_json::Value::Array(values.clone()))
        }
        _ => None,
    }
}

fn minds_uploader_url(uploader_id: Option<&str>) -> Option<String> {
    uploader_id.map(|uploader_id| format!("https://www.minds.com/{uploader_id}"))
}
