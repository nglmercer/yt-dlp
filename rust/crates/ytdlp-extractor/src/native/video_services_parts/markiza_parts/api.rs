fn markiza_video_json(
    context: &ExtractionContext,
    video_id: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let mut request = Request::new("http://videoarchiv.markiza.sk/json/video_jwplayer7.json");
    request.update_query(&[("id".to_owned(), video_id.to_owned())]);
    let response = context.request(&request)?;
    serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Markiza video JSON for {video_id}: {error}"),
        )
    })
}

fn markiza_page_html(
    context: &ExtractionContext,
    url: &str,
) -> Result<String, ExtractorError> {
    let response = context.get(url)?;
    Ok(String::from_utf8_lossy(response.body()).into_owned())
}

fn markiza_value_string(value: Option<&serde_json::Value>) -> Option<String> {
    value.and_then(|value| {
        value
            .as_str()
            .map(str::to_owned)
            .or_else(|| value.as_i64().map(|value| value.to_string()))
            .or_else(|| value.as_u64().map(|value| value.to_string()))
            .filter(|value| !value.is_empty())
    })
}

fn markiza_duration(value: Option<&serde_json::Value>) -> Option<i64> {
    value.and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
            .or_else(|| value.as_f64().map(|value| value as i64))
            .or_else(|| {
                value
                    .as_str()
                    .and_then(|value| value.parse::<i64>().ok())
            })
            .or_else(|| {
                value
                    .as_str()
                    .and_then(yt_dlp_core::parse_duration)
                    .map(|value| value as i64)
            })
    })
}
