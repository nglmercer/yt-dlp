fn filmon_formats_from_object(streams: Option<&serde_json::Value>) -> Vec<serde_json::Value> {
    let Some(streams) = streams.and_then(serde_json::Value::as_object) else {
        return Vec::new();
    };
    streams
        .iter()
        .filter_map(|(format_id, stream)| {
            let media_url = json_string(stream, "url")
                .filter(|value| {
                    value.starts_with("http://")
                        || value.starts_with("https://")
                        || value.starts_with("rtmp://")
                })?
                .to_owned();
            let extension = yt_dlp_core::determine_ext(Some(&media_url), "mp4");
            let is_hls = extension.eq_ignore_ascii_case("m3u8");
            let mut format = serde_json::json!({
                "format_id": format_id,
                "url": media_url,
                "ext": if is_hls { "mp4" } else { extension.as_str() },
                "quality": filmon_quality(stream.get("quality").and_then(serde_json::Value::as_str)
                    .or_else(|| Some(format_id.as_str()))),
            });
            if is_hls {
                format["protocol"] = serde_json::json!("m3u8_native");
            }
            Some(format)
        })
        .collect()
}

fn filmon_quality(value: Option<&str>) -> i64 {
    match value.unwrap_or_default().to_ascii_lowercase().as_str() {
        "high" => 1,
        "medium" => 0,
        "low" => -1,
        _ => 0,
    }
}

fn filmon_thumbnail(id: &str, thumbnail: &serde_json::Value) -> Option<serde_json::Value> {
    let url = json_string(thumbnail, "url")
        .filter(|value| value.starts_with("http://") || value.starts_with("https://"))?;
    let mut result = serde_json::json!({
        "id": id,
        "url": url,
    });
    if let Some(width) = json_i64(thumbnail, "width") {
        result["width"] = serde_json::json!(width);
    }
    if let Some(height) = json_i64(thumbnail, "height") {
        result["height"] = serde_json::json!(height);
    }
    Some(result)
}

fn filmon_vod_thumbnails(poster: Option<&serde_json::Value>) -> Vec<serde_json::Value> {
    let Some(poster) = poster else {
        return Vec::new();
    };
    let mut thumbnails = Vec::new();
    if let Some(thumbs) = poster.get("thumbs").and_then(serde_json::Value::as_object) {
        for (id, thumbnail) in thumbs {
            if let Some(thumbnail) = filmon_thumbnail(id, thumbnail) {
                thumbnails.push(thumbnail);
            }
        }
    }
    if let Some(thumbnail) = filmon_thumbnail("poster", poster) {
        thumbnails.push(thumbnail);
    }
    thumbnails
}

fn filmon_channel_formats(
    streams: Option<&serde_json::Value>,
) -> (Vec<serde_json::Value>, bool) {
    let Some(streams) = streams.and_then(serde_json::Value::as_array) else {
        return (Vec::new(), false);
    };
    let mut formats = Vec::new();
    let mut unsupported = false;
    for stream in streams {
        let Some(media_url) = json_string(stream, "url")
            .filter(|value| {
                value.starts_with("http://")
                    || value.starts_with("https://")
                    || value.starts_with("rtmp://")
            })
        else {
            continue;
        };
        if media_url.starts_with("rtmp://") {
            unsupported = true;
            continue;
        }
        let extension = yt_dlp_core::determine_ext(Some(media_url), "mp4");
        let is_hls = extension.eq_ignore_ascii_case("m3u8");
        let format_id = json_string(stream, "quality")
            .filter(|value| !value.is_empty())
            .unwrap_or("stream");
        let mut format = serde_json::json!({
            "format_id": format_id,
            "url": media_url,
            "ext": if is_hls { "mp4" } else { extension.as_str() },
            "quality": filmon_quality(Some(format_id)),
        });
        if is_hls {
            format["protocol"] = serde_json::json!("m3u8_native");
        }
        formats.push(format);
    }
    (formats, unsupported)
}
