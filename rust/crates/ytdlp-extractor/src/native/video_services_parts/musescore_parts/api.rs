fn musescore_page(
    context: &ExtractionContext,
    url: &str,
) -> Result<String, ExtractorError> {
    let response = context.get(url)?;
    Ok(String::from_utf8_lossy(response.body()).into_owned())
}

fn musescore_audio_url(
    context: &ExtractionContext,
    video_id: &str,
) -> Result<String, ExtractorError> {
    let endpoint = "https://musescore.com/api/jmuse";
    let mut request = Request::new(endpoint);
    request.headers_mut().set(
        "authorization",
        &native_hex(&native_md5(format!("{video_id}mp30gs").as_bytes()))[..4],
    );
    request.update_query(&[
        ("id".to_owned(), video_id.to_owned()),
        ("index".to_owned(), "0".to_owned()),
        ("type".to_owned(), "mp3".to_owned()),
    ]);
    let response = context.request(&request)?;
    let data: serde_json::Value = serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid MuseScore API JSON for {video_id}: {error}"),
        )
    })?;
    data.get("info")
        .and_then(|info| json_string(info, "url"))
        .filter(|url| url.starts_with("http://") || url.starts_with("https://"))
        .map(str::to_owned)
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("MuseScore API has no MP3 URL for {video_id}"),
            )
        })
}
