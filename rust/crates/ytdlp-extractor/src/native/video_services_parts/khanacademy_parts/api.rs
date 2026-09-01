const KHAN_PUBLISHED_CONTENT_VERSION: &str = "dc34750f0572c80f5effe7134082fe351143c1e4";
const KHAN_CONTENT_HASH: &str = "3712657851";
const KHAN_CONTENT_ENDPOINT: &str =
    "https://www.khanacademy.org/api/internal/graphql/ContentForPath";

fn khan_content(
    context: &ExtractionContext,
    display_id: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let variables = serde_json::json!({
        "path": display_id,
        "countryCode": "US",
        "kaLocale": "en",
        "clientPublishedContentVersion": KHAN_PUBLISHED_CONTENT_VERSION,
    });
    let mut request = Request::new(KHAN_CONTENT_ENDPOINT);
    request.update_query(&[
        (
            "fastly_cacheable".to_owned(),
            "persist_until_publish".to_owned(),
        ),
        (
            "pcv".to_owned(),
            KHAN_PUBLISHED_CONTENT_VERSION.to_owned(),
        ),
        ("hash".to_owned(), KHAN_CONTENT_HASH.to_owned()),
        (
            "variables".to_owned(),
            serde_json::to_string(&variables).map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("could not encode Khan Academy GraphQL variables: {error}"),
                )
            })?,
        ),
        ("lang".to_owned(), "en".to_owned()),
    ]);
    let response = context.request(&request)?;
    let payload: serde_json::Value = serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Khan Academy GraphQL response: {error}"),
        )
    })?;
    payload
        .get("data")
        .and_then(|data| data.get("contentRoute"))
        .and_then(|route| route.get("listedPathData"))
        .cloned()
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Khan Academy path {display_id} has no listed path data"),
            )
        })
}

fn khan_string_list(value: Option<&serde_json::Value>) -> Option<serde_json::Value> {
    let values = value?.as_array()?;
    let values = values
        .iter()
        .filter_map(serde_json::Value::as_str)
        .map(serde_json::Value::from)
        .collect::<Vec<_>>();
    (!values.is_empty()).then_some(serde_json::Value::Array(values))
}

fn khan_thumbnails(value: Option<&serde_json::Value>) -> Option<serde_json::Value> {
    let thumbnails = value?
        .as_array()?
        .iter()
        .filter_map(|thumbnail| {
            let url = json_string(thumbnail, "url")
                .filter(|url| url.starts_with("http://") || url.starts_with("https://"))?;
            let mut output = serde_json::json!({"url": url});
            if let Some(width) = json_i64(thumbnail, "width") {
                output["width"] = serde_json::json!(width);
            }
            if let Some(height) = json_i64(thumbnail, "height") {
                output["height"] = serde_json::json!(height);
            }
            Some(output)
        })
        .collect::<Vec<_>>();
    (!thumbnails.is_empty()).then_some(serde_json::Value::Array(thumbnails))
}
