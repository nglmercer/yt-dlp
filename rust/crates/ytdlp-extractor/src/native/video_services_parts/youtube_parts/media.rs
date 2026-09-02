fn youtube_cipher_parameters(cipher: &str) -> Option<(String, Option<String>)> {
    let parsed = url::Url::parse(&format!("https://youtube.invalid/?{cipher}")).ok()?;
    let mut source_url = None;
    let mut signature = None;
    let mut signature_parameter = None;
    for (key, value) in parsed.query_pairs() {
        match key.as_ref() {
            "url" => source_url = Some(value.into_owned()),
            "s" => signature = Some(value.into_owned()),
            "sp" => signature_parameter = Some(value.into_owned()),
            _ => {}
        }
    }
    let source_url = source_url?;
    let Some(signature) = signature else {
        return Some((source_url, None));
    };
    let parameter = signature_parameter.unwrap_or_else(|| "signature".to_owned());
    Some((
        youtube_update_query(&source_url, &[(&parameter, &signature)])?,
        Some(signature),
    ))
}

fn youtube_codec_parts(mime_type: &str) -> (Option<String>, Option<String>) {
    let codecs = mime_type
        .split_once("codecs=\"")
        .and_then(|(_, codecs)| codecs.split_once('"').map(|(codecs, _)| codecs))
        .unwrap_or_default();
    let codecs = codecs
        .split(',')
        .map(str::trim)
        .filter(|codec| !codec.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut video = None;
    let mut audio = None;
    for codec in codecs {
        let lower = codec.to_ascii_lowercase();
        if lower.starts_with("avc")
            || lower.starts_with("av01")
            || lower.starts_with("vp8")
            || lower.starts_with("vp9")
            || lower.starts_with("hev")
            || lower.starts_with("hvc")
        {
            video = Some(codec);
        } else if lower.starts_with("mp4a")
            || lower.starts_with("opus")
            || lower.starts_with("vorbis")
            || lower.starts_with("ac-3")
            || lower.starts_with("ec-3")
            || lower.starts_with("flac")
        {
            audio = Some(codec);
        }
    }
    (video, audio)
}

fn youtube_format_value(
    stream: &serde_json::Value,
    fallback_index: usize,
) -> Option<serde_json::Value> {
    if stream
        .get("drmFamilies")
        .and_then(serde_json::Value::as_object)
        .is_some_and(|drm| !drm.is_empty())
    {
        return None;
    }
    let mut source_url = stream
        .get("url")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    let mut signature_challenge = None;
    if source_url.is_none() {
        if let Some(cipher) = stream
            .get("signatureCipher")
            .or_else(|| stream.get("cipher"))
            .and_then(serde_json::Value::as_str)
        {
            let (url, signature) = youtube_cipher_parameters(cipher)?;
            source_url = Some(url);
            signature_challenge = signature;
        }
    }
    let source_url = source_url?;
    let mime_type = stream
        .get("mimeType")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let mime_base = mime_type.split(';').next().unwrap_or(mime_type);
    let (video_codec, audio_codec) = youtube_codec_parts(mime_type);
    let is_video = mime_base.starts_with("video/") || video_codec.is_some();
    let is_audio = mime_base.starts_with("audio/") || audio_codec.is_some();
    let format_id = youtube_json_i64(stream, "itag")
        .map(|itag| itag.to_string())
        .or_else(|| {
            stream
                .get("itag")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| format!("youtube-{fallback_index}"));
    let extension = mimetype_extension(Some(mime_base)).or_else(|| {
        let inferred = yt_dlp_core::determine_ext(Some(&source_url), "");
        (!inferred.is_empty()).then_some(inferred)
    });
    let mut format = serde_json::Map::new();
    format.insert("format_id".to_owned(), serde_json::json!(format_id));
    format.insert("url".to_owned(), serde_json::json!(source_url));
    format.insert("protocol".to_owned(), serde_json::json!("http"));
    if let Some(extension) = extension {
        format.insert("ext".to_owned(), serde_json::json!(extension));
    }
    format.insert(
        "vcodec".to_owned(),
        serde_json::json!(if is_video {
            video_codec.as_deref().unwrap_or("unknown")
        } else {
            "none"
        }),
    );
    format.insert(
        "acodec".to_owned(),
        serde_json::json!(if is_audio {
            audio_codec.as_deref().unwrap_or("unknown")
        } else {
            "none"
        }),
    );
    for (key, source_key) in [
        ("width", "width"),
        ("height", "height"),
        ("fps", "fps"),
        ("audio_channels", "audioChannels"),
    ] {
        if let Some(value) = stream.get(source_key).cloned() {
            format.insert(key.to_owned(), value);
        }
    }
    if let Some(value) = youtube_json_f64(stream, "bitrate")
        .or_else(|| youtube_json_f64(stream, "averageBitrate"))
    {
        format.insert("tbr".to_owned(), serde_json::json!(value / 1000.0));
    }
    let filesize = youtube_json_i64(stream, "contentLength").or_else(|| {
        youtube_query_value(&source_url, "clen").and_then(|value| value.parse().ok())
    });
    if let Some(filesize) = filesize {
        format.insert("filesize".to_owned(), serde_json::json!(filesize));
    }
    if let Some(quality) = youtube_json_string(stream, "qualityLabel")
        .or_else(|| youtube_json_string(stream, "quality"))
    {
        format.insert("format_note".to_owned(), serde_json::json!(quality));
    }
    if let Some(audio_track) = stream.get("audioTrack") {
        if let Some(language) = youtube_json_string(audio_track, "id")
            .and_then(|value| value.split('.').next().map(str::to_owned))
        {
            format.insert("language".to_owned(), serde_json::json!(language));
        }
        if let Some(display_name) = youtube_json_string(audio_track, "displayName") {
            format.insert("audio_track".to_owned(), serde_json::json!(display_name));
        }
    }
    if let Some(duration) = youtube_json_i64(stream, "approxDurationMs") {
        format.insert("duration".to_owned(), serde_json::json!(duration as f64 / 1000.0));
    }
    if signature_challenge.is_some() {
        format.insert(
            "rust_todo".to_owned(),
            serde_json::json!("TODO: YouTube signatureCipher requires the native player-JavaScript solver"),
        );
    }
    Some(serde_json::Value::Object(format))
}

pub(crate) fn youtube_formats_and_todos(
    responses: &[serde_json::Value],
) -> (Vec<serde_json::Value>, Vec<String>) {
    let mut formats = Vec::new();
    let mut todos = Vec::new();
    let mut seen_urls = std::collections::BTreeSet::new();
    let mut seen_n = std::collections::BTreeSet::new();
    for response in responses {
        let Some(streaming_data) = response.get("streamingData") else {
            continue;
        };
        for key in ["formats", "adaptiveFormats"] {
            let Some(streams) = streaming_data.get(key).and_then(serde_json::Value::as_array)
            else {
                continue;
            };
            for (index, stream) in streams.iter().enumerate() {
                let candidate_url = stream
                    .get("url")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
                    .or_else(|| {
                        stream
                            .get("signatureCipher")
                            .and_then(serde_json::Value::as_str)
                            .and_then(|cipher| youtube_cipher_parameters(cipher).map(|(url, _)| url))
                    });
                let Some(url) = candidate_url else {
                    if stream.get("signatureCipher").is_some() || stream.get("cipher").is_some() {
                        todos.push(
                            "TODO: YouTube signatureCipher format was not exposed because its player signature is unresolved"
                                .to_owned(),
                        );
                    }
                    continue;
                };
                if !seen_urls.insert(url.clone()) {
                    continue;
                }
                if stream
                    .get("signatureCipher")
                    .or_else(|| stream.get("cipher"))
                    .and_then(serde_json::Value::as_str)
                    .and_then(youtube_cipher_parameters)
                    .is_some_and(|(_, signature)| signature.is_some())
                {
                    todos.push(
                        "TODO: YouTube signatureCipher requires the native player-JavaScript solver"
                            .to_owned(),
                    );
                }
                if youtube_url_has_n_challenge(&url) {
                    if let Some(challenge) = youtube_query_value(&url, "n") {
                        if seen_n.insert(challenge) {
                            todos.push(
                                "TODO: YouTube n challenge requires the native player-JavaScript solver"
                                    .to_owned(),
                            );
                        }
                    }
                }
                if let Some(format) = youtube_format_value(stream, index) {
                    formats.push(format);
                }
            }
        }
        for (key, protocol, extension) in [
            ("hlsManifestUrl", "m3u8_native", "mp4"),
            ("dashManifestUrl", "http_dash_segments", "mp4"),
        ] {
            let Some(manifest_url) = streaming_data.get(key).and_then(serde_json::Value::as_str)
            else {
                continue;
            };
            if youtube_url_has_n_challenge(manifest_url) {
                todos.push(format!(
                    "TODO: YouTube {protocol} manifest n challenge requires the native player-JavaScript solver"
                ));
            }
            let format_id = if protocol == "m3u8_native" { "hls" } else { "dash" };
            formats.push(serde_json::json!({
                "format_id": format_id,
                "format_note": protocol,
                "url": manifest_url,
                "ext": extension,
                "protocol": protocol,
                "vcodec": "unknown",
                "acodec": "unknown",
            }));
        }
    }
    (formats, todos)
}

fn youtube_caption_entries(
    response: &serde_json::Value,
) -> (serde_json::Value, serde_json::Value) {
    let mut subtitles = serde_json::Map::new();
    let mut automatic = serde_json::Map::new();
    let tracks = response
        .get("captions")
        .and_then(|captions| captions.get("playerCaptionsTracklistRenderer"))
        .and_then(|renderer| renderer.get("captionTracks"))
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten();
    for track in tracks {
        let Some(base_url) = track
            .get("baseUrl")
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        let language = youtube_json_string(track, "languageCode")
            .or_else(|| youtube_json_string(track, "vssId").map(|id| id.trim_start_matches('.').replace('.', "-")))
            .unwrap_or_else(|| "und".to_owned());
        let name = track
            .get("name")
            .and_then(youtube_text)
            .unwrap_or_else(|| language.clone());
        let target = if track.get("kind").and_then(serde_json::Value::as_str) == Some("asr") {
            &mut automatic
        } else {
            &mut subtitles
        };
        let entries = target.entry(language).or_insert_with(|| serde_json::json!([]));
        if let Some(entries) = entries.as_array_mut() {
            for extension in ["json3", "srv3", "ttml", "srt", "vtt"] {
                let Some(url) = youtube_update_query(base_url, &[("fmt", extension)]) else {
                    continue;
                };
                entries.push(serde_json::json!({
                    "ext": extension,
                    "url": url,
                    "name": name,
                }));
            }
        }
    }
    (
        serde_json::Value::Object(subtitles),
        serde_json::Value::Object(automatic),
    )
}
