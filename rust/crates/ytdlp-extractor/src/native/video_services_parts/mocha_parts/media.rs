fn mocha_media_url(value: &serde_json::Value) -> Option<String> {
    let raw_url = value
        .as_str()
        .or_else(|| json_string(value, "video_path"))
        .or_else(|| json_string(value, "url"))?
        .trim();
    (!raw_url.is_empty()).then(|| raw_url.to_owned())
}

fn mocha_media_format(media_url: String, format_id: &str) -> serde_json::Value {
    let extension = yt_dlp_core::determine_ext(Some(&media_url), "mp4");
    let (protocol, ext) = match extension.as_str() {
        "m3u8" => ("m3u8_native", "mp4"),
        "mpd" => ("http_dash_segments", "mp4"),
        _ => ("http", extension.as_str()),
    };
    serde_json::json!({
        "url": media_url,
        "format_id": format_id,
        "ext": ext,
        "protocol": protocol,
    })
}

fn mocha_formats(video: &serde_json::Value, video_id: &str) -> Result<Vec<serde_json::Value>, ExtractorError> {
    let mut formats = Vec::new();
    if let Some(resolutions) = video
        .get("list_resolution")
        .and_then(serde_json::Value::as_array)
    {
        for (index, resolution) in resolutions.iter().enumerate() {
            let Some(media_url) = mocha_media_url(resolution) else {
                continue;
            };
            let format_id = if resolution.is_object() {
                json_string(resolution, "resolution")
                    .or_else(|| json_string(resolution, "name"))
                    .map_or_else(|| format!("resolution-{index}"), str::to_owned)
            } else {
                format!("resolution-{index}")
            };
            formats.push(mocha_media_format(media_url, &format_id));
        }
    }
    if let Some(original_path) = video.get("original_path") {
        if let Some(media_url) = mocha_media_url(original_path) {
            formats.push(mocha_media_format(media_url, "original"));
        }
    }
    if formats.is_empty() {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Mocha video {video_id} has no playable media URLs"),
        ));
    }
    Ok(formats)
}
