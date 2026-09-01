fn loom_build_metadata(
    metadata: &serde_json::Value,
    video_id: &str,
) -> InfoDict {
    let mut info = InfoDict::new();
    info.insert("id", serde_json::json!(video_id));
    info.insert_if_some("title", json_string(metadata, "name"));
    info.insert_if_some("description", json_string(metadata, "description"));
    info.insert_if_some(
        "uploader",
        metadata
            .get("owner")
            .and_then(|owner| json_string(owner, "display_name")),
    );
    info.insert_if_some(
        "timestamp",
        json_string(metadata, "createdAt").and_then(|value| parse_timestamp(value.to_owned())),
    );
    if let Some(properties) = metadata.get("video_properties") {
        info.insert_if_some("duration", json_i64(properties, "duration"));
        info.insert_if_some("width", json_i64(properties, "width"));
        info.insert_if_some("height", json_i64(properties, "height"));
        if json_bool(properties, "microphone_enabled") == Some(false) {
            info.insert("acodec", serde_json::json!("none"));
        }
    }
    info
}

fn loom_queryless_url(value: &str) -> String {
    url::Url::parse(value).map_or_else(
        |_| value.to_owned(),
        |mut url| {
            url.set_query(None);
            url.to_string()
        },
    )
}

fn loom_format_query(value: &str) -> String {
    url::Url::parse(value)
        .ok()
        .and_then(|url| url.query().map(str::to_owned))
        .unwrap_or_default()
}

fn loom_add_format(
    formats: &mut Vec<serde_json::Value>,
    raw_url: &str,
    format_id: &str,
    quality: i64,
    metadata: &InfoDict,
) {
    let Some(raw_url) = (!raw_url.is_empty()).then_some(raw_url) else {
        return;
    };
    let query = loom_format_query(raw_url);
    let mut media_url = raw_url.to_owned();
    let ext = yt_dlp_core::determine_ext(Some(&media_url), "mp4").to_ascii_lowercase();
    let is_hls = ext == "m3u8" || media_url.contains(".m3u8?");
    let is_dash = ext == "mpd" || media_url.contains(".mpd?");
    if is_hls {
        media_url = media_url.replace("-split.m3u8", ".m3u8");
    }
    let mut format = serde_json::json!({
        "url": media_url,
        "format_id": if is_hls {
            format!("hls-{format_id}")
        } else if is_dash {
            format!("dash-{format_id}")
        } else {
            format!("http-{format_id}")
        },
        "quality": quality,
        "ext": if is_hls { "mp4" } else { ext.as_str() },
    });
    if is_hls {
        format["protocol"] = serde_json::json!("m3u8_native");
    } else if is_dash {
        format["protocol"] = serde_json::json!("http_dash_segments");
    }
    if !query.is_empty() && (is_hls || is_dash) {
        format["extra_param_to_segment_url"] = serde_json::json!(query);
    }
    for key in ["width", "height", "acodec"] {
        if let Some(value) = metadata.get(key) {
            format[key] = value.clone();
        }
    }
    formats.push(format);
}

fn loom_formats(
    context: &ExtractionContext,
    video_id: &str,
    metadata: &InfoDict,
    source_data: &serde_json::Value,
) -> Result<Vec<serde_json::Value>, ExtractorError> {
    let raw_url = loom_url_api(context, video_id, "raw-url")?;
    let transcoded_url = loom_url_api(context, video_id, "transcoded-url")?;
    let cdn_url = source_data
        .get("data")
        .and_then(|data| data.get("getVideo"))
        .and_then(|video| video.get("nullableRawCdnUrl"))
        .and_then(|source| json_string(source, "url"))
        .map(str::to_owned);
    let mut formats = Vec::new();
    let mut seen = Vec::new();
    for (value, format_id, quality) in [
        (raw_url, "raw", 1_i64),
        (transcoded_url, "transcoded", -1_i64),
        (cdn_url, "cdn", 0_i64),
    ] {
        let Some(value) = value else {
            continue;
        };
        if !seen.insert_unique(loom_queryless_url(&value)) {
            continue;
        }
        loom_add_format(&mut formats, &value, format_id, quality, metadata);
    }
    if formats.is_empty() {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Loom video {video_id} has no playable source URL"),
        ));
    }
    Ok(formats)
}

fn loom_subtitles(data: Option<&serde_json::Value>) -> serde_json::Value {
    let Some(data) = data
        .and_then(|data| data.get("data"))
        .and_then(|data| data.get("fetchVideoTranscript"))
    else {
        return serde_json::json!({});
    };
    let url = json_string(data, "source_url")
        .or_else(|| json_string(data, "captions_source_url"))
        .filter(|value| value.starts_with("http://") || value.starts_with("https://"));
    url.map_or_else(
        || serde_json::json!({}),
        |url| serde_json::json!({"en": [{"url": url}]}),
    )
}

fn loom_chapters(
    data: Option<&serde_json::Value>,
    duration: Option<i64>,
) -> Option<serde_json::Value> {
    let content = data?
        .get("data")?
        .get("fetchVideoChapters")?
        .get("content")?
        .as_str()?;
    let matcher = Regex::new(r#"(?m)^\s*(\d{1,2}):(\d{2})(?::(\d{2}))?\s+(.+?)\s*$"#).ok()?;
    let mut chapters = Vec::new();
    for captures in matcher.captures_iter(content).flatten() {
        let first = captures.get(1)?.as_str().parse::<i64>().ok()?;
        let second = captures.get(2)?.as_str().parse::<i64>().ok()?;
        let (start_time, title) = if let Some(hours) = captures.get(3) {
            let hours = hours.as_str().parse::<i64>().ok()?;
            (
                hours * 3600 + first * 60 + second,
                captures.get(4)?.as_str().trim(),
            )
        } else {
            (first * 60 + second, captures.get(4)?.as_str().trim())
        };
        chapters.push(serde_json::json!({
            "start_time": start_time,
            "title": title,
        }));
    }
    if chapters.is_empty() {
        return None;
    }
    for index in 0..chapters.len().saturating_sub(1) {
        let end_time = chapters[index + 1]
            .get("start_time")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        chapters[index]["end_time"] = end_time;
    }
    if let Some(duration) = duration {
        if let Some(last) = chapters.last_mut() {
            last["end_time"] = serde_json::json!(duration);
        }
    }
    Some(serde_json::Value::Array(chapters))
}
