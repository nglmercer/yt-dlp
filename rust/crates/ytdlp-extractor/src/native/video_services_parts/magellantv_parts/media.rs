fn magellantv_hls_urls(data: &serde_json::Value) -> Vec<String> {
    let Some(manifests) = data.get("manifests").and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    let mut urls = Vec::new();
    for manifest in manifests {
        let raw_url = manifest
            .get("hls")
            .and_then(|hls| json_string(hls, "jwp_video_url"))
            .or_else(|| json_string(manifest, "jwp_video_url"));
        let Some(media_url) = raw_url
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
        else {
            continue;
        };
        if !urls.iter().any(|value| value == media_url) {
            urls.push(media_url.to_owned());
        }
    }
    urls
}

fn magellantv_formats(
    data: &serde_json::Value,
    video_id: &str,
) -> Result<Vec<serde_json::Value>, ExtractorError> {
    let formats = magellantv_hls_urls(data)
        .into_iter()
        .enumerate()
        .map(|(index, media_url)| {
            serde_json::json!({
                "url": media_url,
                "format_id": if index == 0 { "hls".to_owned() } else { format!("hls-{index}") },
                "ext": "mp4",
                "protocol": "m3u8_native",
            })
        })
        .collect::<Vec<_>>();
    if formats.is_empty() {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            format!(
                "TODO: MagellanTV video {video_id} has no native HLS manifest (possibly geo-restricted)"
            ),
        ));
    }
    Ok(formats)
}

fn magellantv_duration(data: &serde_json::Value) -> Option<f64> {
    json_f64(data, "duration").or_else(|| {
        let value = json_string(data, "duration")?;
        let mut total = 0.0;
        let mut components = value.split(':').collect::<Vec<_>>();
        if components.len() > 3 {
            return None;
        }
        let seconds = components.pop()?.parse::<f64>().ok()?;
        total += seconds;
        for (index, component) in components.iter().rev().enumerate() {
            total += component.parse::<f64>().ok()? * 60_f64.powi((index + 1) as i32);
        }
        Some(total)
    })
}

fn magellantv_age_limit(data: &serde_json::Value) -> Option<i64> {
    json_i64(data, "ratingCategory").or_else(|| {
        json_string(data, "ratingCategory")?
            .chars()
            .filter(|character| character.is_ascii_digit())
            .collect::<String>()
            .parse()
            .ok()
    })
}
