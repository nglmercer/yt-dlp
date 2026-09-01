fn magentamusik_page_config(webpage: &str) -> Option<serde_json::Value> {
    let matcher = Regex::new(
        r#"(?is)data-js-element\s*=\s*["']o-video-player__config["']\s*>"#,
    )
    .ok()?;
    let marker = matcher.find(webpage).ok().flatten()?;
    json_object_after_marker(&webpage[marker.end()..], "")
}

fn magentamusik_api_json(
    context: &ExtractionContext,
    endpoint: &str,
    label: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let response = context.get(endpoint)?;
    serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid MagentaMusik {label} JSON: {error}"),
        )
    })
}

fn magentamusik_value_string(value: Option<&serde_json::Value>) -> Option<String> {
    value.and_then(|value| {
        value
            .as_str()
            .map(str::to_owned)
            .or_else(|| value.as_i64().map(|value| value.to_string()))
            .or_else(|| value.as_u64().map(|value| value.to_string()))
            .filter(|value| !value.is_empty())
    })
}

fn magentamusik_find_reference(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Object(values) => {
            if let Some(reference) = magentamusik_value_string(values.get("reference")) {
                return Some(reference);
            }
            values.values().find_map(magentamusik_find_reference)
        }
        serde_json::Value::Array(values) => values.iter().find_map(magentamusik_find_reference),
        _ => None,
    }
}

fn magentamusik_valid_http_url(value: Option<String>) -> Option<String> {
    let value = value?;
    let value = value.trim();
    let parsed = url::Url::parse(value).ok()?;
    matches!(parsed.scheme(), "http" | "https").then(|| value.to_owned())
}

fn magentamusik_find_media_href(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Object(values) => {
            if let Some(media) = values.get("media") {
                if let Some(href) = media
                    .get("href")
                    .and_then(|href| magentamusik_value_string(Some(href)))
                {
                    if let Some(href) = magentamusik_valid_http_url(Some(href)) {
                        return Some(href);
                    }
                }
            }
            values.values().find_map(magentamusik_find_media_href)
        }
        serde_json::Value::Array(values) => values.iter().find_map(magentamusik_find_media_href),
        _ => None,
    }
}

fn magentamusik_feature_metadata(vod_data: &serde_json::Value) -> &serde_json::Value {
    vod_data
        .get("content")
        .and_then(|content| content.get("feature"))
        .and_then(|feature| feature.get("metadata"))
        .unwrap_or(&serde_json::Value::Null)
}

fn magentamusik_string_list(value: Option<&serde_json::Value>) -> Option<String> {
    let values = match value? {
        serde_json::Value::Array(values) => values
            .iter()
            .filter_map(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>(),
        serde_json::Value::String(value) if !value.trim().is_empty() => vec![value.trim()],
        _ => Vec::new(),
    };
    (!values.is_empty()).then(|| values.join(", "))
}

fn magentamusik_categories(value: Option<&serde_json::Value>) -> Option<serde_json::Value> {
    let values = match value? {
        serde_json::Value::Array(values) => values
            .iter()
            .filter_map(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| serde_json::json!(value))
            .collect::<Vec<_>>(),
        serde_json::Value::String(value) if !value.trim().is_empty() => {
            vec![serde_json::json!(value.trim())]
        }
        _ => Vec::new(),
    };
    (!values.is_empty()).then_some(serde_json::Value::Array(values))
}
