const YOUTUBE_DEFAULT_CLIENT_NAME: &str = "WEB";
const YOUTUBE_DEFAULT_CLIENT_VERSION: &str = "2.20260708.00.00";
const YOUTUBE_DEFAULT_USER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

fn youtube_page_request(url: &str) -> Request {
    let mut request = Request::new(url);
    request.headers_mut().set("User-Agent", YOUTUBE_DEFAULT_USER_AGENT);
    request
        .headers_mut()
        .set("Accept-Language", "en-US,en;q=0.5");
    request.headers_mut().set("Accept", "text/html,application/xhtml+xml");
    request
}

fn youtube_ytcfg(webpage: &str) -> serde_json::Value {
    json_object_after_marker(webpage, "ytcfg.set").unwrap_or_else(|| serde_json::json!({}))
}

fn youtube_api_context(ytcfg: &serde_json::Value) -> serde_json::Value {
    let mut context = ytcfg
        .get("INNERTUBE_CONTEXT")
        .cloned()
        .filter(serde_json::Value::is_object)
        .unwrap_or_else(|| serde_json::json!({}));
    let context_object = context.as_object_mut().expect("object checked above");
    let client = context_object
        .entry("client".to_owned())
        .or_insert_with(|| serde_json::json!({}));
    let client_object = client.as_object_mut().expect("new client is an object");
    client_object
        .entry("clientName".to_owned())
        .or_insert_with(|| serde_json::json!(YOUTUBE_DEFAULT_CLIENT_NAME));
    client_object
        .entry("clientVersion".to_owned())
        .or_insert_with(|| serde_json::json!(YOUTUBE_DEFAULT_CLIENT_VERSION));
    client_object
        .entry("hl".to_owned())
        .or_insert_with(|| serde_json::json!("en"));
    client_object
        .entry("timeZone".to_owned())
        .or_insert_with(|| serde_json::json!("UTC"));
    client_object
        .entry("utcOffsetMinutes".to_owned())
        .or_insert_with(|| serde_json::json!(0));
    context
}

pub(crate) fn youtube_player_request_payload(
    ytcfg: &serde_json::Value,
    video_id: &str,
    player_po_token: Option<&str>,
) -> serde_json::Value {
    let mut payload = serde_json::json!({
        "context": youtube_api_context(ytcfg),
        "videoId": video_id,
        "contentCheckOk": true,
        "racyCheckOk": true,
    });
    if let Some(po_token) = player_po_token {
        apply_player_po_token(&mut payload, po_token);
    }
    payload
}

/// Build an Innertube API endpoint URL, mirroring `_call_api` (API key from
/// `INNERTUBE_API_KEY`, host from `INNERTUBE_HOST`, `prettyPrint=false`).
pub(crate) fn youtube_api_endpoint(
    ytcfg: &serde_json::Value,
    api: &str,
    what: &str,
) -> Result<String, ExtractorError> {
    let api_key = ytcfg
        .get("INNERTUBE_API_KEY")
        .and_then(serde_json::Value::as_str)
        .filter(|key| !key.is_empty())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!("TODO: YouTube {what} has no INNERTUBE_API_KEY for API fallback"),
            )
        })?;
    let host = ytcfg
        .get("INNERTUBE_HOST")
        .and_then(serde_json::Value::as_str)
        .filter(|host| !host.is_empty())
        .unwrap_or("www.youtube.com");
    let mut endpoint =
        url::Url::parse(&format!("https://{host}/youtubei/v1/{api}")).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid YouTube {api} API endpoint: {error}"),
            )
        })?;
    endpoint
        .query_pairs_mut()
        .append_pair("key", api_key)
        .append_pair("prettyPrint", "false");
    Ok(endpoint.into())
}

fn youtube_api_response(
    context: &ExtractionContext,
    ytcfg: &serde_json::Value,
    video_id: &str,
    player_po_token: Option<&str>,
    visitor_data: Option<&str>,
) -> Result<serde_json::Value, ExtractorError> {
    let endpoint = youtube_api_endpoint(ytcfg, "player", &format!("video {video_id}"))?;
    let payload = youtube_player_request_payload(ytcfg, video_id, player_po_token);
    let mut request = Request::new(endpoint);
    request.set_method("POST").map_err(map_request_error)?;
    request.headers_mut().set("Accept", "application/json");
    request.headers_mut().set("Content-Type", "application/json");
    request.headers_mut().set("Origin", "https://www.youtube.com");
    request
        .headers_mut()
        .set("X-YouTube-Client-Name", "1");
    request
        .headers_mut()
        .set("X-YouTube-Client-Version", YOUTUBE_DEFAULT_CLIENT_VERSION);
    // Mirrors the `X-Goog-Visitor-Id` half of `generate_api_headers`: an
    // explicit value wins, otherwise the header is dropped (`filter_dict`).
    // Cookie/SAPISID authorization headers stay TODO.
    if let Some(visitor_data) = visitor_data.filter(|value| !value.is_empty()) {
        request.headers_mut().set("X-Goog-Visitor-Id", visitor_data);
    }
    request
        .headers_mut()
        .set("User-Agent", YOUTUBE_DEFAULT_USER_AGENT);
    request.set_data(Some(serde_json::to_vec(&payload).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("could not encode YouTube player API request: {error}"),
        )
    })?));
    let response = context.request(&request)?;
    serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid YouTube player API response for {video_id}: {error}"),
        )
    })
}
