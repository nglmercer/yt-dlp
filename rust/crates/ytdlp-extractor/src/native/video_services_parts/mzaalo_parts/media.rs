fn mzaalo_http_url(value: Option<&serde_json::Value>) -> Option<String> {
    let value = value?.as_str()?.trim();
    let parsed = url::Url::parse(value).ok()?;
    matches!(parsed.scheme(), "http" | "https").then(|| value.to_owned())
}

fn mzaalo_hls_format(media_url: String, language: Option<&str>) -> serde_json::Value {
    let mut format = serde_json::json!({
        "url": media_url,
        "format_id": "hls",
        "ext": "mp4",
        "protocol": "m3u8_native",
    });
    if let Some(language) = language {
        format["language"] = serde_json::json!(language);
    }
    format
}

fn mzaalo_subtitles(data: &serde_json::Value) -> serde_json::Value {
    let mut subtitles = serde_json::Map::new();
    let Some(entries) = data.get("subtitles").and_then(serde_json::Value::as_object) else {
        return serde_json::Value::Object(subtitles);
    };
    for (language, value) in entries {
        let Some(url) = mzaalo_http_url(Some(value)) else {
            continue;
        };
        subtitles.insert(
            language.clone(),
            serde_json::json!([{"url": url, "ext": "vtt"}]),
        );
    }
    serde_json::Value::Object(subtitles)
}

fn mzaalo_thumbnails(data: &serde_json::Value) -> Option<Vec<serde_json::Value>> {
    let thumbnails = data
        .get("images")
        .and_then(serde_json::Value::as_array)?
        .iter()
        .filter_map(|image| mzaalo_http_url(image.get("url")))
        .map(|url| serde_json::json!({"url": url}))
        .collect::<Vec<_>>();
    (!thumbnails.is_empty()).then_some(thumbnails)
}

fn mzaalo_duration(value: Option<&serde_json::Value>) -> Option<f64> {
    value.and_then(|value| {
        value
            .as_f64()
            .or_else(|| value.as_i64().map(|value| value as f64))
            .or_else(|| value.as_u64().map(|value| value as f64))
            .or_else(|| value.as_str().and_then(yt_dlp_core::parse_duration))
    })
}

fn mzaalo_age_limit(value: Option<&serde_json::Value>) -> Option<i64> {
    value.and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
            .or_else(|| {
                value.as_str().and_then(|value| {
                    value
                        .chars()
                        .take_while(char::is_ascii_digit)
                        .collect::<String>()
                        .parse()
                        .ok()
                })
            })
    })
}
