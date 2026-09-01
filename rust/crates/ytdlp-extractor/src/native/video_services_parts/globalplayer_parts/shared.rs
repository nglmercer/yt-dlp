fn globalplayer_page_props(
    url: &str,
    video_id: &str,
    context: &ExtractionContext,
) -> Result<serde_json::Value, ExtractorError> {
    let response = context.get(url)?;
    let webpage = String::from_utf8_lossy(response.body());
    let next_data = html_script_json(&webpage, "__NEXT_DATA__")?;
    next_data
        .get("props")
        .and_then(|props| props.get("pageProps"))
        .cloned()
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Global Player page {video_id} has no Next.js page props"),
            )
        })
}

fn globalplayer_string(value: &serde_json::Value, key: &str) -> Option<String> {
    json_string(value, key)
        .map(str::to_owned)
        .filter(|value| !value.is_empty())
}

fn globalplayer_value_string(value: Option<&serde_json::Value>) -> Option<String> {
    value.map(|value| {
        value
            .as_str()
            .map(str::to_owned)
            .or_else(|| value.as_i64().map(|value| value.to_string()))
            .or_else(|| value.as_u64().map(|value| value.to_string()))
            .unwrap_or_else(|| value.to_string())
    })
}

fn globalplayer_url(value: Option<&serde_json::Value>) -> Option<String> {
    let value = globalplayer_value_string(value)?;
    (value.starts_with("http://") || value.starts_with("https://")).then_some(value)
}

fn globalplayer_image(value: Option<&serde_json::Value>) -> Option<String> {
    value
        .and_then(|value| value.as_str().map(str::to_owned).or_else(|| globalplayer_url(value.get("url"))))
        .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
}

fn globalplayer_playback_url(data: &serde_json::Value) -> Option<String> {
    let playback = data.get("playback")?;
    let values = match playback {
        serde_json::Value::Array(values) => values.iter().collect::<Vec<_>>(),
        serde_json::Value::Object(values) => values.values().collect::<Vec<_>>(),
        _ => Vec::new(),
    };
    values.into_iter().find_map(|value| {
        let can_use = json_string(value, "canUse") == Some("true")
            || value.get("canUse").and_then(serde_json::Value::as_bool) == Some(true);
        can_use.then(|| globalplayer_url(value.get("url"))).flatten()
    })
}

fn globalplayer_playable(
    context: &ExtractionContext,
    id: &str,
    video_id: &str,
) -> Result<String, ExtractorError> {
    let data = context.get_json(&format!(
        "https://bff-web-guacamole.musicradio.com/playables/{id}"
    ))?;
    globalplayer_playback_url(&data).ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Global Player playable {id} has no usable playback URL for {video_id}"),
        )
    })
}

fn globalplayer_format(url: &str, ext: &str, audio_only: bool) -> serde_json::Value {
    let mut format = serde_json::json!({
        "url": url,
        "format_id": "http",
        "protocol": "http",
        "ext": ext,
    });
    if audio_only {
        format["vcodec"] = serde_json::json!("none");
    }
    format
}

fn globalplayer_insert_meta(info: &mut InfoDict, meta: &serde_json::Value) {
    info.insert_if_some("thumbnail", globalplayer_image(meta.get("image")));
    info.insert_if_some("description", globalplayer_string(meta, "description"));
    info.insert_if_some("title", globalplayer_string(meta, "title"));
}

fn globalplayer_audio_info(
    id: &str,
    audio_url: &str,
    meta: &serde_json::Value,
) -> InfoDict {
    let ext = yt_dlp_core::determine_ext(Some(audio_url), "mp3");
    let mut info = InfoDict::new();
    info.insert("id", serde_json::json!(id));
    info.insert("url", serde_json::json!(audio_url));
    info.insert("ext", serde_json::json!(ext));
    info.insert("vcodec", serde_json::json!("none"));
    info.insert(
        "formats",
        serde_json::Value::Array(vec![globalplayer_format(audio_url, &ext, true)]),
    );
    globalplayer_insert_meta(&mut info, meta);
    info
}
