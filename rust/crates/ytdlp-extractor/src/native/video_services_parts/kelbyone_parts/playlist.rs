fn kelbyone_playlist_url(html: &str) -> Option<String> {
    let matcher = Regex::new(
        r#"(?is)playlist"\s*:\s*"([^"]*content\.jwplatform\.com[^"]*\.json[^"]*)"#,
    )
    .ok()?;
    matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1).map(|value| value.as_str().replace('\\', "")))
        .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
}

fn kelbyone_format(source: &serde_json::Value) -> Option<serde_json::Value> {
    let media_url = json_string(source, "file")
        .filter(|value| value.starts_with("http://") || value.starts_with("https://"))?;
    let source_type = json_string(source, "type").unwrap_or_default();
    if source_type.eq_ignore_ascii_case("application/vnd.apple.mpegurl")
        || yt_dlp_core::determine_ext(Some(media_url), "mp4").eq_ignore_ascii_case("m3u8")
    {
        return Some(serde_json::json!({
            "url": media_url,
            "format_id": json_string(source, "label").unwrap_or("hls"),
            "protocol": "m3u8_native",
            "ext": "mp4",
        }));
    }
    let extension = yt_dlp_core::determine_ext(Some(media_url), "mp4");
    let mut format = serde_json::json!({
        "url": media_url,
        "ext": extension,
    });
    if let Some(label) = json_string(source, "label") {
        format["format_id"] = serde_json::json!(label);
    }
    if let Some(width) = json_i64(source, "width") {
        format["width"] = serde_json::json!(width);
    }
    if let Some(height) = json_i64(source, "height") {
        format["height"] = serde_json::json!(height);
    }
    if source_type.eq_ignore_ascii_case("audio/mp4") {
        format["vcodec"] = serde_json::json!("none");
    }
    Some(format)
}

fn kelbyone_subtitles(item: &serde_json::Value) -> serde_json::Value {
    let mut subtitles = serde_json::Map::new();
    if let Some(tracks) = item.get("tracks").and_then(serde_json::Value::as_array) {
        let captions = tracks
            .iter()
            .filter(|track| json_string(track, "kind") == Some("captions"))
            .filter_map(|track| {
                let url = json_string(track, "file")
                    .filter(|value| value.starts_with("http://") || value.starts_with("https://"))?;
                Some(serde_json::json!({"url": url}))
            })
            .collect::<Vec<_>>();
        if !captions.is_empty() {
            subtitles.insert("en".to_owned(), serde_json::Value::Array(captions));
        }
    }
    serde_json::Value::Object(subtitles)
}

fn kelbyone_entry(item: &serde_json::Value) -> Option<InfoDict> {
    let video_id = json_string(item, "mediaid")?.to_owned();
    let mut formats = Vec::new();
    if let Some(sources) = item.get("sources").and_then(serde_json::Value::as_array) {
        formats.extend(sources.iter().filter_map(kelbyone_format));
    }
    let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
    let mut entry = InfoDict::new();
    entry.insert("id", serde_json::json!(video_id));
    entry.insert_if_some("title", json_string(item, "title"));
    entry.insert_if_some("description", json_string(item, "description"));
    entry.insert_if_some("thumbnail", json_string(item, "image"));
    entry.insert_if_some("timestamp", json_i64(item, "pubdate"));
    entry.insert_if_some("duration", json_f64(item, "duration"));
    if let Some(images) = item.get("images").and_then(serde_json::Value::as_array) {
        let thumbnails = images
            .iter()
            .filter_map(|image| {
                let url = json_string(image, "src")
                    .filter(|value| value.starts_with("http://") || value.starts_with("https://"))?;
                let mut thumbnail = serde_json::json!({"url": url});
                if let Some(width) = json_i64(image, "width") {
                    thumbnail["width"] = serde_json::json!(width);
                }
                Some(thumbnail)
            })
            .collect::<Vec<_>>();
        if !thumbnails.is_empty() {
            entry.insert("thumbnails", serde_json::Value::Array(thumbnails));
        }
    }
    entry.insert(
        "url",
        first
            .get("url")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    );
    entry.insert(
        "ext",
        first
            .get("ext")
            .cloned()
            .unwrap_or_else(|| serde_json::json!("mp4")),
    );
    entry.insert("formats", serde_json::Value::Array(formats));
    entry.insert("subtitles", kelbyone_subtitles(item));
    Some(entry)
}
