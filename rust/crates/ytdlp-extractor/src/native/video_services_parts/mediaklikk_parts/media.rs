fn mediaklikk_video_id(player_data: &serde_json::Value) -> Option<String> {
    json_i64(player_data, "contentId")
        .map(|value| value.to_string())
        .or_else(|| {
            json_string(player_data, "contentId")
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
        })
}

fn mediaklikk_hls_url(
    player_json: &serde_json::Value,
    video_id: &str,
) -> Result<String, ExtractorError> {
    player_json
        .get("playlist")
        .and_then(serde_json::Value::as_array)
        .and_then(|playlist| {
            playlist.iter().find_map(|entry| {
                (json_string(entry, "type") == Some("hls")).then(|| json_string(entry, "file"))
            })
        })
        .flatten()
        .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
        .map(str::to_owned)
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("MediaKlikk video {video_id} has no HLS playlist"),
            )
        })
}

fn mediaklikk_format(media_url: String) -> serde_json::Value {
    serde_json::json!({
        "url": media_url,
        "format_id": "hls",
        "ext": "mp4",
        "protocol": "m3u8_native",
    })
}
