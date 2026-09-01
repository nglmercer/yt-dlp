const LOCIPO_API_BASE: &str = "https://web-api.locipo.jp";
const LOCIPO_BASE_URL: &str = "https://locipo.jp";
const LOCIPO_PROJECT_ID: &str = "locipo-prod";
const LOCIPO_PAGE_SIZE: i64 = 100;

fn locipo_json_request(
    context: &ExtractionContext,
    endpoint: &str,
    query: &[(String, String)],
    headers: &[(&str, &str)],
    description: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let mut request = Request::new(endpoint);
    request.update_query(query);
    for (name, value) in headers {
        request.headers_mut().set(*name, *value);
    }
    let response = context.request(&request)?;
    serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Locipo {description} JSON: {error}"),
        )
    })
}

fn locipo_api(
    context: &ExtractionContext,
    path: &str,
    query: &[(String, String)],
    description: &str,
) -> Result<serde_json::Value, ExtractorError> {
    locipo_json_request(
        context,
        &format!("{LOCIPO_API_BASE}/{path}"),
        query,
        &[("Accept", "application/json")],
        description,
    )
}

fn locipo_creative(
    context: &ExtractionContext,
    creative_id: &str,
) -> Result<serde_json::Value, ExtractorError> {
    locipo_api(
        context,
        &format!("creatives/{creative_id}"),
        &[],
        "creative",
    )
}

fn locipo_streaks_playback(
    context: &ExtractionContext,
    media_id: &str,
    api_key: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let endpoint =
        format!("https://playback.api.streaks.jp/v1/projects/{LOCIPO_PROJECT_ID}/medias/{media_id}");
    let mut request = Request::new(endpoint);
    request.headers_mut().set("Accept", "application/json");
    request.headers_mut().set("Origin", "https://locipo.jp");
    request.headers_mut().set("X-Streaks-Api-Key", api_key);
    let response = context.request_with_status(&request, &[403, 404])?;
    if response.status() == 403 || response.status() == 404 {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            format!(
                "TODO: Locipo Streaks media {media_id} is unavailable or requires a different API key"
            ),
        ));
    }
    serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Locipo Streaks playback JSON for {media_id}: {error}"),
        )
    })
}

fn locipo_api_key(webpage: &str, video_id: &str) -> Result<String, ExtractorError> {
    let config = json_object_after_marker(webpage, "window.__NUXT__.config").ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            format!(
                "TODO: Locipo video {video_id} has no native Nuxt playback configuration"
            ),
        )
    })?;
    json_string(
        config
            .get("public")
            .unwrap_or(&serde_json::Value::Null),
        "streaksVodPlaybackApiKey",
    )
    .filter(|value| !value.is_empty())
    .map(str::to_owned)
    .ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            format!("TODO: Locipo video {video_id} has no public Streaks API key"),
        )
    })
}

fn locipo_playlist_page(
    context: &ExtractionContext,
    playlist_type: &str,
    playlist_id: &str,
    page: i64,
) -> Result<serde_json::Value, ExtractorError> {
    let path = if playlist_type == "playlist" {
        "playlists"
    } else {
        "series"
    };
    let query = vec![
        ("premium".to_owned(), "false".to_owned()),
        ("live".to_owned(), "false".to_owned()),
        ("limit".to_owned(), LOCIPO_PAGE_SIZE.to_string()),
        (
            "offset".to_owned(),
            ((page.saturating_sub(1)) * LOCIPO_PAGE_SIZE).to_string(),
        ),
    ];
    locipo_api(
        context,
        &format!("{path}/{playlist_id}/creatives"),
        &query,
        &format!("{path} page {page}"),
    )
}
