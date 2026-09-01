const MINDS_API_BASE: &str = "https://www.minds.com/api";

fn minds_xsrf_token(context: &ExtractionContext) -> Option<String> {
    let cookies = context
        .cookie_jar()
        .lock()
        .ok()?
        .cookie_header("https://www.minds.com/api/")
        .ok()??;
    cookies.split(';').find_map(|cookie| {
        let (name, value) = cookie.trim().split_once('=')?;
        (name == "XSRF-TOKEN" && !value.is_empty()).then(|| percent_decode(value))
    })
}

fn minds_api_json(
    context: &ExtractionContext,
    path: &str,
    query: &[(String, String)],
    label: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let endpoint = format!("{MINDS_API_BASE}/{path}");
    let mut request = Request::new(&endpoint);
    if !query.is_empty() {
        request.update_query(query);
    }
    request.headers_mut().set("Referer", "https://www.minds.com/");
    request
        .headers_mut()
        .set("X-XSRF-TOKEN", minds_xsrf_token(context).unwrap_or_default());
    let response = context.request(&request)?;
    serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Minds {label} JSON: {error}"),
        )
    })
}

fn minds_value_string(value: Option<&serde_json::Value>) -> Option<String> {
    value.and_then(|value| {
        value
            .as_str()
            .map(str::to_owned)
            .or_else(|| value.as_i64().map(|value| value.to_string()))
            .or_else(|| value.as_u64().map(|value| value.to_string()))
            .filter(|value| !value.is_empty())
    })
}

fn minds_valid_http_url(value: Option<String>) -> Option<String> {
    let value = value?;
    let value = value.trim();
    let parsed = url::Url::parse(value).ok()?;
    matches!(parsed.scheme(), "http" | "https").then(|| value.to_owned())
}
