fn mirrativ_http_url(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    let parsed = url::Url::parse(value).ok()?;
    matches!(parsed.scheme(), "http" | "https").then(|| value.to_owned())
}

fn mirrativ_hls_format(media_url: String) -> serde_json::Value {
    serde_json::json!({
        "url": media_url,
        "format_id": "hls",
        "ext": "mp4",
        "protocol": "m3u8_native",
    })
}

fn mirrativ_duration(data: &serde_json::Value, is_live: bool) -> Option<i64> {
    if is_live {
        return None;
    }
    let started = json_i64(data, "started_at")?;
    let ended = json_i64(data, "ended_at")?;
    ended.checked_sub(started).filter(|duration| *duration >= 0)
}

fn mirrativ_title(webpage: &str, live_data: &serde_json::Value) -> Option<String> {
    html_meta_value(webpage, "og:title")
        .or_else(|| {
            Regex::new(r#"(?is)<title>\s*(.+?)\s*-\s*Mirrativ\s*</title>"#)
                .ok()
                .and_then(|matcher| matcher.captures(webpage).ok().flatten())
                .and_then(|captures| captures.get(1))
                .map(|value| html_text_fragment(value.as_str()))
        })
        .or_else(|| json_string(live_data, "title").map(str::to_owned))
}

fn mirrativ_live_info(
    video_id: &str,
    webpage: &str,
    live_data: &serde_json::Value,
) -> Result<InfoDict, ExtractorError> {
    let is_live = json_bool(live_data, "is_live").unwrap_or(false);
    let media_url = mirrativ_http_url(
        json_string(live_data, "archive_url_hls")
            .or_else(|| json_string(live_data, "streaming_url_hls")),
    )
    .ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            format!("TODO: Mirrativ live {video_id} has no archive or live HLS stream"),
        )
    })?;
    let format = mirrativ_hls_format(media_url.clone());
    let owner = live_data.get("owner").unwrap_or(&serde_json::Value::Null);
    let mut info = InfoDict::new();
    info.insert("id", serde_json::json!(video_id));
    info.insert_if_some("title", mirrativ_title(webpage, live_data));
    info.insert_if_some("is_live", Some(is_live));
    info.insert_if_some("description", json_string(live_data, "description"));
    info.insert_if_some("thumbnail", json_string(live_data, "image_url"));
    info.insert_if_some("uploader", json_string(owner, "name"));
    info.insert_if_some("uploader_id", mirrativ_value_string(owner, "user_id"));
    info.insert_if_some("duration", mirrativ_duration(live_data, is_live));
    info.insert_if_some("view_count", json_i64(live_data, "total_viewer_num"));
    info.insert_if_some("release_timestamp", json_i64(live_data, "started_at"));
    info.insert_if_some("timestamp", json_i64(live_data, "created_at"));
    info.insert_if_some("was_live", json_bool(live_data, "is_archive"));
    info.insert("url", serde_json::json!(media_url));
    info.insert("ext", serde_json::json!("mp4"));
    info.insert("formats", serde_json::json!([format]));
    Ok(info)
}
