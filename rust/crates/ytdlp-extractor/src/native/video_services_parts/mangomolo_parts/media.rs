fn mangomolo_hidden_value(webpage: &str, name: &str) -> Option<String> {
    html_named_input_value(webpage, name).filter(|value| !value.trim().is_empty())
}

fn mangomolo_stream_url(webpage: &str, video_id: &str) -> Result<String, ExtractorError> {
    Regex::new(r#"(?is)(?:file|src)\s*:\s*"(https?://[^"]+?/playlist\.m3u8)"#)
        .ok()
        .and_then(|matcher| matcher.captures(webpage).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().to_owned())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Mangomolo player {video_id} has no HLS playlist"),
            )
        })
}

fn mangomolo_hls_format(stream_url: &str, is_live: bool) -> serde_json::Value {
    serde_json::json!({
        "url": stream_url,
        "format_id": "hls",
        "ext": "mp4",
        "protocol": if is_live { "m3u8" } else { "m3u8_native" },
    })
}
