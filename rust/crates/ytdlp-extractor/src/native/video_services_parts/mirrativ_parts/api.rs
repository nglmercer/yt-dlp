fn mirrativ_json(
    context: &ExtractionContext,
    endpoint: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let response = context.get(endpoint)?;
    let data: serde_json::Value = serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Mirrativ JSON from {endpoint}: {error}"),
        )
    })?;
    if let Some(error) = data
        .get("status")
        .and_then(|status| json_string(status, "error"))
        .filter(|error| !error.is_empty())
    {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Mirrativ says: {error}"),
        ));
    }
    Ok(data)
}

fn mirrativ_live_json(
    context: &ExtractionContext,
    video_id: &str,
) -> Result<serde_json::Value, ExtractorError> {
    mirrativ_json(
        context,
        &format!("https://www.mirrativ.com/api/live/live?live_id={video_id}"),
    )
}

fn mirrativ_profile_json(
    context: &ExtractionContext,
    user_id: &str,
) -> Result<serde_json::Value, ExtractorError> {
    mirrativ_json(
        context,
        &format!("https://www.mirrativ.com/api/user/profile?user_id={user_id}"),
    )
}

fn mirrativ_history_json(
    context: &ExtractionContext,
    user_id: &str,
    page: i64,
) -> Result<serde_json::Value, ExtractorError> {
    mirrativ_json(
        context,
        &format!(
            "https://www.mirrativ.com/api/live/live_history?user_id={user_id}&page={page}"
        ),
    )
}

fn mirrativ_value_string(value: &serde_json::Value, key: &str) -> Option<String> {
    json_string(value, key)
        .map(str::to_owned)
        .or_else(|| json_i64(value, key).map(|value| value.to_string()))
}
