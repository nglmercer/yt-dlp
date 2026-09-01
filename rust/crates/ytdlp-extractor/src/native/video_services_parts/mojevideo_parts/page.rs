fn mojevideo_page(
    context: &ExtractionContext,
    url: &str,
) -> Result<String, ExtractorError> {
    let response = context.get(url)?;
    Ok(String::from_utf8_lossy(response.body()).into_owned())
}

fn mojevideo_capture(page: &str, pattern: &str, label: &str) -> Result<String, ExtractorError> {
    let matcher = Regex::new(pattern).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Mojevideo {label} matcher: {error}"),
        )
    })?;
    matcher
        .captures(page)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Mojevideo page has no {label}"),
            )
        })
}

fn mojevideo_hashes(page: &str) -> Result<Vec<String>, ExtractorError> {
    let hashes = json_array_after_marker(page, "vHash").ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            "Mojevideo page has no valid vHash array",
        )
    })?;
    let values = hashes
        .as_array()
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Mojevideo vHash is not an array",
            )
        })?
        .iter()
        .filter_map(mojevideo_value_string)
        .collect::<Vec<_>>();
    if values.is_empty() {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Extraction,
            "Mojevideo vHash array has no usable hashes",
        ));
    }
    Ok(values)
}

fn mojevideo_value_string(value: &serde_json::Value) -> Option<String> {
    value
        .as_str()
        .map(str::to_owned)
        .or_else(|| value.as_i64().map(|value| value.to_string()))
        .or_else(|| value.as_u64().map(|value| value.to_string()))
        .filter(|value| !value.is_empty())
}
