fn mdr_http_url(value: &str) -> Option<String> {
    let value = value.trim();
    let parsed = url::Url::parse(value).ok()?;
    matches!(parsed.scheme(), "http" | "https").then(|| value.to_owned())
}

fn mdr_integer(value: Option<&String>) -> Option<i64> {
    value?.trim().parse().ok()
}

fn mdr_scaled_integer(value: Option<&String>) -> Option<i64> {
    mdr_integer(value).map(|value| value / 1_000)
}

fn mdr_format_id(media_type: &str, bitrate: Option<i64>) -> String {
    let media_type = media_type.trim();
    match (media_type.is_empty(), bitrate) {
        (true, Some(bitrate)) => bitrate.to_string(),
        (false, Some(bitrate)) => format!("{media_type}-{bitrate}"),
        (false, None) => media_type.to_owned(),
        (true, None) => "http".to_owned(),
    }
}

fn mdr_formats(
    document: &MdrDocument,
    video_id: &str,
) -> Result<Vec<serde_json::Value>, ExtractorError> {
    let mut formats = Vec::new();
    let mut processed_urls = Vec::new();
    for asset in &document.assets {
        for raw_url in [
            asset.download_url.as_ref(),
            asset.progressive_download_url.as_ref(),
            asset.dynamic_streaming_url.as_ref(),
            asset.adaptive_streaming_url.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            let Some(media_url) = mdr_http_url(raw_url) else {
                continue;
            };
            if processed_urls.iter().any(|value| value == &media_url) {
                continue;
            }
            processed_urls.push(media_url.clone());
            let extension = yt_dlp_core::determine_ext(Some(&media_url), "unknown");
            if extension == "f4m" {
                return Err(ExtractorError::new(
                    ExtractorErrorKind::Unsupported,
                    format!(
                        "TODO: MDR native extractor does not implement legacy F4M stream {media_url}"
                    ),
                ));
            }
            if extension == "m3u8" {
                formats.push(serde_json::json!({
                    "url": media_url,
                    "format_id": "HLS",
                    "ext": "mp4",
                    "protocol": "m3u8_native",
                }));
                continue;
            }
            let media_type = asset.media_type.as_deref().unwrap_or("MP4");
            let vbr = mdr_scaled_integer(asset.bitrate_video.as_ref());
            let abr = mdr_scaled_integer(asset.bitrate_audio.as_ref());
            let bitrate = vbr.or(abr);
            let mut format = serde_json::json!({
                "url": media_url,
                "format_id": mdr_format_id(media_type, bitrate),
                "ext": extension,
                "protocol": "http",
            });
            if let Some(value) = mdr_integer(asset.file_size.as_ref()) {
                format["filesize"] = serde_json::json!(value);
            }
            if let Some(value) = abr {
                format["abr"] = serde_json::json!(value);
            }
            if let Some(value) = vbr {
                format["vbr"] = serde_json::json!(value);
                if let Some(width) = mdr_integer(asset.frame_width.as_ref()) {
                    format["width"] = serde_json::json!(width);
                }
                if let Some(height) = mdr_integer(asset.frame_height.as_ref()) {
                    format["height"] = serde_json::json!(height);
                }
            }
            if document.media_type.as_deref() == Some("audio") {
                format["vcodec"] = serde_json::json!("none");
            }
            formats.push(format);
        }
    }
    if formats.is_empty() {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            format!("TODO: MDR video {video_id} has no native playable formats"),
        ));
    }
    Ok(formats)
}
