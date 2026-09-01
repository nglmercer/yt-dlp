fn museai_page(
    context: &ExtractionContext,
    video_id: &str,
) -> Result<String, ExtractorError> {
    let endpoint = format!("https://muse.ai/embed/{video_id}");
    let response = context.get(&endpoint)?;
    Ok(String::from_utf8_lossy(response.body()).into_owned())
}

fn museai_player_data(
    webpage: &str,
    video_id: &str,
) -> Result<serde_json::Value, ExtractorError> {
    json_object_after_marker(webpage, "player.setData(").ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("MuseAI video {video_id} has no player data"),
        )
    })
}
