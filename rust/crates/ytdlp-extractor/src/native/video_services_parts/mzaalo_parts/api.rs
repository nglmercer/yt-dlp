fn mzaalo_data(
    context: &ExtractionContext,
    media_type: &str,
    video_id: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let is_clip = media_type.eq_ignore_ascii_case("clip");
    let endpoint = if is_clip {
        "https://production.mzaalo.com/platform/partner/streamurl"
    } else {
        "https://production.mzaalo.com/platform/api/v2/player/details"
    };
    let mut request = Request::new(endpoint);
    let query = if is_clip {
        vec![
            ("assetId".to_owned(), video_id.to_owned()),
            ("getClipDetails".to_owned(), "YES".to_owned()),
        ]
    } else {
        vec![
            ("assetType".to_owned(), media_type.to_ascii_uppercase()),
            ("assetId".to_owned(), video_id.to_owned()),
        ]
    };
    request.update_query(&query);
    request
        .headers_mut()
        .set("Ocp-Apim-Subscription-Key", "1d0caac2702049b89a305929fdf4cbae");
    let response = context.request(&request)?;
    let payload: serde_json::Value = serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Mzaalo JSON for {video_id}: {error}"),
        )
    })?;
    payload.get("data").cloned().ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Mzaalo response for {video_id} has no data"),
        )
    })
}
