fn kompas_stream_format(
    stream: &serde_json::Value,
    drm: bool,
) -> Option<serde_json::Value> {
    let media_url = json_string(stream, "url")
        .filter(|value| value.starts_with("http://") || value.starts_with("https://"))?;
    let stream_type = json_string(stream, "type").unwrap_or_default();
    let mut format = if stream_type.eq_ignore_ascii_case("hls") {
        serde_json::json!({
            "url": media_url,
            "format_id": "hls",
            "protocol": "m3u8_native",
            "ext": "mp4",
        })
    } else {
        serde_json::json!({
            "url": media_url,
            "protocol": "http",
            "ext": "mp4",
        })
    };
    if let Some(width) = json_i64(stream, "width") {
        format["width"] = serde_json::json!(width);
    }
    if let Some(height) = json_i64(stream, "height") {
        format["height"] = serde_json::json!(height);
    }
    if drm && stream_type.eq_ignore_ascii_case("hls") {
        format["has_drm"] = serde_json::json!(true);
    }
    Some(format)
}

fn kompas_text_list(value: Option<&serde_json::Value>) -> Option<Vec<String>> {
    let values = value
        .and_then(serde_json::Value::as_str)?
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    (!values.is_empty()).then_some(values)
}

fn kompas_thumbnails(value: Option<&serde_json::Value>) -> Option<Vec<serde_json::Value>> {
    let thumbnails = value
        .and_then(serde_json::Value::as_array)?
        .iter()
        .filter_map(|thumbnail| {
            if thumbnail.is_object() {
                Some(thumbnail.clone())
            } else {
                thumbnail
                    .as_str()
                    .map(|url| serde_json::json!({"url": url}))
            }
        })
        .collect::<Vec<_>>();
    (!thumbnails.is_empty()).then_some(thumbnails)
}

fn kompas_description(value: Option<&serde_json::Value>) -> Option<String> {
    value
        .and_then(serde_json::Value::as_str)
        .map(html_text_fragment)
        .filter(|value| !value.is_empty())
}
