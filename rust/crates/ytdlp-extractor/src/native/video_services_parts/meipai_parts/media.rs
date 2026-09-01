fn meipai_media_formats(
    page_url: &str,
    webpage: &str,
    video_id: &str,
) -> Result<Vec<serde_json::Value>, ExtractorError> {
    let hls_matcher = Regex::new(
        r#"(?is)\bfile\s*:\s*encodeURIComponent\(\s*["']([^"']+)["']\s*\)"#,
    )
    .map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Meipai HLS matcher: {error}"),
        )
    })?;
    if let Some(media_url) = hls_matcher
        .captures(webpage)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| resolve_url(page_url, &unescape_html_attribute(value.as_str())))
    {
        return Ok(vec![serde_json::json!({
            "url": media_url,
            "format_id": "hls",
            "ext": "mp4",
            "protocol": "m3u8_native",
        })]);
    }
    let direct_matcher = Regex::new(
        r#"(?is)\bdata-video\s*=\s*["']([^"']+)["']"#,
    )
    .map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Meipai direct media matcher: {error}"),
        )
    })?;
    if let Some(media_url) = direct_matcher
        .captures(webpage)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| resolve_url(page_url, &unescape_html_attribute(value.as_str())))
    {
        let ext = yt_dlp_core::determine_ext(Some(&media_url), "mp4");
        return Ok(vec![serde_json::json!({
            "url": media_url,
            "format_id": "http",
            "ext": ext,
            "protocol": "http",
        })]);
    }
    Err(ExtractorError::new(
        ExtractorErrorKind::Unsupported,
        format!("TODO: Meipai video {video_id} has no native media URL"),
    ))
}

fn meipai_meta_number(webpage: &str, key: &str) -> Option<i64> {
    html_meta_value(webpage, key)?.trim().parse().ok()
}

fn meipai_meta_duration(webpage: &str) -> Option<f64> {
    meipai_meta_number(webpage, "duration")
        .map(|value| value as f64)
        .or_else(|| {
            html_meta_value(webpage, "duration").and_then(|value| yt_dlp_core::parse_duration(&value))
        })
}

fn meipai_tags(webpage: &str) -> Option<Vec<String>> {
    let tags = html_meta_value(webpage, "video:tag")?
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    (!tags.is_empty()).then_some(tags)
}
