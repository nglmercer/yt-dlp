const KATSOMO_API_DOMAIN: &str = "api.katsomo.fi";
const KATSOMO_PROTOCOLS: [&str; 2] = ["HLS", "MPD"];

fn katsomo_json_request(
    context: &ExtractionContext,
    endpoint: &str,
    video_id: &str,
    allow_unauthorized: bool,
) -> Result<serde_json::Value, ExtractorError> {
    let request = Request::new(endpoint);
    let accepted_statuses = if allow_unauthorized { &[401][..] } else { &[][..] };
    let response = context.request_with_status(&request, accepted_statuses)?;
    let payload: serde_json::Value = serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Katsomo JSON for {video_id}: {error}"),
        )
    })?;
    if response.status() == 401 {
        let error = payload.get("error").unwrap_or(&serde_json::Value::Null);
        let code = json_string(error, "code").unwrap_or("KATSOMO_UNAUTHORIZED");
        let description = json_string(error, "description").unwrap_or("Katsomo playback is unauthorized");
        let message = match code {
            "ASSET_PLAYBACK_INVALID_GEO_LOCATION" => format!(
                "TODO: Katsomo asset {video_id} is geo-restricted; native Finland geo handling is not implemented ({description})"
            ),
            "SESSION_NOT_AUTHENTICATED" => format!(
                "TODO: Katsomo asset {video_id} requires an authenticated session ({description})"
            ),
            _ => format!("Katsomo asset {video_id} playback error {code}: {description}"),
        };
        return Err(ExtractorError::new(ExtractorErrorKind::Unsupported, message));
    }
    Ok(payload)
}

fn katsomo_asset(
    context: &ExtractionContext,
    video_id: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let endpoint = format!(
        "http://{KATSOMO_API_DOMAIN}/api/web/asset/{video_id}.json"
    );
    katsomo_json_request(context, &endpoint, video_id, false)?
        .get("asset")
        .cloned()
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Katsomo asset {video_id} response has no asset object"),
            )
        })
}

fn katsomo_playback(
    context: &ExtractionContext,
    video_id: &str,
    protocol: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let endpoint = format!(
        "http://{KATSOMO_API_DOMAIN}/api/web/asset/{video_id}/play.json?protocol={protocol}&videoFormat=SMIL+ISMUSP"
    );
    katsomo_json_request(context, &endpoint, video_id, true)?
        .get("playback")
        .cloned()
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Katsomo {protocol} playback for {video_id} has no playback object"),
            )
        })
}
