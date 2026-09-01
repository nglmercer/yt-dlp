fn le_rotate_right(value: u32, shift: u32) -> u32 {
    value.rotate_right(shift)
}

fn le_time_key() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as u32;
    let constant = 185_025_305_u32;
    (le_rotate_right(now, constant % 17) ^ constant).to_string()
}

fn le_json_request(
    context: &ExtractionContext,
    endpoint: &str,
    query: &[(String, String)],
    description: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let mut request = Request::new(endpoint);
    request.update_query(query);
    let response = context.request(&request)?;
    serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Le {description} JSON: {error}"),
        )
    })
}

fn le_play_json(
    context: &ExtractionContext,
    video_id: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let query = vec![
        ("id".to_owned(), video_id.to_owned()),
        ("platid".to_owned(), "1".to_owned()),
        ("splatid".to_owned(), "105".to_owned()),
        ("format".to_owned(), "1".to_owned()),
        ("source".to_owned(), "1000".to_owned()),
        ("tkey".to_owned(), le_time_key()),
        ("domain".to_owned(), "www.le.com".to_owned()),
        ("region".to_owned(), "cn".to_owned()),
    ];
    let data = le_json_request(
        context,
        "http://player-pc.le.com/mms/out/video/playJson",
        &query,
        "flash playJson",
    )?;
    let playstatus = data
        .get("msgs")
        .and_then(|msgs| msgs.get("playstatus"))
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Le video {video_id} has no play status"),
            )
        })?;
    if json_i64(playstatus, "status") == Some(0) {
        let flag = json_i64(playstatus, "flag").unwrap_or_default();
        if flag == 1 {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!("TODO: Le video {video_id} is geo-restricted"),
            ));
        }
        return Err(ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Le video {video_id} returned play-status flag {flag}"),
        ));
    }
    Ok(data)
}

fn le_node_json(
    context: &ExtractionContext,
    video_id: &str,
    media_url: &str,
) -> Result<serde_json::Value, ExtractorError> {
    le_json_request(
        context,
        media_url,
        &[
            ("m3v".to_owned(), "1".to_owned()),
            ("format".to_owned(), "1".to_owned()),
            ("expect".to_owned(), "3".to_owned()),
            ("tss".to_owned(), "ios".to_owned()),
        ],
        &format!("format node for {video_id}"),
    )
}

fn le_manifest_request(
    context: &ExtractionContext,
    video_id: &str,
    location: &str,
) -> Result<Vec<u8>, ExtractorError> {
    let response = context.get(location)?;
    Ok(le_decrypt_m3u8(response.body(), video_id))
}
