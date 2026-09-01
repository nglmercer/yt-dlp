const TOGGLE_API_ENDPOINT: &str =
    "http://tvpapi.as.tvinci.com/v2_9/gateways/jsonpostgw.aspx?m=GetMediaInfo";
const TOGGLE_API_USER: &str = "tvpapi_147";
const TOGGLE_API_PASS: &str = "11111";

fn toggle_api_payload(video_id: &str) -> serde_json::Value {
    serde_json::json!({
        "initObj": {
            "Locale": {
                "LocaleLanguage": "",
                "LocaleCountry": "",
                "LocaleDevice": "",
                "LocaleUserState": 0,
            },
            "Platform": 0,
            "SiteGuid": 0,
            "DomainID": "0",
            "UDID": "",
            "ApiUser": TOGGLE_API_USER,
            "ApiPass": TOGGLE_API_PASS,
        },
        "MediaID": video_id,
        "mediaType": 0,
    })
}

fn toggle_media_info(
    context: &ExtractionContext,
    video_id: &str,
) -> Result<serde_json::Value, ExtractorError> {
    native_post_json(
        context,
        TOGGLE_API_ENDPOINT,
        &toggle_api_payload(video_id),
    )
}

fn mewatch_custom_id(
    context: &ExtractionContext,
    item_id: &str,
) -> Result<String, ExtractorError> {
    let mut request = Request::new(&format!("https://cdn.mewatch.sg/api/items/{item_id}"));
    request.update_query(&[("segments".to_owned(), "all".to_owned())]);
    let data = context.request(&request)?;
    let payload: serde_json::Value = serde_json::from_slice(data.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid MeWatch item JSON for {item_id}: {error}"),
        )
    })?;
    json_value_string(payload.get("customId"))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("MeWatch item {item_id} has no custom ID"),
            )
        })
}
