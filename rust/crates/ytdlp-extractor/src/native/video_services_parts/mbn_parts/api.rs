fn mbn_page(
    context: &ExtractionContext,
    url: &str,
) -> Result<String, ExtractorError> {
    let response = context.get(url)?;
    Ok(String::from_utf8_lossy(response.body()).into_owned())
}

fn mbn_content_class_code(webpage: &str) -> String {
    Regex::new(r#"(?i)[?&]content_cls_cd=(\d+)&"#)
        .ok()
        .and_then(|matcher| matcher.captures(webpage).ok().flatten())
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
        .unwrap_or_else(|| "20".to_owned())
}

fn mbn_media_info(
    context: &ExtractionContext,
    video_id: &str,
    content_class_code: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let endpoint = "https://www.mbn.co.kr/player/mbnVodPlayer_2020.mbn";
    let mut request = Request::new(endpoint);
    request.update_query(&[
        ("content_cls_cd".to_owned(), content_class_code.to_owned()),
        ("content_id".to_owned(), video_id.to_owned()),
        ("relay_type".to_owned(), "1".to_owned()),
    ]);
    let response = context.request(&request)?;
    serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid MBN playback JSON for {video_id}: {error}"),
        )
    })
}

fn mbn_authenticated_manifest(
    context: &ExtractionContext,
    stream_url: &str,
) -> Result<Option<String>, ExtractorError> {
    let endpoint = "https://www.mbn.co.kr/player/mbnStreamAuth_new_vod.mbn";
    let mut request = Request::new(endpoint);
    request.update_query(&[("vod_url".to_owned(), stream_url.to_owned())]);
    let response = context.request(&request)?;
    let body = String::from_utf8_lossy(response.body());
    let value = body
        .trim()
        .trim_matches('"')
        .trim()
        .to_owned();
    Ok(mbn_http_url(&value))
}
