fn loco_millis_timestamp(value: Option<i64>) -> Option<i64> {
    value.map(|value| if value.abs() >= 100_000_000_000 { value / 1000 } else { value })
}

fn loco_formats(
    stream: &serde_json::Value,
    video_id: &str,
    is_live: bool,
) -> Result<Vec<serde_json::Value>, ExtractorError> {
    let hls_url = stream
        .get("conf")
        .and_then(|conf| json_string(conf, "hls"))
        .filter(|value| !value.is_empty())
        .map(|value| proto_relative_url(value, "https:"))
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Loco stream {video_id} has no HLS URL"),
            )
        })?;
    let mut format = serde_json::json!({
        "url": hls_url,
        "format_id": "hls",
        "ext": "mp4",
        "protocol": "m3u8_native",
    });
    if is_live {
        format["live"] = serde_json::json!(true);
    }
    Ok(vec![format])
}

fn loco_stream_info(
    stream: &serde_json::Value,
    video_id: &str,
    is_live: bool,
) -> Result<InfoDict, ExtractorError> {
    let formats = loco_formats(stream, video_id, is_live)?;
    let mut info = InfoDict::new();
    info.insert("id", serde_json::json!(video_id));
    info.insert("formats", serde_json::Value::Array(formats));
    info.insert("is_live", serde_json::json!(is_live));
    info.insert_if_some("title", json_string(stream, "title"));
    info.insert_if_some("description", json_string(stream, "description"));
    info.insert_if_some("series", json_string(stream, "game_name"));
    info.insert_if_some("uploader_id", json_string(stream, "user_uid"));
    info.insert_if_some("channel", json_string(stream, "alias"));
    info.insert_if_some(
        "concurrent_view_count",
        json_i64(stream, "viewersCurrent"),
    );
    info.insert_if_some("view_count", json_i64(stream, "total_views"));
    info.insert_if_some(
        "thumbnail",
        json_string(stream, "thumbnail_url_small").map(|value| proto_relative_url(value, "https:")),
    );
    info.insert_if_some("like_count", json_i64(stream, "likes"));
    info.insert_if_some("tags", stream.get("tags").cloned());
    info.insert_if_some(
        "timestamp",
        loco_millis_timestamp(json_i64(stream, "started_at")),
    );
    info.insert_if_some(
        "modified_timestamp",
        loco_millis_timestamp(json_i64(stream, "updated_at")),
    );
    info.insert_if_some("comment_count", json_i64(stream, "comments_count"));
    info.insert_if_some(
        "channel_follower_count",
        json_i64(stream, "followers_count"),
    );
    info.insert_if_some("duration", json_i64(stream, "duration"));
    Ok(info)
}
