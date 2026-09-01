fn libsyn_formats(data: &serde_json::Value) -> Vec<serde_json::Value> {
    [
        ("media_url_libsyn", "libsyn"),
        ("media_url", "main"),
        ("download_link", "download"),
    ]
    .into_iter()
    .filter_map(|(key, format_id)| {
        let url = json_string(data, key)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)?;
        Some(serde_json::json!({
            "url": url,
            "format_id": format_id,
        }))
    })
    .collect()
}

fn libsyn_duration(value: Option<&serde_json::Value>) -> Option<f64> {
    value.and_then(|value| {
        value
            .as_f64()
            .or_else(|| value.as_i64().map(|value| value as f64))
            .or_else(|| value.as_str().and_then(|value| yt_dlp_core::parse_duration(value)))
    })
}
