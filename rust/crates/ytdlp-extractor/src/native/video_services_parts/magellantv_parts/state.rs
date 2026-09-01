fn magellantv_react_context<'a>(
    next_data: &'a serde_json::Value,
    video_id: &str,
) -> Result<&'a serde_json::Value, ExtractorError> {
    next_data
        .get("props")
        .and_then(|props| props.get("pageProps"))
        .and_then(|page_props| page_props.get("reactContext"))
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("MagellanTV page {video_id} has no React context"),
            )
        })
}

fn magellantv_video_data<'a>(
    react_context: &'a serde_json::Value,
    video_id: &str,
) -> Result<&'a serde_json::Value, ExtractorError> {
    react_context
        .get("video")
        .and_then(|video| video.get("detail"))
        .filter(|value| value.is_object())
        .or_else(|| {
            react_context
                .get("series")
                .and_then(|series| series.get("currentEpisode"))
                .filter(|value| value.is_object())
        })
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("MagellanTV page {video_id} has no video data"),
            )
        })
}
