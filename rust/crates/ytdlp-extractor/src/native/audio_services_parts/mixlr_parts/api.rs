fn mixlr_api(
    context: &ExtractionContext,
    username: &str,
    resource: &str,
    resource_id: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let endpoint = format!(
        "https://api.mixlr.com/v3/channels/{username}/{resource}/{resource_id}"
    );
    let response = context.get(&endpoint)?;
    serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Mixlr {resource} JSON for {resource_id}: {error}"),
        )
    })
}

fn mixlr_data_attributes(payload: &serde_json::Value) -> &serde_json::Value {
    payload
        .get("data")
        .and_then(|data| data.get("attributes"))
        .unwrap_or(&serde_json::Value::Null)
}

fn mixlr_included_attributes(payload: &serde_json::Value) -> &serde_json::Value {
    payload
        .get("included")
        .and_then(serde_json::Value::as_array)
        .and_then(|included| included.first())
        .and_then(|record| record.get("attributes"))
        .unwrap_or(&serde_json::Value::Null)
}

fn mixlr_attribute_value<'a>(
    primary: &'a serde_json::Value,
    fallback: &'a serde_json::Value,
    key: &str,
) -> Option<&'a serde_json::Value> {
    primary
        .get(key)
        .filter(|value| !value.is_null())
        .or_else(|| fallback.get(key).filter(|value| !value.is_null()))
}

fn mixlr_attribute_string(
    primary: &serde_json::Value,
    fallback: &serde_json::Value,
    key: &str,
) -> Option<String> {
    mixlr_attribute_value(primary, fallback, key).and_then(|value| {
        value
            .as_str()
            .map(str::to_owned)
            .or_else(|| value.as_i64().map(|value| value.to_string()))
            .or_else(|| value.as_u64().map(|value| value.to_string()))
            .filter(|value| !value.is_empty())
    })
}

fn mixlr_attribute_i64(
    primary: &serde_json::Value,
    fallback: &serde_json::Value,
    key: &str,
) -> Option<i64> {
    mixlr_attribute_value(primary, fallback, key).and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
            .or_else(|| value.as_f64().map(|value| value as i64))
            .or_else(|| value.as_str().and_then(|value| value.trim().parse().ok()))
    })
}

fn mixlr_attribute_bool(
    primary: &serde_json::Value,
    fallback: &serde_json::Value,
    key: &str,
) -> Option<bool> {
    mixlr_attribute_value(primary, fallback, key)
        .and_then(serde_json::Value::as_bool)
}
