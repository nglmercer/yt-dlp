fn medaltv_content(
    context: &ExtractionContext,
    video_id: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let endpoint = format!("https://medal.tv/api/content/{video_id}");
    let mut request = Request::new(&endpoint);
    request.headers_mut().set("Accept", "application/json");
    let response = context.request(&request)?;
    serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Medal.tv API JSON for {video_id}: {error}"),
        )
    })
}

fn medaltv_social_url(
    context: &ExtractionContext,
    video_id: &str,
) -> Result<String, ExtractorError> {
    let endpoint = format!("https://medal.tv/api/content/{video_id}/socialVideoUrl");
    Ok(context.get(&endpoint)?.url().to_owned())
}
