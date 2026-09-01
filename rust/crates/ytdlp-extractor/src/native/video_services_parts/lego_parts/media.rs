fn lego_quality_rank(value: Option<&str>) -> Option<i64> {
    match value {
        Some("Lowest") => Some(0),
        Some("Low") => Some(1),
        Some("Medium") => Some(2),
        Some("High") => Some(3),
        Some("Highest") => Some(4),
        _ => None,
    }
}

fn lego_quality_dimensions(value: Option<&str>) -> Option<(i64, i64, i64)> {
    match value {
        Some("Lowest") => Some((64, 180, 320)),
        Some("Low") => Some((64, 270, 480)),
        Some("Medium") => Some((96, 360, 640)),
        Some("High") => Some((128, 540, 960)),
        Some("Highest") => Some((128, 720, 1280)),
        _ => None,
    }
}

fn lego_formats(
    item: &serde_json::Value,
    video_id: &str,
) -> Result<Vec<serde_json::Value>, ExtractorError> {
    let mut formats = Vec::new();
    for source in item
        .get("VideoFormats")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(media_url) = json_string(source, "Url").filter(|value| !value.is_empty()) else {
            continue;
        };
        let format = json_string(source, "Format").unwrap_or("HTTP");
        if format.eq_ignore_ascii_case("F4M") {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!("TODO: LEGO video {video_id} requires Adobe HDS/F4M manifest parsing"),
            ));
        }
        if format.eq_ignore_ascii_case("M3U8") {
            formats.push(serde_json::json!({
                "url": media_url,
                "format_id": format,
                "ext": "mp4",
                "protocol": "m3u8_native",
            }));
            continue;
        }
        let quality = json_string(source, "Quality");
        let format_id = match quality {
            Some(quality) => format!("{format}-{quality}"),
            None => format.to_owned(),
        };
        let mut output = serde_json::json!({
            "url": media_url,
            "format_id": format_id,
        });
        if let Some(rank) = lego_quality_rank(quality) {
            output["quality"] = serde_json::json!(rank);
        }
        if let Some((abr, height, width)) = lego_quality_dimensions(quality) {
            output["abr"] = serde_json::json!(abr);
            output["height"] = serde_json::json!(height);
            output["width"] = serde_json::json!(width);
        }
        formats.push(output);
    }
    if formats.is_empty() {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("LEGO video {video_id} has no playable formats"),
        ));
    }
    Ok(formats)
}

fn lego_subtitles(video: &serde_json::Value, locale: &str) -> serde_json::Value {
    let zero_uuid = "00000000-0000-0000-0000-000000000000";
    let Some(_sub_file_id) = json_string(video, "SubFileId")
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case(zero_uuid))
    else {
        return serde_json::json!({});
    };
    let Some(netstorage_path) = json_string(video, "NetstoragePath") else {
        return serde_json::json!({});
    };
    let Some(invariant_id) = json_string(video, "InvariantId") else {
        return serde_json::json!({});
    };
    let Some(video_file_id) = json_string(video, "VideoFileId") else {
        return serde_json::json!({});
    };
    let Some(video_version) = json_string(video, "VideoVersion") else {
        return serde_json::json!({});
    };
    let language = locale.get(..2).unwrap_or(locale);
    serde_json::json!({
        language: [{
            "url": format!(
            "https://lc-mediaplayerns-live-s.legocdn.com/public/{netstorage_path}/{invariant_id}_{video_file_id}_{locale}_{video_version}_sub.srt",
            )
        }]
    })
}
