fn mave_value_string(value: Option<&serde_json::Value>) -> Option<String> {
    let value = value?;
    match value {
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn mave_integer(value: Option<&serde_json::Value>) -> Option<i64> {
    let value = value?;
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
        .or_else(|| value.as_f64().map(|value| value as i64))
        .or_else(|| value.as_str().and_then(|value| value.trim().parse().ok()))
}

fn mave_url(value: Option<&serde_json::Value>) -> Option<String> {
    let value = value.and_then(serde_json::Value::as_str)?.trim();
    (!value.is_empty()).then(|| resolve_url(MAVE_STORAGE_BASE_URL, value))
}

fn mave_description(value: Option<&serde_json::Value>) -> Option<String> {
    let value = value.and_then(serde_json::Value::as_str)?;
    let value = html_text_fragment(value);
    (!value.is_empty()).then_some(value)
}

fn mave_timestamp(value: Option<&serde_json::Value>) -> Option<i64> {
    let value = value.and_then(serde_json::Value::as_str)?.trim();
    (!value.is_empty())
        .then(|| parse_timestamp(value.to_owned()))
        .flatten()
}

fn mave_reaction_count(
    episode: &serde_json::Value,
    reaction_type: &str,
) -> Option<i64> {
    episode
        .get("reactions")
        .and_then(serde_json::Value::as_array)
        .and_then(|reactions| {
            reactions.iter().find_map(|reaction| {
                (json_string(reaction, "type") == Some(reaction_type))
                    .then(|| mave_integer(reaction.get("count")))
                    .flatten()
            })
        })
}

fn mave_episode_code(episode: &serde_json::Value) -> Option<String> {
    mave_integer(episode.get("code")).map(|value| value.to_string())
}

fn mave_episode_entry(
    channel_id: &str,
    channel_meta: &serde_json::Value,
    episode: &serde_json::Value,
) -> Result<InfoDict, ExtractorError> {
    let episode_code = mave_episode_code(episode).ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Mave channel {channel_id} has an episode without a code"),
        )
    })?;
    let audio_url = mave_url(episode.get("audio")).ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Mave episode {channel_id}-{episode_code} has no audio URL"),
        )
    })?;
    let webpage_url = format!("https://{channel_id}.mave.digital/ep-{episode_code}");
    let format_ext = yt_dlp_core::determine_ext(Some(&audio_url), "mp3");
    let mut info = InfoDict::new();
    info.insert("display_id", serde_json::json!(format!("{channel_id}-{episode_code}")));
    info.insert("extractor_key", serde_json::json!("Mave"));
    info.insert("extractor", serde_json::json!("mave"));
    info.insert("webpage_url", serde_json::json!(webpage_url));
    info.insert("channel_id", serde_json::json!(channel_id));
    info.insert(
        "channel_url",
        serde_json::json!(format!("https://{channel_id}.mave.digital/")),
    );
    info.insert("vcodec", serde_json::json!("none"));
    info.insert_if_some("id", mave_value_string(episode.get("id")));
    info.insert_if_some("url", Some(audio_url.clone()));
    info.insert_if_some("ext", Some(format_ext.clone()));
    info.insert(
        "formats",
        serde_json::json!([{
            "format_id": "audio",
            "url": audio_url,
            "ext": format_ext,
            "vcodec": "none",
            "acodec": "mp3",
        }]),
    );
    info.insert_if_some("title", json_string(episode, "title"));
    info.insert_if_some("description", mave_description(episode.get("description")));
    info.insert_if_some("thumbnail", mave_url(episode.get("image")));
    info.insert_if_some("duration", mave_integer(episode.get("duration")));
    info.insert_if_some("season_number", mave_integer(episode.get("season")));
    info.insert_if_some("episode_number", mave_integer(episode.get("number")));
    info.insert_if_some("view_count", mave_integer(episode.get("listenings")));
    info.insert_if_some("like_count", mave_reaction_count(episode, "like"));
    info.insert_if_some("dislike_count", mave_reaction_count(episode, "dislike"));
    info.insert_if_some(
        "age_limit",
        json_bool(episode, "is_explicit")
            .filter(|is_explicit| *is_explicit)
            .map(|_| 18),
    );
    info.insert_if_some("timestamp", mave_timestamp(episode.get("publish_date")));
    info.insert_if_some("series_id", mave_value_string(channel_meta.get("id")));
    info.insert_if_some("series", json_string(channel_meta, "title"));
    info.insert_if_some("channel", json_string(channel_meta, "title"));
    info.insert_if_some("uploader", json_string(channel_meta, "author"));
    Ok(info)
}
