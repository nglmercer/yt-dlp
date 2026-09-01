fn markiza_http_url(value: Option<String>) -> Option<String> {
    let value = value?;
    let value = value.trim();
    let parsed = url::Url::parse(value).ok()?;
    matches!(parsed.scheme(), "http" | "https").then(|| value.to_owned())
}

fn markiza_sources(
    item: &serde_json::Value,
    video_id: &str,
) -> Result<Vec<serde_json::Value>, ExtractorError> {
    let source_values = item
        .get("sources")
        .and_then(serde_json::Value::as_array)
        .map(|sources| sources.iter().collect::<Vec<_>>())
        .unwrap_or_else(|| vec![item]);
    let mut formats = Vec::new();
    for source in source_values {
        let Some(media_url) = markiza_http_url(
            markiza_value_string(source.get("file"))
                .or_else(|| markiza_value_string(source.get("src"))),
        ) else {
            continue;
        };
        let extension = yt_dlp_core::determine_ext(Some(&media_url), "mp4").to_ascii_lowercase();
        let is_hls = extension == "m3u8";
        let is_dash = extension == "mpd";
        let mut format = serde_json::json!({
            "url": media_url,
            "format_id": json_string(source, "label").unwrap_or("source"),
            "ext": if is_hls || is_dash { "mp4" } else { extension.as_str() },
        });
        if is_hls {
            format["protocol"] = serde_json::json!("m3u8_native");
        } else if is_dash {
            format["protocol"] = serde_json::json!("http_dash_segments");
        }
        if let Some(height) = json_i64(source, "height") {
            format["height"] = serde_json::json!(height);
        }
        if let Some(width) = json_i64(source, "width") {
            format["width"] = serde_json::json!(width);
        }
        formats.push(format);
    }
    if formats.is_empty() {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Markiza video {video_id} has no playable sources"),
        ));
    }
    Ok(formats)
}

fn markiza_item_info(
    item: &serde_json::Value,
    fallback_id: &str,
    fallback_duration: Option<i64>,
) -> Result<InfoDict, ExtractorError> {
    let video_id = markiza_value_string(item.get("mediaid"))
        .or_else(|| markiza_value_string(item.get("id")))
        .unwrap_or_else(|| fallback_id.to_owned());
    let formats = markiza_sources(item, &video_id)?;
    let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
    let title = markiza_value_string(item.get("title"))
        .or_else(|| markiza_value_string(item.get("name")))
        .unwrap_or_else(|| video_id.clone());
    let mut info = InfoDict::new();
    info.insert("id", serde_json::json!(video_id));
    info.insert("title", serde_json::json!(title));
    info.insert("formats", serde_json::Value::Array(formats));
    info.insert_if_some(
        "description",
        json_string(item, "description").map(html_text_fragment),
    );
    info.insert_if_some(
        "thumbnail",
        markiza_http_url(markiza_value_string(item.get("image"))),
    );
    info.insert_if_some(
        "duration",
        markiza_duration(item.get("duration")).or(fallback_duration),
    );
    info.insert_if_some("url", first.get("url").and_then(serde_json::Value::as_str));
    info.insert_if_some("ext", first.get("ext").and_then(serde_json::Value::as_str));
    Ok(info)
}
