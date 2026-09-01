fn massengeschmack_media(
    webpage: &str,
    episode: &str,
) -> Result<serde_json::Value, ExtractorError> {
    json_array_after_marker(webpage, "MEDIA").ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Massengeschmack episode {episode} has no MEDIA array"),
        )
    })
}

fn massengeschmack_title(webpage: &str) -> Option<String> {
    Regex::new(r#"(?is)<span\b[^>]*\bid\s*=\s*["']clip-title["'][^>]*>(.*?)</span\s*>"#)
        .ok()
        .and_then(|matcher| matcher.captures(webpage).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| html_text_fragment(value.as_str()))
        .filter(|value| !value.is_empty())
}

fn massengeschmack_poster(webpage: &str) -> Option<String> {
    Regex::new(r#"(?is)\bPOSTER\s*=\s*"([^"]+)"#)
        .ok()
        .and_then(|matcher| matcher.captures(webpage).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| unescape_html_attribute(value.as_str()))
        .filter(|value| !value.is_empty())
}
