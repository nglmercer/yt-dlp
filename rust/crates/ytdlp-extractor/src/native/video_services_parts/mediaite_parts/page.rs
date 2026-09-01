fn mediaite_page(
    context: &ExtractionContext,
    url: &str,
) -> Result<String, ExtractorError> {
    let response = context.get(url)?;
    Ok(String::from_utf8_lossy(response.body()).into_owned())
}

fn mediaite_video_id(webpage: &str, matchers: &[Regex]) -> Result<String, ExtractorError> {
    matchers
        .iter()
        .find_map(|matcher| {
            matcher
                .captures(webpage)
                .ok()
                .flatten()
                .and_then(|captures| captures.name("id"))
                .map(|value| value.as_str().to_owned())
        })
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Mediaite page has no JWPlatform media ID",
            )
        })
}
