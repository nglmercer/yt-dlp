fn lrt_media_url(value: &str, base_url: &str) -> String {
    let value = lrt_unescape(value.trim());
    resolve_url(base_url, &proto_relative_url(&value, "https:"))
}

fn lrt_formats_from_urls(
    urls: impl IntoIterator<Item = String>,
    video_id: &str,
    fallback_ext: &str,
    live: bool,
) -> Result<Vec<serde_json::Value>, ExtractorError> {
    let mut formats = Vec::new();
    let mut seen_urls = Vec::new();
    for media_url in urls {
        if media_url.is_empty() || !seen_urls.insert_unique(media_url.clone()) {
            continue;
        }
        let lower_url = media_url.to_ascii_lowercase();
        if lower_url.starts_with("rtmp://") || lower_url.starts_with("rtmps://") {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!("TODO: LRT video {video_id} requires native RTMP support"),
            ));
        }
        let extension = yt_dlp_core::determine_ext(Some(&media_url), fallback_ext);
        let is_hls = extension.eq_ignore_ascii_case("m3u8")
            || lower_url.contains(".m3u8?")
            || lower_url.ends_with(".m3u8");
        let is_dash = extension.eq_ignore_ascii_case("mpd")
            || lower_url.contains(".mpd?")
            || lower_url.ends_with(".mpd");
        let format_id = if is_hls {
            format!("hls-{}", formats.len())
        } else if is_dash {
            format!("dash-{}", formats.len())
        } else {
            format!("http-{}", formats.len())
        };
        let mut format = serde_json::json!({
            "format_id": format_id,
            "url": media_url,
            "ext": if is_hls || is_dash { fallback_ext } else { extension.as_str() },
        });
        if is_hls {
            format["protocol"] = serde_json::json!("m3u8_native");
        } else if is_dash {
            format["protocol"] = serde_json::json!("http_dash_segments");
        } else {
            format["protocol"] = serde_json::json!("http");
        }
        if live {
            format["live"] = serde_json::json!(true);
        }
        formats.push(format);
    }
    if formats.is_empty() {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("LRT video {video_id} has no playable media URLs"),
        ));
    }
    Ok(formats)
}

fn lrt_collect_source_urls(value: &serde_json::Value, urls: &mut Vec<String>) {
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                lrt_collect_source_urls(value, urls);
            }
        }
        serde_json::Value::Object(values) => {
            for key in ["file", "src", "url"] {
                if let Some(value) = values
                    .get(key)
                    .and_then(serde_json::Value::as_str)
                    .filter(|value| !value.trim().is_empty())
                {
                    urls.push(value.to_owned());
                }
            }
            for key in ["sources", "playlist", "playlist_item"] {
                if let Some(value) = values.get(key) {
                    lrt_collect_source_urls(value, urls);
                }
            }
        }
        _ => {}
    }
}

fn lrt_item_formats(
    item: &serde_json::Value,
    video_id: &str,
    fallback_ext: &str,
    base_url: &str,
) -> Result<Vec<serde_json::Value>, ExtractorError> {
    let mut raw_urls = Vec::new();
    lrt_collect_source_urls(item, &mut raw_urls);
    lrt_formats_from_urls(
        raw_urls
            .into_iter()
            .map(|value| lrt_media_url(&value, base_url)),
        video_id,
        fallback_ext,
        false,
    )
}

fn lrt_subtitles(item: &serde_json::Value, base_url: &str) -> serde_json::Value {
    let mut subtitles = serde_json::Map::new();
    let Some(tracks) = item.get("tracks") else {
        return serde_json::Value::Object(subtitles);
    };
    let values = match tracks {
        serde_json::Value::Array(values) => values.iter().collect::<Vec<_>>(),
        serde_json::Value::Object(values) => values.values().collect::<Vec<_>>(),
        _ => Vec::new(),
    };
    for track in values {
        let Some(url) = ["file", "src", "url"]
            .iter()
            .find_map(|key| json_string(track, key))
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let language = json_string(track, "language")
            .or_else(|| json_string(track, "lang"))
            .or_else(|| json_string(track, "label"))
            .unwrap_or("und")
            .to_owned();
        let entry = subtitles
            .entry(language)
            .or_insert_with(|| serde_json::json!([]));
        if let Some(values) = entry.as_array_mut() {
            values.push(serde_json::json!({
                "url": lrt_media_url(url, base_url),
            }));
        }
    }
    serde_json::Value::Object(subtitles)
}

fn lrt_text_list(value: Option<&serde_json::Value>) -> Option<Vec<String>> {
    let values = value?.as_array()?;
    let values = values
        .iter()
        .filter_map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .or_else(|| json_string(value, "name").map(str::to_owned))
        })
        .map(|value| html_text_fragment(&value))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    (!values.is_empty()).then_some(values)
}

fn lrt_timestamp(value: &str) -> Option<i64> {
    if let Some(timestamp) = parse_timestamp(value.to_owned()) {
        return Some(timestamp);
    }
    let matcher = Regex::new(
        r#"(?x)^(?P<day>\d{2})[./-](?P<month>\d{2})[./-](?P<year>\d{4})(?:[ T]+(?P<hour>\d{2}):(?P<minute>\d{2})(?::(?P<second>\d{2}))?)?"#,
    )
    .ok()?;
    let captures = matcher.captures(value).ok().flatten()?;
    let day = captures.name("day")?.as_str();
    let month = captures.name("month")?.as_str();
    let year = captures.name("year")?.as_str();
    let hour = captures.name("hour").map_or("00", |value| value.as_str());
    let minute = captures
        .name("minute")
        .map_or("00", |value| value.as_str());
    let second = captures
        .name("second")
        .map_or("00", |value| value.as_str());
    yt_dlp_core::parse_iso8601(&format!("{year}-{month}-{day}T{hour}:{minute}:{second}Z"))
}

fn lrt_tag_values(value: Option<&serde_json::Value>) -> Option<Vec<String>> {
    lrt_text_list(value)
}

fn lrt_info_from_playlist_item(
    item: &serde_json::Value,
    video_id: &str,
    fallback_ext: &str,
    base_url: &str,
) -> Result<InfoDict, ExtractorError> {
    let formats = lrt_item_formats(item, video_id, fallback_ext, base_url)?;
    let mut info = InfoDict::new();
    info.insert("id", serde_json::json!(video_id));
    info.insert_if_some("title", json_string(item, "title").map(html_text_fragment));
    info.insert_if_some(
        "description",
        json_string(item, "description").map(html_text_fragment),
    );
    info.insert_if_some(
        "thumbnail",
        json_string(item, "image").map(|value| lrt_media_url(value, base_url)),
    );
    info.insert_if_some("duration", json_f64(item, "duration"));
    info.insert("formats", serde_json::Value::Array(formats));
    info.insert("subtitles", lrt_subtitles(item, base_url));
    Ok(info)
}
