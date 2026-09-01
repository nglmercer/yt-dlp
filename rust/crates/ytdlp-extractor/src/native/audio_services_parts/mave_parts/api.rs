const MAVE_API_BASE_URL: &str = "https://api.mave.digital/v1/website";
const MAVE_STORAGE_BASE_URL: &str = "https://store.cloud.mts.ru/mave/";
const MAVE_PAGE_SIZE: i64 = 50;

fn mave_channel_meta(
    context: &ExtractionContext,
    channel_id: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let endpoint = format!("{MAVE_API_BASE_URL}/{channel_id}/");
    let response = context.get_json(&endpoint)?;
    response.get("podcast").cloned().ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Mave channel {channel_id} has no podcast metadata"),
        )
    })
}

fn mave_episode_meta(
    context: &ExtractionContext,
    channel_id: &str,
    episode_code: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let endpoint = format!("{MAVE_API_BASE_URL}/{channel_id}/episodes/{episode_code}");
    context.get_json(&endpoint)
}

fn mave_episode_page(
    context: &ExtractionContext,
    channel_id: &str,
    page: i64,
) -> Result<serde_json::Value, ExtractorError> {
    let endpoint = format!("{MAVE_API_BASE_URL}/{channel_id}/episodes");
    let mut request = Request::new(&endpoint);
    request.update_query(&[
        ("view".to_owned(), "all".to_owned()),
        ("page".to_owned(), (page + 1).to_string()),
        ("sort".to_owned(), "newest".to_owned()),
        ("format".to_owned(), "all".to_owned()),
    ]);
    let response = context.request(&request)?;
    serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Mave episode page JSON from {}: {error}", response.url()),
        )
    })
}
