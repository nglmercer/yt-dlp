fn fourtube_player_parameters(
    html: &str,
    page_url: &str,
    video_id: &str,
    context: &ExtractionContext,
) -> Result<(String, Vec<String>), ExtractorError> {
    let player_url = Regex::new(
        r#"(?is)<script\b[^>]*\bid\s*=\s*["']playerembed["'][^>]*\bsrc\s*=\s*["']([^"']+)["']"#,
    )
    .ok()
    .and_then(|matcher| matcher.captures(html).ok().flatten())
    .and_then(|captures| captures.get(1))
    .map(|value| resolve_url(page_url, value.as_str()))
    .ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            format!(
                "TODO: FourTube-family video {video_id} requires an unrecognized \
                 player bootstrap script"
            ),
        )
    })?;
    let response = context.get(&player_url)?;
    let player_js = String::from_utf8_lossy(response.body());
    let params = Regex::new(
        r#"\$\.ajax\(url,\s*opts\);\s*\}\s*\}\)\s*\(([0-9,\[\] ]+)\)"#,
    )
    .ok()
    .and_then(|matcher| matcher.captures(&player_js).ok().flatten())
    .and_then(|captures| captures.get(1))
    .and_then(|value| parse_common_javascript_value(&format!("[{}]", value.as_str())))
    .and_then(|value| value.as_array().cloned())
    .ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            format!(
                "TODO: FourTube-family video {video_id} has an unsupported \
                 player bootstrap parameter format"
            ),
        )
    })?;
    let media_id = params
        .first()
        .and_then(|value| json_value_string(Some(value)))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("FourTube-family video {video_id} player data has no media ID"),
            )
        })?;
    let sources = params
        .get(2)
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|value| json_value_string(Some(value)))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    Ok((media_id, sources))
}

fn fourtube_token_formats(
    context: &ExtractionContext,
    page_url: &str,
    video_id: &str,
    token_host: &str,
    media_id: &str,
    sources: &[String],
) -> Result<Vec<serde_json::Value>, ExtractorError> {
    let token_url = format!(
        "https://{token_host}/{media_id}/{}/desktop",
        sources.join("+")
    );
    let parsed_url = url::Url::parse(page_url).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::InvalidUrl,
            format!("invalid FourTube-family page URL: {error}"),
        )
    })?;
    let origin = format!(
        "{}://{}",
        parsed_url.scheme(),
        parsed_url.host_str().unwrap_or_default()
    );
    let mut request = Request::new(token_url);
    request.headers_mut().set("Origin", origin);
    request.headers_mut().set("Referer", page_url);
    request.set_data(Some(Vec::new()));
    let response = context.request(&request)?;
    let tokens: serde_json::Value = serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid FourTube-family token response for {video_id}: {error}"),
        )
    })?;
    sources
        .iter()
        .map(|source| {
            let quality = source.parse::<i64>().map_err(|_| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!(
                        "FourTube-family quality {source} for video {video_id} is not numeric"
                    ),
                )
            })?;
            let token = tokens
                .get(source)
                .and_then(|value| json_string(value, "token"))
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    ExtractorError::new(
                        ExtractorErrorKind::Extraction,
                        format!(
                            "FourTube-family token response has no token for {source}p \
                             video {video_id}"
                        ),
                    )
                })?;
            let format_url = token.to_owned();
            let extension = yt_dlp_core::determine_ext(Some(&format_url), "mp4");
            let resolution = format!("{source}p");
            Ok(serde_json::json!({
                "url": format_url,
                "format_id": resolution,
                "resolution": resolution,
                "quality": quality,
                "ext": extension,
            }))
        })
        .collect()
}
