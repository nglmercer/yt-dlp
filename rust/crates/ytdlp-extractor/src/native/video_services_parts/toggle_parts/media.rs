fn toggle_format_id(value: &str) -> String {
    value.replace(' ', "")
}

fn toggle_extension(value: &str) -> String {
    let path = value.split_once('?').map_or(value, |(path, _)| path);
    let lower_path = path.to_ascii_lowercase();
    if lower_path.ends_with(".ism/manifest") || lower_path.ends_with(".isml/manifest") {
        return "ism".to_owned();
    }
    yt_dlp_core::determine_ext(Some(value), "")
}

fn toggle_formats(
    info: &serde_json::Value,
    video_id: &str,
) -> Result<Vec<serde_json::Value>, ExtractorError> {
    let mut formats = Vec::new();
    let mut has_smooth_streaming = false;
    for video_file in info
        .get("Files")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(video_url) = json_string(video_file, "URL")
            .filter(|value| !value.is_empty() && *value != "NA")
        else {
            continue;
        };
        let Some(format_name) = json_string(video_file, "Format")
            .filter(|value| !value.is_empty())
            .map(toggle_format_id)
        else {
            continue;
        };
        let extension = toggle_extension(video_url);
        match extension.to_ascii_lowercase().as_str() {
            "m3u8" if video_url.contains("/fpshls/") => continue,
            "m3u8" => formats.push(serde_json::json!({
                "url": video_url,
                "format_id": format_name,
                "protocol": "m3u8_native",
                "ext": "mp4",
            })),
            "mpd" => formats.push(serde_json::json!({
                "url": video_url,
                "format_id": format_name,
                "protocol": "http_dash_segments",
                "ext": "mp4",
            })),
            "ism" => has_smooth_streaming = true,
            "mp4" => formats.push(serde_json::json!({
                "url": video_url,
                "format_id": format_name,
                "ext": "mp4",
            })),
            _ => {}
        }
    }
    if formats.is_empty() && has_smooth_streaming {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            format!(
                "TODO: Toggle video {video_id} exposes Microsoft Smooth Streaming (ISM), which is not implemented natively"
            ),
        ));
    }
    Ok(formats)
}

fn toggle_thumbnails(info: &serde_json::Value) -> Vec<serde_json::Value> {
    let Ok(size_matcher) = Regex::new(r"(?P<width>\d+)[xX](?P<height>\d+)") else {
        return Vec::new();
    };
    info.get("Pictures")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|picture| {
            let picture_url = json_string(picture, "URL").filter(|value| !value.is_empty())?;
            let mut thumbnail = serde_json::json!({"url": picture_url});
            if let Some(size) = json_string(picture, "PicSize") {
                if let Some(captures) = size_matcher.captures(size).ok().flatten() {
                    if let (Some(width), Some(height)) = (
                        captures
                            .name("width")
                            .and_then(|value| value.as_str().parse::<i64>().ok()),
                        captures
                            .name("height")
                            .and_then(|value| value.as_str().parse::<i64>().ok()),
                    ) {
                        thumbnail["width"] = serde_json::json!(width);
                        thumbnail["height"] = serde_json::json!(height);
                    }
                }
            }
            Some(thumbnail)
        })
        .collect()
}

fn toggle_counter(info: &serde_json::Value, prefix: &str) -> Option<i64> {
    let lowercase_key = format!("{}_counter", prefix.to_ascii_lowercase());
    json_i64(info, &format!("{prefix}Counter"))
        .or_else(|| json_i64(info, &lowercase_key))
}
