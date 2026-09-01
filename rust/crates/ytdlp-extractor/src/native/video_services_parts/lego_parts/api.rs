fn lego_uuid(value: &str) -> Option<String> {
    if value.len() != 32 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    Some(format!(
        "{}-{}-{}-{}-{}",
        &value[0..8],
        &value[8..12],
        &value[12..16],
        &value[16..20],
        &value[20..32]
    ))
}

fn lego_item(
    context: &ExtractionContext,
    video_id: &str,
    locale: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let video_uuid = lego_uuid(video_id).ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::InvalidUrl,
            format!("LEGO video ID {video_id} is not a 32-character UUID"),
        )
    })?;
    let mut request = Request::new("https://services.slingshot.lego.com/mediaplayer/v2");
    request.update_query(&[("videoId".to_owned(), format!("{video_uuid}_{locale}"))]);
    let response = context.request_with_status(&request, &[451])?;
    if response.status() == 451 {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            format!("TODO: LEGO video {video_id} is geo-restricted for locale {locale}"),
        ));
    }
    serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid LEGO media-player JSON for {video_id}: {error}"),
        )
    })
}
