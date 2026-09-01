fn melonvod_json(
    context: &ExtractionContext,
    endpoint: &str,
    query: &[(String, String)],
) -> Result<serde_json::Value, ExtractorError> {
    let mut request = Request::new(endpoint);
    request.update_query(query);
    let response = context.request(&request)?;
    serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Melon VOD JSON from {endpoint}: {error}"),
        )
    })
}

fn melonvod_player_info(
    context: &ExtractionContext,
    video_id: &str,
) -> Result<serde_json::Value, ExtractorError> {
    melonvod_json(
        context,
        "http://vod.melon.com/video/playerInfo.json",
        &[("mvId".to_owned(), video_id.to_owned())],
    )
}

fn melonvod_streaming_info(
    context: &ExtractionContext,
    video_id: &str,
) -> Result<serde_json::Value, ExtractorError> {
    melonvod_json(
        context,
        "http://vod.melon.com/delivery/streamingInfo.json",
        &[
            ("contsId".to_owned(), video_id.to_owned()),
            ("contsType".to_owned(), "VIDEO".to_owned()),
        ],
    )
}
