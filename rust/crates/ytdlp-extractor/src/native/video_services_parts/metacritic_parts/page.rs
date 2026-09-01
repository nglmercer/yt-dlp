fn metacritic_page(
    context: &ExtractionContext,
    url: &str,
) -> Result<String, ExtractorError> {
    let response = context.get(url)?;
    Ok(String::from_utf8_lossy(response.body()).into_owned())
}

fn metacritic_description(page: &str) -> Result<String, ExtractorError> {
    let matcher = Regex::new(r"(?is)<b>\s*Description:\s*</b>(.*?)</p>").map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Metacritic description matcher: {error}"),
        )
    })?;
    matcher
        .captures(page)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1).map(|value| html_text_fragment(value.as_str())))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Metacritic page has no trailer description",
            )
        })
}
