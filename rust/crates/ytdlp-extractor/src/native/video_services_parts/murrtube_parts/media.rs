fn murrtube_video_element(page: &str) -> Option<String> {
    let matcher = Regex::new(r#"(?is)<[^>]+\bid\s*=\s*["']video["'][^>]*>"#).ok()?;
    matcher
        .captures(page)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(0).map(|value| value.as_str().to_owned()))
        .or_else(|| {
            let matcher = Regex::new(r#"(?is)<[^>]+\bdata-url\s*=\s*["'][^"']+["'][^>]*\bid\s*=\s*["']video["'][^>]*>"#).ok()?;
            matcher
                .captures(page)
                .ok()
                .flatten()
                .and_then(|captures| captures.get(0).map(|value| value.as_str().to_owned()))
        })
}

fn murrtube_playlist_url(page_url: &str, page: &str, video_id: &str) -> Result<String, ExtractorError> {
    let element = murrtube_video_element(page).ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Murrtube video {video_id} has no video element"),
        )
    })?;
    let raw_url = murrtube_attribute(&element, "data-url").ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Murrtube video {video_id} has no media URL"),
        )
    })?;
    let resolved = resolve_url(page_url, &raw_url);
    let mut parsed = url::Url::parse(&resolved).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Murrtube media URL for {video_id}: {error}"),
        )
    })?;
    parsed.set_query(None);
    parsed.set_fragment(None);
    let playlist = parsed.to_string();
    let matcher = Regex::new(r#"(?i)/([\da-f]+)/index\.m3u8(?:$|[?#])"#).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Murrtube playlist matcher: {error}"),
        )
    })?;
    if !matcher.is_match(&playlist).unwrap_or(false) {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Murrtube video {video_id} has an invalid HLS URL"),
        ));
    }
    Ok(playlist)
}

fn murrtube_count(page: &str, label: &str) -> Option<i64> {
    let pattern = format!(
        r#"(?is)([\d,]+)\s+<span[^>]*>\s*{}\s*</span>"#,
        regex::escape(label)
    );
    Regex::new(&pattern)
        .ok()
        .and_then(|matcher| matcher.captures(page).ok().flatten())
        .and_then(|captures| captures.get(1))
        .and_then(|value| value.as_str().replace(',', "").parse().ok())
}

fn murrtube_uploader(page: &str) -> Option<String> {
    let matcher = Regex::new(
        r#"(?is)<[^>]*\bclass\s*=\s*["'][^"']*\bpl-1\b[^"']*\bis-size-6\b[^"']*\bhas-text-lighter\b[^"']*["'][^>]*>(.*?)</[^>]+>"#,
    )
    .ok()?;
    matcher
        .captures(page)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1).map(|value| html_text_fragment(value.as_str())))
        .filter(|value| !value.is_empty())
}

fn murrtube_thumbnail(page: &str) -> Option<String> {
    let raw = html_meta_value(page, "og:image")?;
    let mut parsed = url::Url::parse(&raw).ok()?;
    parsed.set_query(None);
    parsed.set_fragment(None);
    Some(parsed.to_string())
}
