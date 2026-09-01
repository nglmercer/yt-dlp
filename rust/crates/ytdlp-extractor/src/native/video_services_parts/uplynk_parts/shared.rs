fn uplynk_content_info(
    context: &ExtractionContext,
    path: &str,
    session_id: Option<&str>,
    origin: Option<&str>,
) -> Result<InfoDict, ExtractorError> {
    let content_url = uplynk_content_url(path, session_id);
    let mut format = serde_json::json!({
        "url": content_url,
        "format_id": "hls",
        "protocol": "m3u8_native",
        "ext": "mp4",
    });
    if let Some(session_id) = session_id {
        // The source extractor appends this parameter to every media
        // fragment. The native downloader consumes the field directly.
        format["extra_param_to_segment_url"] = serde_json::json!(format!("pbs={session_id}"));
    }
    let asset_url = format!("http://content.uplynk.com/player/assetinfo/{path}.json");
    let asset = context.get_json(&asset_url)?;
    if json_i64(&asset, "error") == Some(1) {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!(
                "Uplynk asset {path} said: {}",
                json_string(&asset, "msg").unwrap_or("unknown error")
            ),
        ));
    }
    let asset_id = json_value_string(asset.get("asset")).ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Uplynk asset {path} has no asset ID"),
        )
    })?;
    let mut info = InfoDict::new();
    info.insert("id", serde_json::json!(asset_id));
    info.insert_if_some("title", json_string(&asset, "desc"));
    info.insert_if_some("thumbnail", json_string(&asset, "default_poster_url"));
    info.insert_if_some("duration", json_f64(&asset, "duration"));
    info.insert_if_some("uploader_id", json_string(&asset, "owner"));
    info.insert(
        "url",
        format
            .get("url")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    );
    info.insert("ext", serde_json::json!("mp4"));
    info.insert("formats", serde_json::Value::Array(vec![format]));
    info.insert("subtitles", serde_json::json!({}));
    info.insert_if_some(
        "http_headers",
        origin.map(|origin| serde_json::json!({"Origin": origin})),
    );
    Ok(info)
}

fn uplynk_content_url(path: &str, session_id: Option<&str>) -> String {
    let mut request = Request::new(format!("http://content.uplynk.com/{path}.m3u8"));
    if let Some(session_id) = session_id {
        request.update_query(&[("pbs".to_owned(), session_id.to_owned())]);
    }
    request.url().to_owned()
}

fn uplynk_preplay_path(url: &str) -> Option<String> {
    let matcher = Regex::new(
        r#"(?i)^https?://[\w-]+\.uplynk\.com/preplay2?/(?P<path>ext/[0-9a-f]{32}/[^/?&]+|[0-9a-f]{32})\.json"#,
    )
    .ok()?;
    matcher
        .captures(url)
        .ok()
        .flatten()
        .and_then(|captures| captures.name("path"))
        .map(|value| value.as_str().to_owned())
}
