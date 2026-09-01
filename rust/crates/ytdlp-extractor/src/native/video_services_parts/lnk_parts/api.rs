const LNK_VIDEO_CONFIG: &str = "https://lnk.lt/api/video/video-config";

fn lnk_video_info(
    context: &ExtractionContext,
    video_id: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let endpoint = format!("{LNK_VIDEO_CONFIG}/{video_id}");
    context
        .get_json(&endpoint)?
        .get("videoInfo")
        .cloned()
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("LNK video {video_id} response has no videoInfo object"),
            )
        })
}
