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

/// Recover the encrypted signature and its query parameter name from a
/// `signatureCipher` value, for handoff to the challenge solver.
fn youtube_cipher_signature(cipher: &str) -> Option<(String, String)> {
    let parsed = url::Url::parse(&format!("https://youtube.invalid/?{cipher}")).ok()?;
    let mut signature = None;
    let mut parameter = None;
    for (key, value) in parsed.query_pairs() {
        match key.as_ref() {
            "s" => signature = Some(value.into_owned()),
            "sp" => parameter = Some(value.into_owned()),
            _ => {}
        }
    }
    Some((
        signature?,
        parameter.unwrap_or_else(|| "signature".to_owned()),
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

/// Mirrors the `qualities(...)` rank table in `_extract_formats_and_subtitles`.
/// Unknown quality names rank `-1`, exactly like the Python helper.
const YOUTUBE_QUALITY_RANKS: &[&str] = &[
    "tiny",
    "audio_quality_ultralow",
    "audio_quality_low",
    "audio_quality_medium",
    "audio_quality_high",
    "small",
    "medium",
    "large",
    "hd720",
    "hd1080",
    "hd1440",
    "hd2160",
    "hd2880",
    "highres",
];

fn youtube_quality_rank(quality: &str) -> i64 {
    YOUTUBE_QUALITY_RANKS
        .iter()
        .position(|rank| *rank == quality)
        .map(|index| index as i64)
        .unwrap_or(-1)
}

/// Mirrors `get_language_code_and_preference`: descriptive tracks get a
/// `-desc` suffix and `-10`, original tracks `10`, default tracks `5`.
fn youtube_audio_language(audio_track: Option<&serde_json::Value>) -> (Option<String>, i64) {
    let track = audio_track.unwrap_or(&serde_json::Value::Null);
    let display_name = youtube_json_string(track, "displayName").unwrap_or_default();
    let display_name = display_name.to_ascii_lowercase();
    let language_code = track
        .get("id")
        .and_then(serde_json::Value::as_str)
        .and_then(|id| id.split('.').next())
        .filter(|code| !code.is_empty())
        .map(str::to_owned);
    if display_name.contains("descriptive") {
        let language = language_code.map(|code| format!("{code}-desc"));
        return (language, -10);
    }
    if display_name.contains("original") {
        return (language_code, 10);
    }
    if track
        .get("audioIsDefault")
        .and_then(serde_json::Value::as_bool)
        == Some(true)
    {
        return (language_code, 5);
    }
    (language_code, -1)
}

/// Mirrors `is_super_resolution`: an `sr=1` entry inside the `xtags` URL
/// parameter marks AI-upscaled formats.
fn youtube_is_super_resolution(url: &str) -> bool {
    url::Url::parse(url).ok().is_some_and(|parsed| {
        parsed.query_pairs().any(|(key, value)| {
            key == "xtags"
                && url::form_urlencoded::parse(value.as_bytes())
                    .any(|(name, val)| name == "sr" && val == "1")
        })
    })
}

fn youtube_join_nonempty(parts: &[Option<String>], delimiter: &str) -> Option<String> {
    let joined = parts
        .iter()
        .filter_map(|part| part.as_deref().filter(|part| !part.is_empty()))
        .collect::<Vec<_>>()
        .join(delimiter);
    (!joined.is_empty()).then_some(joined)
}

fn youtube_stream_identity(stream: &serde_json::Value) -> (String, Option<String>, bool) {
    let itag = youtube_json_i64(stream, "itag")
        .map(|itag| itag.to_string())
        .or_else(|| {
            stream
                .get("itag")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_default();
    let (language, _) = youtube_audio_language(stream.get("audioTrack"));
    let is_drc = stream.get("isDrc").and_then(serde_json::Value::as_bool) == Some(true);
    (itag, language, is_drc)
}

fn youtube_has_drm(stream: &serde_json::Value) -> bool {
    stream
        .get("drmFamilies")
        .and_then(serde_json::Value::as_object)
        .is_some_and(|drm| !drm.is_empty())
}

fn youtube_format_value(
    stream: &serde_json::Value,
    fallback_index: usize,
    duration_secs: Option<i64>,
    skip_live_adaptive: bool,
) -> Option<serde_json::Value> {
    let has_drm = youtube_has_drm(stream);
    // `FORMAT_STREAM_TYPE_OTF` adaptive formats require init-fragment probing
    // and are skipped unless DRM-flagged, mirroring `process_https_formats`.
    if stream.get("type").and_then(serde_json::Value::as_str) == Some("FORMAT_STREAM_TYPE_OTF")
        && !has_drm
    {
        return None;
    }
    // Live adaptive formats are incomplete unless post-live processing applies.
    if skip_live_adaptive && stream.get("targetDurationSec").is_some() {
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
    let raw_itag = youtube_json_i64(stream, "itag")
        .map(|itag| itag.to_string())
        .or_else(|| {
            stream
                .get("itag")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        });
    let is_drc = stream.get("isDrc").and_then(serde_json::Value::as_bool) == Some(true);
    let super_resolution = youtube_is_super_resolution(&source_url);
    let format_id = match raw_itag.clone() {
        Some(itag) => youtube_join_nonempty(
            &[
                Some(itag),
                is_drc.then_some("drc".to_owned()),
                super_resolution.then_some("sr".to_owned()),
            ],
            "-",
        )
        .expect("itag is always present here"),
        None => format!("youtube-{fallback_index}"),
    };
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
    let single_stream = !is_video || !is_audio;
    if single_stream {
        if let Some(extension) = format.get("ext").cloned() {
            let container = format!("{}_dash", extension.as_str().unwrap_or_default());
            format.insert("container".to_owned(), serde_json::json!(container));
        }
    }
    for (key, source_key) in [
        ("width", "width"),
        ("height", "height"),
        ("audio_channels", "audioChannels"),
    ] {
        if let Some(value) = stream.get(source_key).cloned() {
            format.insert(key.to_owned(), value);
        }
    }
    // Python drops wrongly-reported `fps: 1` values.
    if let Some(fps) = stream.get("fps").and_then(|fps| {
        fps.as_i64()
            .or_else(|| fps.as_u64().and_then(|fps| i64::try_from(fps).ok()))
            .or_else(|| fps.as_str().and_then(|fps| fps.parse::<i64>().ok()))
    }) {
        if fps > 1 {
            format.insert("fps".to_owned(), serde_json::json!(fps));
        }
    }
    let tbr =
        youtube_json_f64(stream, "averageBitrate").or_else(|| youtube_json_f64(stream, "bitrate"));
    if let Some(value) = tbr {
        format.insert("tbr".to_owned(), serde_json::json!(value / 1000.0));
    }
    let filesize = youtube_json_i64(stream, "contentLength")
        .or_else(|| youtube_query_value(&source_url, "clen").and_then(|value| value.parse().ok()));
    if let Some(filesize) = filesize {
        format.insert("filesize".to_owned(), serde_json::json!(filesize));
    }
    let format_duration =
        youtube_json_i64(stream, "approxDurationMs").map(|duration| duration as f64 / 1000.0);
    if let Some(duration) = format_duration {
        format.insert("duration".to_owned(), serde_json::json!(duration));
    }
    if filesize.is_none() {
        // `filesize_from_tbr` takes kilobits/sec, like the Python helper.
        if let (Some(tbr), Some(duration)) = (tbr, format_duration) {
            format.insert(
                "filesize_approx".to_owned(),
                serde_json::json!((duration * (tbr / 1000.0) * (1000.0 / 8.0)) as i64),
            );
        }
    }
    // Damaged formats (much shorter than the video) are deprioritized,
    // mirroring the `duration // 2` guard in `process_format_stream`.
    let is_damaged = match (format_duration, duration_secs) {
        (Some(format_duration), Some(duration_secs)) => {
            format_duration < duration_secs as f64 / 2.0
        }
        _ => false,
    };
    let mut quality_name = youtube_json_string(stream, "quality").unwrap_or_default();
    if quality_name == "tiny" || quality_name.is_empty() {
        let audio_quality = youtube_json_string(stream, "audioQuality")
            .map(|quality| quality.to_ascii_lowercase())
            .filter(|quality| !quality.is_empty());
        quality_name = audio_quality.unwrap_or(quality_name);
    }
    // The 3gp itag 17 is worse than its peers despite a "small" label.
    if raw_itag.as_deref() == Some("17") {
        quality_name = "tiny".to_owned();
    }
    let display_name = stream
        .get("audioTrack")
        .and_then(|track| youtube_json_string(track, "displayName"));
    let audio_default = stream.get("audioTrack").and_then(|track| {
        track
            .get("audioIsDefault")
            .and_then(serde_json::Value::as_bool)
    }) == Some(true);
    let short_name = youtube_json_string(stream, "qualityLabel")
        .or_else(|| (!quality_name.is_empty()).then(|| quality_name.replace("audio_quality_", "")));
    let projection = stream
        .get("projectionType")
        .and_then(serde_json::Value::as_str)
        .map(|projection| projection.replace("RECTANGULAR", "").to_ascii_lowercase())
        .filter(|projection| !projection.trim().is_empty());
    let spatial_audio = stream
        .get("spatialAudioType")
        .and_then(serde_json::Value::as_str)
        .map(|audio| {
            audio
                .replace("SPATIAL_AUDIO_TYPE_", "")
                .to_ascii_lowercase()
        })
        .filter(|audio| !audio.trim().is_empty());
    if let Some(note) = youtube_join_nonempty(
        &[
            youtube_join_nonempty(
                &[
                    display_name.clone(),
                    audio_default.then_some("(default)".to_owned()),
                ],
                " ",
            ),
            short_name.clone(),
            is_drc.then_some("DRC".to_owned()),
            super_resolution.then_some("AI-upscaled".to_owned()),
            projection,
            spatial_audio,
            is_damaged.then_some("DAMAGED".to_owned()),
        ],
        ", ",
    ) {
        format.insert("format_note".to_owned(), serde_json::json!(note));
    }
    if raw_itag.as_deref() == Some("22") {
        format.insert("source_preference".to_owned(), serde_json::json!(-5));
    } else {
        let premium = short_name
            .as_deref()
            .is_some_and(|name| name.contains("Premium"));
        format.insert(
            "source_preference".to_owned(),
            serde_json::json!(if premium { 99 } else { -1 }),
        );
    }
    // Format 22 is likely damaged; itag 17 is 3gp.
    if is_damaged {
        format.insert("preference".to_owned(), serde_json::json!(-10));
    } else if raw_itag.as_deref() == Some("17") {
        format.insert("preference".to_owned(), serde_json::json!(-2));
    }
    format.insert(
        "quality".to_owned(),
        serde_json::json!(youtube_quality_rank(&quality_name)),
    );
    let (language, language_preference) = youtube_audio_language(stream.get("audioTrack"));
    if let Some(language) = language {
        format.insert("language".to_owned(), serde_json::json!(language));
    }
    format.insert(
        "language_preference".to_owned(),
        serde_json::json!(language_preference),
    );
    if let Some(display_name) = display_name {
        format.insert("audio_track".to_owned(), serde_json::json!(display_name));
    }
    if has_drm {
        format.insert("has_drm".to_owned(), serde_json::json!(true));
    }
    if signature_challenge.is_some() {
        format.insert(
            "rust_todo".to_owned(),
            serde_json::json!("TODO: YouTube signatureCipher requires the native player-JavaScript solver"),
        );
    }
    Some(serde_json::Value::Object(format))
}

/// Extract an `n` challenge from a format or manifest URL, either from the
/// `n` query parameter or from an `/n/<challenge>/` path segment (manifest
/// URLs), mirroring `get_manifest_n_challenge` and the query check in
/// `process_https_formats`.
fn youtube_n_challenge(url: &str) -> Option<(String, bool)> {
    if let Some(value) = youtube_query_value(url, "n") {
        return Some((value, false));
    }
    let path = url::Url::parse(url).ok()?.path().to_owned();
    Regex::new(r"/n/([^/]+)/")
        .ok()?
        .captures(&path)
        .ok()
        .flatten()?
        .get(1)
        .map(|challenge| (challenge.as_str().to_owned(), true))
}

pub(crate) fn youtube_formats_and_todos(
    responses: &[serde_json::Value],
    duration_secs: Option<i64>,
    live_status: Option<&str>,
) -> (Vec<serde_json::Value>, Vec<String>, YoutubeChallenges) {
    let mut formats = Vec::new();
    let mut todos = Vec::new();
    let mut challenges = YoutubeChallenges::default();
    let mut seen_urls = std::collections::BTreeSet::new();
    let mut seen_streams = std::collections::BTreeSet::new();
    let mut seen_n = std::collections::BTreeSet::new();
    let mut seen_drm_notice = false;
    // Without live-from-start handling, live adaptive formats are incomplete
    // unless the video is post-live, mirroring `_needs_live_processing`.
    let skip_live_adaptive = live_status != Some("post_live");
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
                // Duplicate (itag, language, DRC) streams across player
                // responses are exposed once, like the default `stream_ids`
                // handling in `process_https_formats`.
                if !seen_streams.insert(youtube_stream_identity(stream)) {
                    continue;
                }
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
                if youtube_has_drm(stream) && !seen_drm_notice {
                    seen_drm_notice = true;
                    todos.push(
                        "TODO: YouTube DRM-protected formats are exposed with has_drm and may not be downloadable"
                            .to_owned(),
                    );
                }
                if let Some(format) =
                    youtube_format_value(stream, index, duration_secs, skip_live_adaptive)
                {
                    let format_index = formats.len();
                    // Record solvable challenges alongside the pushed format
                    // so the solver can rewrite URLs by index.
                    if let Some(cipher) = stream
                        .get("signatureCipher")
                        .or_else(|| stream.get("cipher"))
                        .and_then(serde_json::Value::as_str)
                        .and_then(youtube_cipher_signature)
                    {
                        challenges.sig.push(YoutubeSigChallenge {
                            format_index,
                            encrypted: cipher.0,
                            param: cipher.1,
                        });
                    }
                    if let Some((value, in_path)) = youtube_n_challenge(&url) {
                        challenges.n.push(YoutubeNChallenge {
                            format_index,
                            value,
                            in_path,
                        });
                    }
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
            let format_index = formats.len();
            if let Some((value, in_path)) = youtube_n_challenge(manifest_url) {
                challenges.n.push(YoutubeNChallenge {
                    format_index,
                    value,
                    in_path,
                });
            }
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
    (formats, todos, challenges)
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
            // Exact `_SUBTITLE_FORMATS` order. `xosf` is stripped because it
            // produces undesirable text positioning, mirroring
            // `process_language`.
            let base_url = youtube_strip_query_param(base_url, "xosf").unwrap_or_else(|| base_url.to_owned());
            for extension in ["json3", "srv1", "srv2", "srv3", "ttml", "srt", "vtt"] {
                let Some(url) = youtube_update_query(&base_url, &[("fmt", extension)]) else {
                    continue;
                };
                entries.push(serde_json::json!({
                    "ext": extension,
                    "url": url,
                    "name": name,
                    "impersonate": true,
                }));
            }
        }
    }
    (
        serde_json::Value::Object(subtitles),
        serde_json::Value::Object(automatic),
    )
}
