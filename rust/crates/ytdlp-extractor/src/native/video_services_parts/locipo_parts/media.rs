fn locipo_source_is_drm(source: &serde_json::Value) -> bool {
    source.get("key_systems").is_some_and(|value| match value {
        serde_json::Value::Null => false,
        serde_json::Value::Object(values) => !values.is_empty(),
        serde_json::Value::Array(values) => !values.is_empty(),
        _ => true,
    })
}

fn locipo_formats(
    response: &serde_json::Value,
    media_id: &str,
) -> Result<Vec<serde_json::Value>, ExtractorError> {
    let live_status = match json_string(response, "type") {
        Some("linear") | Some("live") => Some("is_live"),
        Some("clip") => Some("was_live"),
        Some("file") => Some("not_live"),
        _ => None,
    };
    let mut formats = Vec::new();
    let mut saw_drm = false;
    let mut saw_unsupported = false;
    let mut saw_ssai = false;
    for source in response
        .get("sources")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        if locipo_source_is_drm(source) {
            saw_drm = true;
            continue;
        }
        if live_status == Some("is_live") && source.get("ssai").is_some() {
            saw_ssai = true;
        }
        let Some(source_url) = json_string(source, "src")
            .filter(|value| !value.is_empty())
            .map(|value| proto_relative_url(value, "https:"))
        else {
            continue;
        };
        let source_type = json_string(source, "type").unwrap_or_default();
        let ext = yt_dlp_core::determine_ext(Some(&source_url), "unknown");
        let is_hls = ext.eq_ignore_ascii_case("m3u8")
            || source_type.to_ascii_lowercase().contains("mpegurl")
            || source_type.eq_ignore_ascii_case("m3u8");
        if !is_hls {
            saw_unsupported = true;
            continue;
        }
        let mut format = serde_json::json!({
            "url": source_url,
            "format_id": format!("hls-{}", formats.len()),
            "ext": "mp4",
            "protocol": "m3u8_native",
        });
        if live_status == Some("is_live") {
            format["live"] = serde_json::json!(true);
        }
        formats.push(format);
    }
    if formats.is_empty() {
        let detail = if saw_drm {
            "the only sources are DRM-protected"
        } else if saw_ssai {
            "the live source requires SSAI session negotiation"
        } else if saw_unsupported {
            "the sources are not HLS"
        } else {
            "the playback response has no usable sources"
        };
        return Err(ExtractorError::new(
            if saw_drm || saw_ssai || saw_unsupported {
                ExtractorErrorKind::Unsupported
            } else {
                ExtractorErrorKind::Extraction
            },
            format!("TODO: Locipo Streaks media {media_id} {detail}"),
        ));
    }
    if saw_ssai {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            format!(
                "TODO: Locipo live media {media_id} requires native SSAI session negotiation"
            ),
        ));
    }
    Ok(formats)
}

fn locipo_text_list(value: Option<&serde_json::Value>) -> Option<Vec<String>> {
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

fn locipo_thumbnail(response: &serde_json::Value) -> Option<String> {
    ["thumbnail", "poster"].iter().find_map(|key| {
        response
            .get(*key)
            .and_then(|value| json_string(value, "src"))
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    })
}

fn locipo_streaks_info(
    response: &serde_json::Value,
    media_id: &str,
) -> Result<InfoDict, ExtractorError> {
    let formats = locipo_formats(response, media_id)?;
    let streaks_id = json_value_string(response.get("id")).unwrap_or_else(|| media_id.to_owned());
    let mut info = InfoDict::new();
    info.insert("id", serde_json::json!(streaks_id));
    info.insert("display_id", serde_json::json!(media_id));
    info.insert("formats", serde_json::Value::Array(formats.clone()));
    info.insert_if_some(
        "url",
        formats
            .first()
            .and_then(|format| json_string(format, "url")),
    );
    info.insert("subtitles", locipo_subtitles(response));
    info.insert_if_some(
        "live_status",
        match json_string(response, "type") {
            Some("clip") => Some("was_live"),
            Some("file") => Some("not_live"),
            Some("linear") | Some("live") => Some("is_live"),
            _ => None,
        },
    );
    info.insert("uploader_id", serde_json::json!("locipo-prod"));
    info.insert_if_some("title", json_string(response, "name").map(html_text_fragment));
    info.insert_if_some(
        "description",
        json_string(response, "description").map(html_text_fragment),
    );
    info.insert_if_some("duration", json_f64(response, "duration"));
    info.insert_if_some(
        "modified_timestamp",
        json_string(response, "updated_at").and_then(|value| parse_timestamp(value.to_owned())),
    );
    info.insert_if_some(
        "timestamp",
        json_string(response, "created_at").and_then(|value| parse_timestamp(value.to_owned())),
    );
    info.insert_if_some("tags", locipo_text_list(response.get("tags")));
    info.insert_if_some("thumbnail", locipo_thumbnail(response));
    Ok(info)
}

fn locipo_subtitles(response: &serde_json::Value) -> serde_json::Value {
    let mut subtitles = serde_json::Map::new();
    for track in response
        .get("tracks")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        if !matches!(
            json_string(track, "kind"),
            Some("captions") | Some("subtitles")
        ) {
            continue;
        }
        let Some(url) = json_string(track, "src").filter(|value| !value.is_empty()) else {
            continue;
        };
        let language = json_string(track, "srclang")
            .map(|value| value.to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "ja".to_owned());
        subtitles
            .entry(language)
            .or_insert_with(|| serde_json::json!([]))
            .as_array_mut()
            .map(|values| values.push(serde_json::json!({"url": url})));
    }
    serde_json::Value::Object(subtitles)
}
