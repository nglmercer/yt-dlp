fn loc_media_id(webpage: &str) -> Result<String, ExtractorError> {
    let patterns = [
        r#"(?is)\bid\s*=\s*["']media-player-(?P<id>.+?)["']"#,
        r#"(?is)<video[^>]+\bid\s*=\s*["']uuid-(?P<id>.+?)["']"#,
        r#"(?is)<video[^>]+\bdata-uuid\s*=\s*["'](?P<id>.+?)["']"#,
        r#"(?is)\bmediaObjectId\s*:\s*["'](?P<id>.+?)["']"#,
        r#"(?is)\bdata-tab\s*=\s*["']share-media-(?P<id>[0-9A-F]{32})["']"#,
    ];
    for pattern in patterns {
        let Some(media_id) = Regex::new(pattern)
            .ok()
            .and_then(|matcher| matcher.captures(webpage).ok().flatten())
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
        else {
            continue;
        };
        if !media_id.is_empty() {
            return Ok(media_id);
        }
    }
    Err(ExtractorError::new(
        ExtractorErrorKind::Extraction,
        "Library of Congress page has no media ID",
    ))
}

fn loc_media_object(
    context: &ExtractionContext,
    media_id: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let payload = context.get_json(&format!(
        "https://media.loc.gov/services/v1/media?id={media_id}&context=json"
    ))?;
    payload
        .get("mediaObject")
        .cloned()
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Library of Congress media {media_id} has no mediaObject"),
            )
        })
}

fn loc_parse_filesize(value: &str) -> Option<i64> {
    let captures = Regex::new(
        r#"(?ix)^\s*(?P<number>[0-9]+(?:[.,][0-9]+)?)\s*(?P<unit>[a-z]+)?\s*$"#,
    )
    .ok()?
    .captures(value)
    .ok()
    .flatten()?;
    let number = captures
        .name("number")?
        .as_str()
        .replace(',', ".")
        .parse::<f64>()
        .ok()?;
    let unit = captures
        .name("unit")
        .map(|value| value.as_str().to_ascii_lowercase())
        .unwrap_or_else(|| "b".to_owned());
    let multiplier = match unit.as_str() {
        "b" | "byte" | "bytes" => 1.0,
        "kb" | "kilobyte" | "kilobytes" => 1_000.0,
        "kib" | "kibibyte" | "kibibytes" => 1_024.0,
        "mb" | "megabyte" | "megabytes" => 1_000_000.0,
        "mib" | "mebibyte" | "mebibytes" => 1_048_576.0,
        "gb" | "gigabyte" | "gigabytes" => 1_000_000_000.0,
        "gib" | "gibibyte" | "gibibytes" => 1_073_741_824.0,
        "tb" | "terabyte" | "terabytes" => 1_000_000_000_000.0,
        "tib" | "tebibyte" | "tebibytes" => 1_099_511_627_776.0,
        _ => return None,
    };
    Some((number * multiplier).round() as i64)
}
