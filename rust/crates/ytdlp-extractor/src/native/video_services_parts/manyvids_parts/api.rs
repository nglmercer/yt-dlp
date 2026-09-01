const MANYVIDS_API_BASE: &str = "https://www.manyvids.com/bff/store/video";

fn manyvids_request_json(
    context: &ExtractionContext,
    endpoint: &str,
    video_id: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let mut request = Request::new(endpoint);
    request.headers_mut().set("Accept", "application/json");
    let response = context.request(&request)?;
    serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid ManyVids JSON for {video_id}: {error}"),
        )
    })
}

fn manyvids_data(
    context: &ExtractionContext,
    video_id: &str,
    suffix: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let endpoint = format!("{MANYVIDS_API_BASE}/{video_id}/{suffix}");
    let response = manyvids_request_json(context, &endpoint, video_id)?;
    response.get("data").cloned().ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("ManyVids response for {video_id} has no data"),
        )
    })
}

fn manyvids_optional_data(
    context: &ExtractionContext,
    video_id: &str,
) -> Option<serde_json::Value> {
    let endpoint = format!("{MANYVIDS_API_BASE}/{video_id}");
    let response = manyvids_request_json(context, &endpoint, video_id).ok()?;
    response.get("data").cloned()
}
