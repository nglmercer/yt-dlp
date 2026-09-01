const LECTURIO_API_BASE: &str = "https://app.lecturio.com/api/en/latest/html5/";

fn lecturio_get_json(
    context: &ExtractionContext,
    path: &str,
    display_id: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let endpoint = format!("{LECTURIO_API_BASE}{path}");
    let response = context.request_with_status(&Request::new(&endpoint), &[401, 403])?;
    if response.status() == 401 || response.status() == 403 {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            format!(
                "TODO: Lecturio resource {display_id} requires an authenticated session"
            ),
        ));
    }
    serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Lecturio API JSON for {display_id}: {error}"),
        )
    })
}

fn lecturio_capture_id<'a>(
    matcher: &Regex,
    url: &'a str,
) -> Result<fancy_regex::Captures<'a>, ExtractorError> {
    matcher
        .captures(url)
        .map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Lecturio URL matcher failed: {error}"),
            )
        })?
        .ok_or_else(|| {
            ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Lecturio URL has no match")
        })
}
