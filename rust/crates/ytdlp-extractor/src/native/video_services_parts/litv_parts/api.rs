fn litv_puid(context: &ExtractionContext) -> String {
    if let Ok(jar) = context.cookie_jar().lock() {
        if let Ok(Some(cookie_header)) = jar.cookie_header("https://www.litv.tv/") {
            if let Some(value) = cookie_header.split(';').find_map(|cookie| {
                cookie
                    .trim()
                    .strip_prefix("PUID=")
                    .map(str::to_owned)
            }) {
                if !value.is_empty() {
                    return value;
                }
            }
        }
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!(
        "{:08x}-{:04x}-4{:03x}-8{:03x}-{:012x}",
        (nanos >> 96) as u32,
        (nanos >> 80) as u16,
        ((nanos >> 64) & 0x0fff) as u16,
        ((nanos >> 48) & 0x0fff) as u16,
        nanos & 0xffff_ffff_ffff,
    )
}

fn litv_playback_json(
    context: &ExtractionContext,
    video_id: &str,
    asset_id: &str,
    media_type: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let endpoint = if context
        .cookie_jar()
        .lock()
        .ok()
        .and_then(|jar| jar.cookie_header("https://www.litv.tv/").ok().flatten())
        .is_some_and(|cookies| {
            cookies
                .split(';')
                .any(|cookie| cookie.trim().starts_with("PUID="))
        }) {
        "get-urls"
    } else {
        "get-urls-no-auth"
    };
    let mut request = Request::new(format!("https://www.litv.tv/api/{endpoint}"));
    request.set_method("POST").map_err(map_request_error)?;
    request
        .headers_mut()
        .set("Content-Type", "application/json");
    request.set_data(Some(
        serde_json::to_vec(&serde_json::json!({
            "AssetId": asset_id,
            "MediaType": media_type,
            "puid": litv_puid(context),
        }))
        .map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("could not encode LiTV playback request: {error}"),
            )
        })?,
    ));
    let response = context.request_with_status(&request, &[400, 401, 403, 404])?;
    let data: serde_json::Value = serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid LiTV playback JSON for {video_id}: {error}"),
        )
    })?;
    if let Some(error) = data.get("error") {
        let message = json_string(error, "message").unwrap_or("unknown LiTV error");
        if message.contains("OutsideRegionError") || response.status() == 403 {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!("TODO: LiTV video {video_id} is available in Taiwan only"),
            ));
        }
        return Err(ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("LiTV said: {message}"),
        ));
    }
    if response.status() >= 400 {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Network,
            format!(
                "HTTP {} while extracting LiTV video {video_id}",
                response.status()
            ),
        ));
    }
    Ok(data)
}

fn litv_force_no_playlist(url: &str) -> bool {
    url_query_value(url, "force_noplaylist").is_some_and(|value| {
        value == "1" || value.eq_ignore_ascii_case("true")
    })
}
