fn mdr_page(
    context: &ExtractionContext,
    url: &str,
) -> Result<String, ExtractorError> {
    let response = context.get(url)?;
    Ok(String::from_utf8_lossy(response.body()).into_owned())
}

fn mdr_data_url(webpage: &str) -> Result<String, ExtractorError> {
    let matcher = Regex::new(
        r#"(?:dataURL|playerXml(?:["'])?)\s*:\s*(["'])(?P<url>.+?-avCustom\.xml)\1"#,
    )
    .map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid MDR data URL matcher: {error}"),
        )
    })?;
    matcher
        .captures(webpage)
        .ok()
        .flatten()
        .and_then(|captures| captures.name("url"))
        .map(|value| value.as_str().replace(r"\/", "/"))
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "MDR page has no avCustom XML data URL",
            )
        })
}
