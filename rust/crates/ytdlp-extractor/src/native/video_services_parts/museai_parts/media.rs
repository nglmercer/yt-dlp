fn museai_url(value: Option<&serde_json::Value>) -> Option<String> {
    let value = value.and_then(serde_json::Value::as_str)?.trim();
    let value = proto_relative_url(value, "https:");
    let parsed = url::Url::parse(&value).ok()?;
    matches!(parsed.scheme(), "http" | "https").then_some(value)
}

fn museai_formats(
    data: &serde_json::Value,
    video_id: &str,
) -> Result<Vec<serde_json::Value>, ExtractorError> {
    let source_url = museai_url(data.get("url")).ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("MuseAI video {video_id} has no valid source URL"),
        )
    })?;
    let source_ext = json_string(data, "filename")
        .map(|filename| yt_dlp_core::determine_ext(Some(filename), "mp4"))
        .unwrap_or_else(|| yt_dlp_core::determine_ext(Some(&source_url), "mp4"));
    let mut source = serde_json::json!({
        "url": source_url.clone(),
        "format_id": "source",
        "quality": 1,
        "ext": source_ext,
    });
    if let Some(width) = json_i64(data, "width") {
        source["width"] = serde_json::json!(width);
    }
    if let Some(height) = json_i64(data, "height") {
        source["height"] = serde_json::json!(height);
    }
    if let Some(filesize) = json_i64(data, "size") {
        source["filesize"] = serde_json::json!(filesize);
    }
    let mut formats = vec![source];
    if let Some(base_url) = source_url.strip_suffix("/data") {
        formats.push(serde_json::json!({
            "url": format!("{base_url}/videos/hls.m3u8"),
            "format_id": "hls",
            "ext": "mp4",
            "protocol": "m3u8_native",
        }));
        formats.push(serde_json::json!({
            "url": format!("{base_url}/videos/dash.mpd"),
            "format_id": "dash",
            "ext": "mp4",
            "protocol": "http_dash_segments",
        }));
    }
    Ok(formats)
}
