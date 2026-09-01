fn manyvids_url(value: Option<&serde_json::Value>) -> Option<String> {
    let value = value.and_then(serde_json::Value::as_str)?.trim();
    if value.is_empty() {
        return None;
    }
    let parsed = url::Url::parse(value).ok()?;
    matches!(parsed.scheme(), "http" | "https").then(|| value.to_owned())
}

fn manyvids_path_string(
    video_data: &serde_json::Value,
    path: &[&str],
) -> Option<String> {
    let mut value = video_data;
    for key in path {
        value = value.get(*key)?;
    }
    manyvids_url(Some(value))
}

fn manyvids_count(value: Option<&serde_json::Value>) -> Option<i64> {
    let value = value?;
    if let Some(integer) = value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
    {
        return Some(integer);
    }
    let raw = value.as_str()?.trim().replace(',', "");
    if raw.is_empty() {
        return None;
    }
    let (number, multiplier) = match raw.chars().last()? {
        'k' | 'K' => (&raw[..raw.len() - 1], 1_000.0),
        'm' | 'M' => (&raw[..raw.len() - 1], 1_000_000.0),
        'b' | 'B' => (&raw[..raw.len() - 1], 1_000_000_000.0),
        _ => (raw.as_str(), 1.0),
    };
    number
        .trim()
        .parse::<f64>()
        .ok()
        .map(|value| (value * multiplier) as i64)
}

fn manyvids_duration(value: Option<&serde_json::Value>) -> Option<f64> {
    let value = value?;
    value.as_f64().or_else(|| {
        value
            .as_str()
            .and_then(|value| yt_dlp_core::parse_duration(value))
    })
}

fn manyvids_format_height(media_url: &str) -> Option<i64> {
    Regex::new(r"_(\d{2,3}[02468])_")
        .ok()
        .and_then(|matcher| matcher.captures(media_url).ok().flatten())
        .and_then(|captures| captures.get(1))
        .and_then(|value| value.as_str().parse::<i64>().ok())
}

fn manyvids_formats(video_data: &serde_json::Value) -> (Vec<serde_json::Value>, bool) {
    let candidates = [
        ("preview", ["teaser", "filepath"].as_slice()),
        ("transcoded", ["transcodedFilepath"].as_slice()),
        ("filepath", ["filepath"].as_slice()),
    ];
    let mut formats = Vec::new();
    let mut preview_only = true;
    for (format_id, path) in candidates {
        let Some(media_url) = manyvids_path_string(video_data, path) else {
            continue;
        };
        let extension = yt_dlp_core::determine_ext(Some(&media_url), "mp4");
        if extension.eq_ignore_ascii_case("m3u8") {
            formats.push(serde_json::json!({
                "url": media_url,
                "format_id": format_id,
                "ext": "mp4",
                "protocol": "m3u8_native",
            }));
        } else {
            let mut format = serde_json::json!({
                "url": media_url,
                "format_id": format_id,
                "ext": extension,
            });
            if format_id == "preview" {
                format["preference"] = serde_json::json!(-10);
            }
            if format_id == "filepath" {
                format["quality"] = serde_json::json!(10);
            }
            if let Some(height) = manyvids_format_height(
                format
                    .get("url")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default(),
            ) {
                format["height"] = serde_json::json!(height);
            }
            formats.push(format);
        }
        if format_id != "preview" {
            preview_only = false;
        }
    }
    (formats, preview_only)
}

fn manyvids_thumbnail(metadata: &serde_json::Value) -> Option<String> {
    manyvids_url(metadata.get("screenshot").and_then(|value| value.get("thumbnail")))
        .or_else(|| manyvids_url(metadata.get("thumbnail")))
}

fn manyvids_tags(metadata: &serde_json::Value) -> Option<serde_json::Value> {
    let tags = metadata
        .get("tagList")
        .and_then(serde_json::Value::as_array)?
        .iter()
        .filter_map(|tag| json_string(tag, "label"))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    (!tags.is_empty()).then(|| serde_json::json!(tags))
}
