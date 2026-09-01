fn mbn_http_url(value: &str) -> Option<String> {
    let value = value.trim();
    let parsed = url::Url::parse(value).ok()?;
    matches!(parsed.scheme(), "http" | "https").then(|| value.to_owned())
}

fn mbn_manifest_url(value: &str) -> Option<String> {
    let value = mbn_http_url(value)?;
    Regex::new(r#"/(?:chunk|play)list(?:_pd\d+)?\.m3u8"#)
        .ok()
        .map(|matcher| matcher.replace(&value, "/manifest.m3u8").into_owned())
}

fn mbn_formats(
    context: &ExtractionContext,
    media_info: &serde_json::Value,
    video_id: &str,
) -> Result<Vec<serde_json::Value>, ExtractorError> {
    let mut formats = Vec::new();
    let Some(movie_list) = media_info
        .get("movie_list")
        .and_then(serde_json::Value::as_array)
    else {
        return Ok(formats);
    };
    for (index, movie) in movie_list.iter().enumerate() {
        let Some(raw_url) = json_string(movie, "url") else {
            continue;
        };
        let Some(manifest_url) = mbn_manifest_url(raw_url) else {
            continue;
        };
        let Some(authenticated_url) = mbn_authenticated_manifest(context, &manifest_url)? else {
            continue;
        };
        formats.push(serde_json::json!({
            "url": authenticated_url,
            "format_id": format!("hls-{index}"),
            "ext": "mp4",
            "protocol": "m3u8_native",
        }));
    }
    if formats.is_empty() {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            format!(
                "TODO: MBN video {video_id} has no authenticated native HLS manifest"
            ),
        ));
    }
    Ok(formats)
}
