fn mocha_video_detail(
    context: &ExtractionContext,
    page_url: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let mut request = Request::new(
        "http://apivideo.mocha.com.vn:8081/onMediaBackendBiz/mochavideo/getVideoDetail",
    );
    request.update_query(&[
        ("url".to_owned(), page_url.to_owned()),
        ("token".to_owned(), String::new()),
    ]);
    let response = context.request(&request)?;
    let payload: serde_json::Value = serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Mocha video JSON: {error}"),
        )
    })?;
    payload
        .get("data")
        .and_then(|data| data.get("videoDetail"))
        .cloned()
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Mocha API response has no data.videoDetail",
            )
        })
}
