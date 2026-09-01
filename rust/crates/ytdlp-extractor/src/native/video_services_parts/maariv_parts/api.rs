fn maariv_media_data(
    context: &ExtractionContext,
    video_id: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let endpoint = format!(
        "https://dal.walla.co.il/media/{video_id}?origin=player.maariv.co.il"
    );
    let response = context.get(&endpoint)?;
    let payload = serde_json::from_slice::<serde_json::Value>(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Maariv API JSON for {video_id}: {error}"),
        )
    })?;
    payload.get("data").cloned().ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Maariv API response for {video_id} has no data"),
        )
    })
}
