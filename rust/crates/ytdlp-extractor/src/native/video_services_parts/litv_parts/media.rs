fn litv_formats(
    video_data: &serde_json::Value,
    video_id: &str,
) -> Result<Vec<serde_json::Value>, ExtractorError> {
    let media_url = video_data
        .get("result")
        .and_then(|result| result.get("AssetURLs"))
        .and_then(serde_json::Value::as_array)
        .and_then(|urls| urls.first())
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("LiTV video {video_id} has no HLS asset URL"),
            )
        })?;
    Ok(vec![serde_json::json!({
        "url": media_url,
        "format_id": "hls",
        "ext": "mp4",
        "protocol": "m3u8_native",
        "http_headers": {"Accept-Encoding": "identity"},
    })])
}

fn litv_program_title(program_info: &serde_json::Value) -> Option<String> {
    let title = json_string(program_info, "title").unwrap_or_default();
    let secondary = json_string(program_info, "secondary_mark").unwrap_or_default();
    let value = format!("{title}{secondary}");
    (!value.is_empty()).then_some(value)
}

fn litv_playlist_entries(playlist_data: &serde_json::Value) -> Vec<InfoDict> {
    let content_type = playlist_data
        .get("content_type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("drama");
    let mut entries = Vec::new();
    if let Some(seasons) = playlist_data
        .get("seasons")
        .and_then(serde_json::Value::as_array)
    {
        for season in seasons {
            let Some(episodes) = season
                .get("episodes")
                .and_then(serde_json::Value::as_array)
            else {
                continue;
            };
            for episode in episodes {
                let Some(content_id) = json_string(episode, "content_id") else {
                    continue;
                };
                let mut entry = native_url_result(&format!(
                    "https://www.litv.tv/{content_type}/watch/{content_id}?force_noplaylist=1"
                ));
                entry.insert("ie_key", serde_json::json!("LiTV"));
                entry.insert("id", serde_json::json!(content_id));
                entries.push(entry);
            }
        }
    }
    entries
}
