fn katsomo_thumbnail_list(value: Option<&serde_json::Value>) -> Option<serde_json::Value> {
    let thumbnails = value?
        .as_object()?
        .iter()
        .filter_map(|(thumbnail_id, thumbnail)| {
            let url = json_string(thumbnail, "url")
                .filter(|url| url.starts_with("http://") || url.starts_with("https://"))?;
            Some(serde_json::json!({"id": thumbnail_id, "url": url}))
        })
        .collect::<Vec<_>>();
    (!thumbnails.is_empty()).then_some(serde_json::Value::Array(thumbnails))
}

fn katsomo_playback_items(playback: &serde_json::Value) -> Vec<&serde_json::Value> {
    let Some(item) = playback
        .get("items")
        .and_then(|items| items.get("item"))
    else {
        return Vec::new();
    };
    match item {
        serde_json::Value::Array(items) => items.iter().collect(),
        serde_json::Value::Object(_) => vec![item],
        _ => Vec::new(),
    }
}

fn katsomo_format(
    protocol: &str,
    item: &serde_json::Value,
    drm_protected: bool,
    video_id: &str,
) -> Result<Option<serde_json::Value>, ExtractorError> {
    let Some(media_url) = json_string(item, "url")
        .filter(|url| url.starts_with("http://") || url.starts_with("https://"))
    else {
        return Ok(None);
    };
    let extension = yt_dlp_core::determine_ext(Some(media_url), "mp4").to_ascii_lowercase();
    if extension == "f4m" {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            format!(
                "TODO: Katsomo asset {video_id} requires Adobe HDS/F4M manifest parsing"
            ),
        ));
    }
    if extension == "ism" || media_url.ends_with(".ism/Manifest") {
        return Ok(None);
    }
    if extension == "m3u8" && drm_protected {
        return Ok(None);
    }
    let format_id = format!(
        "{}-{}",
        protocol.to_ascii_lowercase(),
        json_value_string(item.get("mediaFormat")).unwrap_or_else(|| "unknown".to_owned())
    );
    let is_hls = extension == "m3u8";
    let is_dash = extension == "mpd";
    let mut format = serde_json::json!({
        "url": media_url,
        "format_id": format_id,
        "protocol": if is_hls {
            "m3u8_native"
        } else if is_dash {
            "http_dash_segments"
        } else {
            "http"
        },
        "ext": if is_hls || is_dash { "mp4" } else { extension.as_str() },
    });
    if !is_hls && !is_dash {
        if let Some(bitrate) = json_i64(item, "bitrate") {
            format["tbr"] = serde_json::json!(bitrate);
        }
        if let Some(file_size) = json_i64(item, "fileSize") {
            format["filesize"] = serde_json::json!(file_size);
        }
    }
    Ok(Some(format))
}

fn katsomo_formats(
    video_id: &str,
    is_live: bool,
    context: &ExtractionContext,
) -> Result<Vec<serde_json::Value>, ExtractorError> {
    let mut formats = Vec::new();
    let mut seen_urls = Vec::new();
    let mut drm_protected = false;
    for protocol in KATSOMO_PROTOCOLS {
        let playback = katsomo_playback(context, video_id, protocol)?;
        drm_protected |= json_bool(&playback, "drmProtected").unwrap_or(false);
        for item in katsomo_playback_items(&playback) {
            let Some(media_url) = json_string(item, "url") else {
                continue;
            };
            if seen_urls.iter().any(|url| url == media_url) {
                continue;
            }
            let Some(format) = katsomo_format(protocol, item, drm_protected, video_id)? else {
                continue;
            };
            seen_urls.push(media_url.to_owned());
            formats.push(format);
        }
    }
    if formats.is_empty() && drm_protected {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            format!(
                "TODO: Katsomo asset {video_id} has only DRM-protected media; native DRM extraction is not implemented"
            ),
        ));
    }
    if formats.is_empty() {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Katsomo asset {video_id} has no playable formats"),
        ));
    }
    if is_live {
        for format in &mut formats {
            format["is_live"] = serde_json::json!(true);
        }
    }
    Ok(formats)
}
