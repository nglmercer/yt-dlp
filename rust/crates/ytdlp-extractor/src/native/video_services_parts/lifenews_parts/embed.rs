fn lifenews_embed_add_format(
    formats: &mut Vec<serde_json::Value>,
    page_url: &str,
    raw_url: &str,
) {
    let raw_url = unescape_html_attribute(raw_url).trim().to_owned();
    if raw_url.is_empty() {
        return;
    }
    let media_url = resolve_url(page_url, &proto_relative_url(&raw_url, "https:"));
    let extension = yt_dlp_core::determine_ext(Some(&media_url), "mp4");
    if extension.eq_ignore_ascii_case("m3u8") {
        formats.push(serde_json::json!({
            "url": media_url,
            "format_id": "m3u8",
            "ext": "mp4",
            "protocol": "m3u8_native",
        }));
    } else {
        formats.push(serde_json::json!({
            "url": media_url,
            "format_id": extension,
            "quality": 1,
        }));
    }
}

fn lifenews_embed_media(
    page_url: &str,
    webpage: &str,
    video_id: &str,
) -> Result<(Vec<serde_json::Value>, Option<String>), ExtractorError> {
    let mut formats = Vec::new();
    let mut thumbnail = None;
    if let Some(options) = json_object_after_marker(webpage, "options") {
        if let Some(playlist) = options.get("playlist") {
            if let Some(master) = json_string(playlist, "master")
                .filter(|value| yt_dlp_core::determine_ext(Some(value), "").eq_ignore_ascii_case("m3u8"))
            {
                lifenews_embed_add_format(&mut formats, page_url, master);
            }
            if let Some(original) = json_string(playlist, "original") {
                lifenews_embed_add_format(&mut formats, page_url, original);
            }
            thumbnail = json_string(playlist, "image")
                .map(unescape_html_attribute)
                .filter(|value| !value.is_empty());
        }
    }
    if formats.is_empty() {
        if let Ok(matcher) = Regex::new(r#""file"\s*:\s*"([^"]+)"#) {
            for captures in matcher.captures_iter(webpage).flatten() {
                if let Some(value) = captures.get(1) {
                    lifenews_embed_add_format(&mut formats, page_url, value.as_str());
                }
            }
        }
    }
    if thumbnail.is_none() {
        thumbnail = Regex::new(r#""image"\s*:\s*"([^"]+)"#)
            .ok()
            .and_then(|matcher| matcher.captures(webpage).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| unescape_html_attribute(value.as_str()));
    }
    if formats.is_empty() {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Life.ru embed {video_id} has no playable media URL"),
        ));
    }
    Ok((formats, thumbnail))
}
