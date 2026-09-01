fn maariv_formats(
    data: &serde_json::Value,
    video_id: &str,
) -> Result<Vec<serde_json::Value>, ExtractorError> {
    let video = data.get("video").ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Maariv video {video_id} has no video data"),
        )
    })?;
    let mut formats = Vec::new();
    if let Some(hls_url) = json_string(video, "url").filter(|value| !value.is_empty()) {
        formats.push(serde_json::json!({
            "url": hls_url,
            "format_id": "hls",
            "ext": "mp4",
            "protocol": "m3u8_native",
        }));
    }
    if let Some(stream_urls) = video
        .get("stream_urls")
        .and_then(serde_json::Value::as_array)
    {
        for stream in stream_urls {
            let Some(media_url) = json_string(stream, "stream_url").filter(|value| !value.is_empty())
            else {
                continue;
            };
            let mut format = serde_json::json!({
                "url": media_url,
                "format_id": "http",
                "ext": yt_dlp_core::determine_ext(Some(media_url), "mp4"),
            });
            if let Some((width, height)) = maariv_resolution(media_url) {
                format["width"] = serde_json::json!(width);
                format["height"] = serde_json::json!(height);
            }
            formats.push(format);
        }
    }
    if formats.is_empty() {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Maariv video {video_id} has no playable streams"),
        ));
    }
    Ok(formats)
}

fn maariv_resolution(media_url: &str) -> Option<(i64, i64)> {
    let matcher = Regex::new(r"(?i)(\d{3,4})x(\d{3,4})").ok()?;
    let captures = matcher.captures(media_url).ok().flatten()?;
    let width = captures.get(1)?.as_str().parse().ok()?;
    let height = captures.get(2)?.as_str().parse().ok()?;
    Some((width, height))
}
