const YOUTUBE_FIXTURE_ID: &str = "dQw4w9WgXcQ";

fn youtube_fixture_player_response(with_streams: bool) -> serde_json::Value {
    let mut response = serde_json::json!({
        "playabilityStatus": {
            "status": "OK",
            "playableInEmbed": true
        },
        "videoDetails": {
            "videoId": YOUTUBE_FIXTURE_ID,
            "title": "Native YouTube fixture",
            "shortDescription": "A Rust-only YouTube extraction fixture.",
            "lengthSeconds": "42",
            "viewCount": "1234",
            "author": "Rust Channel",
            "channelId": "UCnativefixture",
            "isLive": false,
            "isLiveContent": false,
            "keywords": ["rust", "youtube"] ,
            "thumbnail": {
                "thumbnails": [
                    {"url": "https://i.ytimg.com/vi/dQw4w9WgXcQ/default.jpg", "width": 120, "height": 90},
                    {"url": "https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg", "width": 480, "height": 360}
                ]
            }
        },
        "microformat": {
            "playerMicroformatRenderer": {
                "publishDate": "2026-08-31",
                "category": "Science & Technology",
                "ownerChannelName": "Rust Channel",
                "isFamilySafe": true
            }
        },
        "captions": {
            "playerCaptionsTracklistRenderer": {
                "captionTracks": [
                    {
                        "baseUrl": "https://www.youtube.com/api/timedtext?v=dQw4w9WgXcQ&lang=en",
                        "languageCode": "en",
                        "name": {"simpleText": "English"}
                    },
                    {
                        "baseUrl": "https://www.youtube.com/api/timedtext?v=dQw4w9WgXcQ&lang=es",
                        "languageCode": "es",
                        "kind": "asr",
                        "name": {"simpleText": "Spanish (auto-generated)"}
                    }
                ]
            }
        }
    });
    if with_streams {
        response["streamingData"] = serde_json::json!({
            "expiresInSeconds": "21600",
            "formats": [
                {
                    "itag": 18,
                    "url": "https://rr1---sn.example.googlevideo.com/videoplayback?itag=18&clen=123456",
                    "mimeType": "video/mp4; codecs=\"avc1.42001E, mp4a.40.2\"",
                    "quality": "medium",
                    "qualityLabel": "360p",
                    "width": 640,
                    "height": 360,
                    "fps": 30,
                    "bitrate": 600000,
                    "contentLength": "123456"
                }
            ],
            "adaptiveFormats": [
                {
                    "itag": 137,
                    "url": "https://rr1---sn.example.googlevideo.com/videoplayback?itag=137&clen=999999",
                    "mimeType": "video/mp4; codecs=\"avc1.640028\"",
                    "quality": "hd1080",
                    "qualityLabel": "1080p",
                    "width": 1920,
                    "height": 1080,
                    "fps": 30,
                    "bitrate": 4000000,
                    "contentLength": "999999"
                },
                {
                    "itag": 140,
                    "url": "https://rr1---sn.example.googlevideo.com/videoplayback?itag=140&clen=555555",
                    "mimeType": "audio/mp4; codecs=\"mp4a.40.2\"",
                    "audioQuality": "AUDIO_QUALITY_MEDIUM",
                    "audioSampleRate": "44100",
                    "audioChannels": 2,
                    "bitrate": 128000,
                    "contentLength": "555555",
                    "audioTrack": {"id": "en.0", "displayName": "English", "audioIsDefault": true}
                }
            ]
        });
    }
    response
}

fn youtube_fixture_page(with_streams: bool) -> Vec<u8> {
    let config = serde_json::json!({
        "INNERTUBE_API_KEY": "fixture-api-key",
        "INNERTUBE_CONTEXT_CLIENT_NAME": 1,
        "INNERTUBE_CONTEXT": {
            "client": {
                "clientName": "WEB",
                "clientVersion": "2.20260708.00.00"
            }
        }
    });
    let player = serde_json::to_string(&youtube_fixture_player_response(with_streams)).unwrap();
    format!(
        "<html><script>ytcfg.set({config});</script><script>var ytInitialPlayerResponse = {player};</script></html>",
        config = config,
        player = player
    )
    .into_bytes()
}

struct YoutubeFixtureHandler {
    page: Vec<u8>,
    api: Vec<u8>,
    player_js: Vec<u8>,
}

impl yt_dlp_networking::RequestHandler for YoutubeFixtureHandler {
    fn name(&self) -> &str {
        "youtube-fixture"
    }

    fn supports(
        &self,
        _request: &yt_dlp_networking::Request,
    ) -> Result<(), yt_dlp_networking::RequestError> {
        Ok(())
    }

    fn send(
        &self,
        request: &yt_dlp_networking::Request,
    ) -> Result<yt_dlp_networking::Response, yt_dlp_networking::RequestError> {
        let body = if request.url().contains("/youtubei/v1/player") {
            self.api.clone()
        } else if request.url().contains("/s/player/") {
            self.player_js.clone()
        } else {
            self.page.clone()
        };
        Ok(yt_dlp_networking::Response::new(
            request.url(),
            200,
            "OK",
            body,
        ))
    }
}

fn youtube_fixture_context(page_streams: bool) -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(YoutubeFixtureHandler {
        page: youtube_fixture_page(page_streams),
        api: serde_json::to_vec(&youtube_fixture_player_response(true)).unwrap(),
        player_js: Vec::new(),
    });
    ExtractionContext::new(director, CookieJar::new().shared())
}

const YOUTUBE_FIXTURE_PLAYER_ID: &str = "abcd1234";

fn youtube_challenge_player_response() -> serde_json::Value {
    serde_json::json!({
        "videoDetails": {
            "videoId": YOUTUBE_FIXTURE_ID,
            "title": "Challenge fixture",
            "lengthSeconds": "10",
            "author": "Rust Channel",
            "channelId": "UCnativefixture",
            "isLive": false,
            "isLiveContent": false
        },
        "streamingData": {
            "adaptiveFormats": [
                {
                    "itag": 251,
                    "signatureCipher": "url=https%3A%2F%2Fmedia.example%2Fa.webm%3Fn%3Dplain&s=encrypted&sp=sig",
                    "mimeType": "audio/webm; codecs=\"opus\""
                },
                {
                    "itag": 140,
                    "url": "https://media.example/a.m4a?n=challenge",
                    "mimeType": "audio/mp4; codecs=\"mp4a.40.2\""
                }
            ]
        }
    })
}

fn youtube_challenge_context() -> ExtractionContext {
    let config = serde_json::json!({
        "INNERTUBE_API_KEY": "fixture-api-key",
        "PLAYER_JS_URL": format!("/s/player/{YOUTUBE_FIXTURE_PLAYER_ID}/player_ias.vflset/en_US/base.js"),
        "INNERTUBE_CONTEXT": {
            "client": {
                "clientName": "WEB",
                "clientVersion": "2.20260708.00.00"
            }
        }
    });
    let player = serde_json::to_string(&youtube_challenge_player_response()).unwrap();
    let page = format!(
        "<html><script>ytcfg.set({config});</script><script>var ytInitialPlayerResponse = {player};</script></html>",
        config = config,
    )
    .into_bytes();
    let mut director = RequestDirector::new();
    director.add_handler(YoutubeFixtureHandler {
        page,
        api: serde_json::to_vec(&youtube_challenge_player_response()).unwrap(),
        player_js: b"var _yt_player = {signatureTimestamp:19234};".to_vec(),
    });
    ExtractionContext::new(director, CookieJar::new().shared())
}

#[test]
fn youtube_video_urls_are_reduced_to_official_video_ids() {
    for url in [
        "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
        "https://youtu.be/dQw4w9WgXcQ?t=3",
        "https://www.youtube.com/shorts/dQw4w9WgXcQ",
        "https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ",
        "dQw4w9WgXcQ",
    ] {
        assert_eq!(
            crate::native::youtube_video_id(url).as_deref(),
            Some(YOUTUBE_FIXTURE_ID)
        );
    }
    assert!(
        crate::native::youtube_video_id("https://www.youtube.com/playlist?list=PLfixture")
            .is_none()
    );
    assert!(crate::native::youtube_video_id("https://example.test/watch?v=dQw4w9WgXcQ").is_none());
}

#[test]
fn youtube_native_extractor_maps_player_metadata_formats_and_captions() {
    let descriptor = ExtractorDescriptor::new(
        "YoutubeIE",
        "youtube",
        r"https?://(?:www\.)?youtube\.com/.*",
        true,
    );
    let extractor = YoutubeExtractor::new(descriptor).unwrap();
    let result = extractor
        .extract_with_context(
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            &youtube_fixture_context(true),
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some(YOUTUBE_FIXTURE_ID));
    assert_eq!(result.get_str("title"), Some("Native YouTube fixture"));
    assert_eq!(result.get_str("channel_id"), Some("UCnativefixture"));
    assert_eq!(result.get_i64("duration"), Some(42));
    assert_eq!(result.get_i64("view_count"), Some(1234));
    assert_eq!(result.get_str("upload_date"), Some("20260831"));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg")
    );
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 3);
    assert_eq!(formats[0].get("format_id"), Some(&serde_json::json!("18")));
    assert_eq!(
        formats[0].get("vcodec"),
        Some(&serde_json::json!("avc1.42001E"))
    );
    assert_eq!(formats[1].get("acodec"), Some(&serde_json::json!("none")));
    assert_eq!(formats[2].get("vcodec"), Some(&serde_json::json!("none")));
    assert_eq!(formats[2].get("language"), Some(&serde_json::json!("en")));
    assert!(
        result
            .get("subtitles")
            .and_then(|value| value.get("en"))
            .is_some()
    );
    assert!(
        result
            .get("automatic_captions")
            .and_then(|value| value.get("es"))
            .is_some()
    );
}

#[test]
fn youtube_native_extractor_uses_player_api_when_page_has_no_streams() {
    let extractor = YoutubeExtractor::new(ExtractorDescriptor::new(
        "YoutubeIE",
        "youtube",
        r"https?://(?:www\.)?youtube\.com/.*",
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            &youtube_fixture_context(false),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_str("title"), Some("Native YouTube fixture"));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(3)
    );
}

#[test]
fn youtube_native_extractor_marks_signature_and_n_challenges_as_todo() {
    let response = serde_json::json!({
        "videoDetails": {"videoId": YOUTUBE_FIXTURE_ID, "title": "Challenge"},
        "streamingData": {
            "adaptiveFormats": [
                {
                    "itag": 251,
                    "signatureCipher": "url=https%3A%2F%2Fmedia.example%2Fa.webm%3Fn%3Dchallenge&s=encrypted&sp=sig",
                    "mimeType": "audio/webm; codecs=\"opus\""
                },
                {
                    "itag": 140,
                    "url": "https://media.example/a.m4a?n=challenge",
                    "mimeType": "audio/mp4; codecs=\"mp4a.40.2\""
                }
            ]
        }
    });
    let (formats, todos, challenges) =
        crate::native::youtube_formats_and_todos(&[response], None, Some("not_live"));
    assert_eq!(formats.len(), 2);
    assert_eq!(challenges.sig.len(), 1);
    // Both the cipher URL and the plain URL carry the same `n` challenge.
    assert_eq!(challenges.n.len(), 2);
    assert!(
        formats
            .iter()
            .any(|format| format.get("rust_todo").is_some())
    );
    assert!(todos.iter().any(|todo| todo.contains("signatureCipher")));
    assert!(todos.iter().any(|todo| todo.contains("n challenge")));
}

#[test]
fn youtube_player_url_is_resolved_from_page_configuration() {
    let ytcfg = serde_json::json!({
        "PLAYER_JS_URL": format!("/s/player/{YOUTUBE_FIXTURE_PLAYER_ID}/player_ias.vflset/en_US/base.js"),
    });
    assert_eq!(
        crate::native::youtube_extract_player_url(&ytcfg).as_deref(),
        Some("https://www.youtube.com/s/player/abcd1234/player_ias.vflset/en_US/base.js"),
    );
}

#[test]
fn youtube_player_url_falls_back_to_web_player_context_configs() {
    let ytcfg = serde_json::json!({
        "WEB_PLAYER_CONTEXT_CONFIGS": {
            "WEB_PLAYER_CONTEXT_CONFIG_ID_KEVLAR_WATCH": {
                "jsUrl": format!("/s/player/{YOUTUBE_FIXTURE_PLAYER_ID}/player_ias.vflset/en_US/base.js"),
            },
        },
    });
    assert_eq!(
        crate::native::youtube_extract_player_url(&ytcfg).as_deref(),
        Some("https://www.youtube.com/s/player/abcd1234/player_ias.vflset/en_US/base.js"),
    );
    assert!(crate::native::youtube_extract_player_url(&serde_json::json!({})).is_none());
}

#[test]
fn youtube_player_url_normalises_non_default_variants_to_main() {
    // A non-default variant keeps the player ID but uses the main script path.
    let url = crate::native::youtube_construct_player_url(
        None,
        Some("/s/player/abcd1234/player_es5.vflset/en_US/base.js"),
    )
    .unwrap();
    assert_eq!(
        url,
        "https://www.youtube.com/s/player/abcd1234/player_ias.vflset/en_US/base.js",
    );
    // Localised default-variant paths are already the default variant.
    let url = crate::native::youtube_construct_player_url(
        None,
        Some("/s/player/abcd1234/player_ias.vflset/de_DE/base.js"),
    )
    .unwrap();
    assert_eq!(
        url,
        "https://www.youtube.com/s/player/abcd1234/player_ias.vflset/de_DE/base.js",
    );
    // A bare player ID builds the default player URL.
    let url =
        crate::native::youtube_construct_player_url(Some(YOUTUBE_FIXTURE_PLAYER_ID), None).unwrap();
    assert_eq!(
        url,
        "https://www.youtube.com/s/player/abcd1234/player_ias.vflset/en_US/base.js",
    );
    // URLs without a player hash are rejected like `_extract_player_info`.
    assert!(
        crate::native::youtube_construct_player_url(None, Some("/watch?v=dQw4w9WgXcQ")).is_err()
    );
    assert!(crate::native::youtube_construct_player_url(None, None).is_err());
}

#[test]
fn youtube_player_cache_key_covers_known_and_unknown_variants() {
    assert_eq!(
        crate::native::youtube_player_js_cache_key(
            "https://www.youtube.com/s/player/abcd1234/player_ias.vflset/en_US/base.js"
        )
        .as_deref(),
        Some("abcd1234-main"),
    );
    assert_eq!(
        crate::native::youtube_player_js_cache_key(
            "https://www.youtube.com/s/player/abcd1234/custom_variant.9/base.js"
        )
        .as_deref(),
        Some("abcd1234-custom_variant_9_base"),
    );
    assert!(
        crate::native::youtube_player_js_cache_key("https://www.youtube.com/watch?v=dQw4w9WgXcQ")
            .is_none()
    );
}

#[test]
fn youtube_signature_timestamp_prefers_page_config_then_player_script() {
    let ytcfg = serde_json::json!({"STS": 19234});
    assert_eq!(
        crate::native::youtube_signature_timestamp(&ytcfg, None),
        Some(19234)
    );
    let ytcfg = serde_json::json!({"STS": "19234"});
    assert_eq!(
        crate::native::youtube_signature_timestamp(&ytcfg, None),
        Some(19234)
    );
    let ytcfg = serde_json::json!({});
    assert_eq!(
        crate::native::youtube_signature_timestamp(
            &ytcfg,
            Some("var _yt_player = {signatureTimestamp:19234};")
        ),
        Some(19234)
    );
    assert_eq!(
        crate::native::youtube_signature_timestamp(&ytcfg, Some("var _yt_player = {sts:12345};")),
        Some(12345)
    );
    assert_eq!(
        crate::native::youtube_signature_timestamp(&ytcfg, Some("var _yt_player = {};")),
        None
    );
}

#[test]
fn youtube_native_extractor_names_player_revision_in_challenge_todos() {
    let extractor = YoutubeExtractor::new(ExtractorDescriptor::new(
        "YoutubeIE",
        "youtube",
        r"https?://(?:www\.)?youtube\.com/.*",
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            &youtube_challenge_context(),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_str("title"), Some("Challenge fixture"));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    let todos = result
        .get("rust_todo")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    assert!(!todos.is_empty());
    assert!(todos.iter().all(|todo| {
        todo.as_str()
            .is_some_and(|todo| todo.contains("player abcd1234, sts 19234"))
    }));
}

#[test]
fn youtube_missing_player_inventory_keeps_challenge_todos_explicit() {
    let extractor = YoutubeExtractor::new(ExtractorDescriptor::new(
        "YoutubeIE",
        "youtube",
        r"https?://(?:www\.)?youtube\.com/.*",
        true,
    ))
    .unwrap();
    // `youtube_page_context_for` has no player-JS URL, so inventory fails
    // before any solver runtime is attempted.
    let result = extractor
        .extract_with_context(
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            &youtube_page_context_for(youtube_challenge_player_response()),
        )
        .unwrap()
        .into_info_dict();
    let todos = result
        .get("rust_todo")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    assert!(todos.iter().any(|todo| todo
        .as_str()
        .is_some_and(|todo| todo.contains("player JavaScript URL was not found"))));
    assert!(todos.iter().any(|todo| todo
        .as_str()
        .is_some_and(|todo| todo.contains("signatureCipher"))));
    assert!(todos.iter().any(|todo| todo
        .as_str()
        .is_some_and(|todo| todo.contains("n challenge"))));
}

fn youtube_page_context_for(player: serde_json::Value) -> ExtractionContext {
    let config = serde_json::json!({
        "INNERTUBE_API_KEY": "fixture-api-key",
        "INNERTUBE_CONTEXT": {
            "client": {
                "clientName": "WEB",
                "clientVersion": "2.20260708.00.00"
            }
        }
    });
    let player = serde_json::to_string(&player).unwrap();
    let page = format!(
        "<html><script>ytcfg.set({config});</script><script>var ytInitialPlayerResponse = {player};</script></html>",
        config = config,
    )
    .into_bytes();
    let mut director = RequestDirector::new();
    director.add_handler(YoutubeFixtureHandler {
        page,
        api: b"{}".to_vec(),
        player_js: Vec::new(),
    });
    ExtractionContext::new(director, CookieJar::new().shared())
}

#[test]
fn youtube_format_model_matches_python_field_contract() {
    let response = serde_json::json!({
        "videoDetails": {
            "videoId": YOUTUBE_FIXTURE_ID,
            "title": "Parity",
            "lengthSeconds": "100",
            "isLive": false,
            "isLiveContent": false
        },
        "streamingData": {
            "formats": [
                {
                    "itag": 22,
                    "url": "https://media.example/v.mp4?clen=1000",
                    "mimeType": "video/mp4; codecs=\"avc1.42001E, mp4a.40.2\"",
                    "quality": "medium",
                    "qualityLabel": "720p",
                    "width": 1280,
                    "height": 720,
                    "fps": 30,
                    "bitrate": 2000000,
                    "contentLength": "1000",
                    "approxDurationMs": "100000"
                },
                {
                    "itag": 17,
                    "url": "https://media.example/v.3gp",
                    "mimeType": "video/3gpp; codecs=\"mp4v.20.3, mp4a.40.2\"",
                    "quality": "small",
                    "width": 176,
                    "height": 144,
                    "fps": 1
                },
                {
                    "itag": 18,
                    "url": "https://media.example/damaged.mp4",
                    "mimeType": "video/mp4; codecs=\"avc1.42001E, mp4a.40.2\"",
                    "quality": "medium",
                    "approxDurationMs": "10000"
                },
                {
                    "itag": 18,
                    "url": "https://media.example/damaged.mp4",
                    "mimeType": "video/mp4; codecs=\"avc1.42001E, mp4a.40.2\"",
                    "quality": "medium",
                    "approxDurationMs": "10000"
                }
            ],
            "adaptiveFormats": [
                {
                    "itag": 616,
                    "url": "https://media.example/premium.mp4",
                    "mimeType": "video/mp4; codecs=\"avc1.640028\"",
                    "quality": "hd1080",
                    "qualityLabel": "1080p Premium",
                    "width": 1920,
                    "height": 1080
                },
                {
                    "itag": 140,
                    "url": "https://media.example/a.m4a",
                    "mimeType": "audio/mp4; codecs=\"mp4a.40.2\"",
                    "audioQuality": "AUDIO_QUALITY_MEDIUM",
                    "bitrate": 128000,
                    "approxDurationMs": "60000",
                    "audioTrack": {"id": "en.0", "displayName": "English", "audioIsDefault": true}
                },
                {
                    "itag": 251,
                    "url": "https://media.example/otf.webm",
                    "mimeType": "audio/webm; codecs=\"opus\"",
                    "type": "FORMAT_STREAM_TYPE_OTF"
                },
                {
                    "itag": 333,
                    "url": "https://media.example/drm.mp4",
                    "mimeType": "video/mp4; codecs=\"avc1.640028\"",
                    "drmFamilies": {"WIDEVINE": [{}]}
                },
                {
                    "itag": 999,
                    "url": "https://media.example/live.mp4",
                    "mimeType": "video/mp4; codecs=\"avc1.640028\"",
                    "targetDurationSec": 5
                }
            ]
        }
    });
    let (formats, todos, challenges) =
        crate::native::youtube_formats_and_todos(&[response], Some(100), Some("not_live"));
    assert!(challenges.is_empty());
    // Duplicate itag 18, OTF, and live-adaptive entries are skipped; DRM is
    // exposed with a notice.
    assert_eq!(formats.len(), 6);
    assert!(todos.iter().any(|todo| todo.contains("DRM-protected")));

    let by_id = |id: &str| {
        formats
            .iter()
            .find(|format| format.get("format_id").and_then(serde_json::Value::as_str) == Some(id))
            .unwrap()
    };
    let twenty_two = by_id("22");
    assert_eq!(
        twenty_two.get("source_preference"),
        Some(&serde_json::json!(-5))
    );
    assert_eq!(twenty_two.get("quality"), Some(&serde_json::json!(6)));
    assert_eq!(twenty_two.get("fps"), Some(&serde_json::json!(30)));

    let seventeen = by_id("17");
    assert_eq!(seventeen.get("quality"), Some(&serde_json::json!(0)));
    assert_eq!(seventeen.get("preference"), Some(&serde_json::json!(-2)));
    assert!(seventeen.get("fps").is_none());

    let damaged = by_id("18");
    assert_eq!(damaged.get("preference"), Some(&serde_json::json!(-10)));
    assert!(
        damaged
            .get("format_note")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|note| note.contains("DAMAGED"))
    );

    let premium = by_id("616");
    assert_eq!(
        premium.get("source_preference"),
        Some(&serde_json::json!(99))
    );
    assert_eq!(
        premium.get("container"),
        Some(&serde_json::json!("mp4_dash"))
    );

    let audio = by_id("140");
    assert_eq!(audio.get("language"), Some(&serde_json::json!("en")));
    assert_eq!(
        audio.get("language_preference"),
        Some(&serde_json::json!(5))
    );
    assert_eq!(
        audio.get("format_note"),
        Some(&serde_json::json!("English (default), medium"))
    );
    assert_eq!(
        audio.get("filesize_approx"),
        Some(&serde_json::json!(960000))
    );
}

#[test]
fn youtube_no_formats_errors_match_python_branches() {
    let error = crate::native::youtube_no_formats_error(
        YOUTUBE_FIXTURE_ID,
        &[serde_json::json!({"streamingData": {"licenseInfos": [{}]}})],
        &[],
    );
    assert_eq!(error.to_string(), "This video is DRM protected");

    let error = crate::native::youtube_no_formats_error(
        YOUTUBE_FIXTURE_ID,
        &[
            serde_json::json!({"playabilityStatus": {"status": "LOGIN_REQUIRED", "reason": "Please sign in to view this video. This helps protect our community. Learn more"}}),
        ],
        &[],
    );
    assert!(error.to_string().contains("Login with cookies"));

    let error = crate::native::youtube_no_formats_error(
        YOUTUBE_FIXTURE_ID,
        &[
            serde_json::json!({"playabilityStatus": {"errorScreen": {"playerErrorMessageRenderer": {"reason": {"simpleText": "Video unavailable"}, "subreason": {"simpleText": "The uploader has not made this video available in your country."}}}}}),
        ],
        &[],
    );
    assert!(error.to_string().contains(
        "Video unavailable. The uploader has not made this video available in your country."
    ));

    let error = crate::native::youtube_no_formats_error(
        YOUTUBE_FIXTURE_ID,
        &[serde_json::json!({"playabilityStatus": {"status": "ERROR", "reason": "Bad"}})],
        &[],
    );
    assert!(
        !error
            .to_string()
            .contains("YouTube returned no downloadable formats")
    );
    assert!(error.to_string().contains("Bad"));

    let error =
        crate::native::youtube_no_formats_error(YOUTUBE_FIXTURE_ID, &[serde_json::json!({})], &[]);
    assert!(
        error
            .to_string()
            .contains("YouTube returned no downloadable formats")
    );
}

#[test]
fn youtube_solver_scripts_match_pinned_hashes() {
    // Empty-string vector for the dependency-free SHA3-512, then the three
    // vendored EJS scripts (this also proves the `include_str!` paths).
    assert_eq!(
        crate::native::sha3_512_hex(b""),
        "a69f73cca23a9ac5c8b567dc185a756e97c982164fe25859e0d1dcc1475c80a615b2123af1f5f94c11e3e9402c3ac558f500199d95b6d3e301758586281dcd26",
    );
    assert_eq!(
        crate::native::sha3_512_hex(b"abc"),
        "b751850b1a57168a5693cd924b6b096e08f621827444f70d884f5d0240d2712e10e116e9192af3c91a7ec57647e3934057340b4cf408d5a56592f8274eec53f0",
    );
    assert!(crate::native::youtube_verify_solver_scripts().is_ok());
}

#[test]
fn youtube_sig_spec_application_matches_python_solve_sig() {
    // `''.join(s[i] for i in spec)` with the spec as ordinal characters.
    let mut formats = vec![serde_json::json!({
        "format_id": "140",
        "url": "https://media.example/a.m4a?sig=abcdef",
    })];
    let challenges = crate::native::YoutubeChallenges {
        sig: vec![crate::native::YoutubeSigChallenge {
            format_index: 0,
            encrypted: "abcdef".to_owned(),
            param: "sig".to_owned(),
        }],
        n: vec![],
    };
    let mut solutions = crate::native::YoutubeSolutions::default();
    solutions
        .sig_specs
        .insert(6, "\u{2}\u{1}\u{0}".to_owned());
    let (sig_done, n_done) =
        crate::native::youtube_apply_solutions(&mut formats, &challenges, &solutions);
    assert!(sig_done && n_done);
    assert_eq!(
        formats[0].get("url").and_then(serde_json::Value::as_str),
        Some("https://media.example/a.m4a?sig=cba")
    );
    assert!(formats[0].get("rust_todo").is_none());

    // Out-of-range spec ordinals leave the format unsolved.
    let mut formats = vec![serde_json::json!({
        "format_id": "140",
        "url": "https://media.example/a.m4a?sig=abcdef",
        "rust_todo": "TODO: YouTube signatureCipher requires the native player-JavaScript solver",
    })];
    let mut solutions = crate::native::YoutubeSolutions::default();
    solutions.sig_specs.insert(6, "\u{FFFF}".to_owned());
    let (sig_done, _) =
        crate::native::youtube_apply_solutions(&mut formats, &challenges, &solutions);
    assert!(!sig_done);
    assert!(formats[0].get("rust_todo").is_some());
}

#[test]
fn youtube_solver_stdin_lists_n_before_sig() {
    let dummy = crate::native::youtube_sig_dummy(3);
    assert_eq!(dummy.chars().count(), 3);
    let stdin = crate::native::youtube_solver_stdin(
        "player-code",
        "const lib = {};",
        "function jsc(input) { return input; }",
        &[dummy],
        &["challenge-n".to_owned()],
    );
    assert!(stdin.contains("Object.assign(globalThis, lib);"));
    assert!(stdin.contains("\"type\":\"player\""));
    assert!(stdin.contains("\"output_preprocessed\":true"));
    let n_at = stdin.find("\"type\":\"n\"").unwrap();
    let sig_at = stdin.find("\"type\":\"sig\"").unwrap();
    assert!(n_at < sig_at);
}

#[test]
fn youtube_solver_output_parsing_matches_bulk_solve_contract() {
    let dummies = vec![crate::native::youtube_sig_dummy(2)];
    let stdout = serde_json::json!({
        "responses": [
            {"data": {"challenge-n": "solved-n"}},
            {"data": {dummies[0].clone(): "BA"}},
        ]
    })
    .to_string();
    let solutions = crate::native::youtube_parse_solver_output(
        &stdout,
        &dummies,
        &["challenge-n".to_owned()],
    )
    .unwrap();
    assert_eq!(
        solutions.n_results.get("challenge-n").map(String::as_str),
        Some("solved-n")
    );
    assert_eq!(solutions.sig_specs.get(&2).map(String::as_str), Some("BA"));

    let error = crate::native::youtube_parse_solver_output(
        &serde_json::json!({"type": "error", "error": "boom"}).to_string(),
        &[],
        &[],
    );
    assert!(error.is_err());

    let mismatch = crate::native::youtube_parse_solver_output(
        &serde_json::json!({"responses": []}).to_string(),
        &dummies,
        &[],
    );
    assert!(mismatch.is_err());
}

#[test]
fn youtube_solver_output_parsing_preserves_partial_failures() {
    let dummies = vec![crate::native::youtube_sig_dummy(2)];
    let stdout = serde_json::json!({
        "responses": [
            {"data": {"challenge": "solved-challenge"}},
            {"type": "error", "error": "boom"},
        ]
    })
    .to_string();
    let solutions =
        crate::native::youtube_parse_solver_output(&stdout, &dummies, &["challenge".to_owned()])
            .unwrap();
    assert_eq!(
        solutions.n_results.get("challenge").map(String::as_str),
        Some("solved-challenge")
    );
    assert!(solutions.sig_specs.is_empty());
    assert_eq!(
        solutions.errors,
        vec!["solver sig challenge failed: boom".to_owned()]
    );

    let stdout = serde_json::json!({
        "responses": [
            {"data": {"challenge": "solved-challenge"}},
            {"type": "result"},
        ]
    })
    .to_string();
    let solutions =
        crate::native::youtube_parse_solver_output(&stdout, &dummies, &["challenge".to_owned()])
            .unwrap();
    assert_eq!(solutions.errors.len(), 1);
    assert!(solutions.errors[0].contains("no result data object"));

    let challenges = crate::native::YoutubeChallenges {
        sig: vec![crate::native::YoutubeSigChallenge {
            format_index: 0,
            encrypted: "abcdef".to_owned(),
            param: "sig".to_owned(),
        }],
        n: vec![crate::native::YoutubeNChallenge {
            format_index: 1,
            value: "challenge".to_owned(),
            in_path: false,
        }],
    };
    let mut formats = vec![
        serde_json::json!({
            "format_id": "140",
            "url": "https://media.example/a.m4a?sig=abcdef",
        }),
        serde_json::json!({
            "format_id": "251",
            "url": "https://media.example/a.webm?n=challenge",
        }),
    ];
    let (sig_done, n_done) =
        crate::native::youtube_apply_solutions(&mut formats, &challenges, &solutions);
    assert!(!sig_done && n_done);
    assert_eq!(
        formats[1].get("url").and_then(serde_json::Value::as_str),
        Some("https://media.example/a.webm?n=solved-challenge")
    );
}

#[test]
fn youtube_solver_runtime_selection_prefers_first_usable_provider() {
    use yt_dlp_javascript::{JavascriptRuntime, RuntimeInfo, RuntimeKind};
    let calls = std::cell::RefCell::new(Vec::new());
    let selected = crate::native::youtube_select_solver_runtime_with(|kind| {
        calls.borrow_mut().push(kind);
        match kind {
            RuntimeKind::Deno | RuntimeKind::Node | RuntimeKind::QuickJs => None,
            RuntimeKind::Bun => Some((
                JavascriptRuntime::from_info(RuntimeInfo {
                    kind,
                    name: "bun".to_owned(),
                    path: std::path::PathBuf::from("bun"),
                    version: "1.2.11".to_owned(),
                    version_tuple: vec![1, 2, 11],
                    supported: true,
                })
                .unwrap(),
                "bun-lib",
            )),
        }
    })
    .unwrap();
    assert_eq!(selected.0.info().kind, RuntimeKind::Bun);
    assert_eq!(
        *calls.borrow(),
        vec![
            RuntimeKind::Deno,
            RuntimeKind::Node,
            RuntimeKind::QuickJs,
            RuntimeKind::Bun
        ]
    );
}

#[test]
fn youtube_solver_stderr_filtering_matches_provider_contract() {
    use yt_dlp_javascript::RuntimeKind;
    assert_eq!(
        crate::native::youtube_clean_solver_stderr(
            RuntimeKind::Deno,
            "\u{1b}[1mDownload https://example.test/lib.js\u{1b}[0m\nDANGER: TLS certificate validation is disabled for all hostnames\nboom",
        )
        .as_str(),
        "boom"
    );
    assert_eq!(
        crate::native::youtube_clean_solver_stderr(
            RuntimeKind::Node,
            "[stdin]:1\nvar jsc = 1;\n(Use `node --trace-uncaught ...` to show where the exception was thrown)\nNode.js v26.7.0\nboom",
        )
        .as_str(),
        "boom"
    );
    assert_eq!(
        crate::native::youtube_clean_solver_stderr(
            RuntimeKind::Bun,
            "Bun v1.2.11 (linux x64)\nboom",
        )
        .as_str(),
        "boom"
    );
    assert_eq!(
        crate::native::youtube_clean_solver_stderr(RuntimeKind::QuickJs, "boom").as_str(),
        "boom"
    );
    assert!(
        crate::native::youtube_clean_solver_stderr(RuntimeKind::Node, "Node.js v26.7.0\n")
            .is_empty()
    );
}

#[test]
fn youtube_solver_library_availability_matches_builtin_chain() {
    use yt_dlp_javascript::RuntimeKind;
    assert!(crate::native::youtube_solver_library(RuntimeKind::Deno).is_some());
    assert!(crate::native::youtube_solver_library(RuntimeKind::Bun).is_some());
    // No vendored lib script exists for Node or QuickJS, mirroring the
    // builtin source fallback chain (pypackage/cache/web only).
    assert!(crate::native::youtube_solver_library(RuntimeKind::Node).is_none());
    assert!(crate::native::youtube_solver_library(RuntimeKind::QuickJs).is_none());
}

#[test]
fn youtube_node_solver_flags_match_provider_permissions() {
    use yt_dlp_javascript::{JavascriptRuntime, RuntimeInfo, RuntimeKind};
    let modern = JavascriptRuntime::from_info(RuntimeInfo {
        kind: RuntimeKind::Node,
        name: "node".to_owned(),
        path: std::path::PathBuf::from("node"),
        version: "26.0.0".to_owned(),
        version_tuple: vec![26, 0, 0],
        supported: true,
    })
    .unwrap();
    assert_eq!(
        crate::native::youtube_node_extra_args(&modern),
        vec!["--permission".to_owned()]
    );
    let legacy = JavascriptRuntime::from_info(RuntimeInfo {
        kind: RuntimeKind::Node,
        name: "node".to_owned(),
        path: std::path::PathBuf::from("node"),
        version: "22.0.0".to_owned(),
        version_tuple: vec![22, 0, 0],
        supported: true,
    })
    .unwrap();
    assert_eq!(
        crate::native::youtube_node_extra_args(&legacy),
        vec![
            "--experimental-permission".to_owned(),
            "--no-warnings=ExperimentalWarning".to_owned()
        ]
    );
}

const YOUTUBE_STUB_SOLVER_CORE: &str = r#"function jsc(input) {
  const responses = [];
  for (const request of input.requests) {
    const data = {};
    for (const challenge of request.challenges) {
      data[challenge] = request.type === "n" ? "solved-" + challenge : "\u0002\u0001\u0000";
    }
    responses.push({data: data});
  }
  return {responses: responses};
}"#;

#[test]
fn youtube_challenge_solving_rewrites_format_urls_end_to_end() {
    let Some(runtime) =
        yt_dlp_javascript::JavascriptRuntime::probe(yt_dlp_javascript::RuntimeKind::Node, None)
            .ok()
            .flatten()
            .filter(|runtime| runtime.info().supported)
    else {
        return;
    };
    let challenges = crate::native::YoutubeChallenges {
        sig: vec![crate::native::YoutubeSigChallenge {
            format_index: 0,
            encrypted: "abcdef".to_owned(),
            param: "sig".to_owned(),
        }],
        n: vec![
            crate::native::YoutubeNChallenge {
                format_index: 1,
                value: "challenge".to_owned(),
                in_path: false,
            },
            crate::native::YoutubeNChallenge {
                format_index: 2,
                value: "pathchallenge".to_owned(),
                in_path: true,
            },
        ],
    };
    let solutions = crate::native::youtube_bulk_solve_with(
        "ignored-player",
        &challenges,
        &runtime,
        "const lib = {};",
        YOUTUBE_STUB_SOLVER_CORE,
    )
    .unwrap();
    let mut formats = vec![
        serde_json::json!({
            "format_id": "140",
            "url": "https://media.example/a.m4a?sig=abcdef",
            "rust_todo": "TODO: YouTube signatureCipher requires the native player-JavaScript solver",
        }),
        serde_json::json!({
            "format_id": "251",
            "url": "https://media.example/a.webm?n=challenge",
            "rust_todo": "TODO: YouTube n challenge requires the native player-JavaScript solver",
        }),
        serde_json::json!({
            "format_id": "hls",
            "url": "https://media.example/videoplayback/n/pathchallenge/extra.m3u8",
        }),
    ];
    let (sig_done, n_done) =
        crate::native::youtube_apply_solutions(&mut formats, &challenges, &solutions);
    assert!(sig_done && n_done);
    assert_eq!(
        formats[0].get("url").and_then(serde_json::Value::as_str),
        Some("https://media.example/a.m4a?sig=cba")
    );
    assert_eq!(
        formats[1].get("url").and_then(serde_json::Value::as_str),
        Some("https://media.example/a.webm?n=solved-challenge")
    );
    assert_eq!(
        formats[2].get("url").and_then(serde_json::Value::as_str),
        Some("https://media.example/videoplayback/n/solved-pathchallenge/extra.m3u8")
    );
    assert!(formats.iter().all(|format| format.get("rust_todo").is_none()));
}

#[test]
fn youtube_music_description_parsing_matches_python_backtracking() {
    // Bare `℗ YEAR` footers yield no release year in Python either: the
    // trailing `.+` consumes the line during backtracking.
    let mut info = yt_dlp_core::InfoDict::new();
    crate::native::youtube_music_metadata(
        &mut info,
        "Track Title · Artist Name\nAlbum Name\n℗ 2024\nAuto-generated by YouTube.",
    );
    assert_eq!(info.get_str("track"), Some("Track Title"));
    assert_eq!(info.get_str("album"), Some("Album Name"));
    assert_eq!(info.get_i64("release_year"), None);

    // An extra content line lets the `℗` group participate, as in Python.
    let mut info = yt_dlp_core::InfoDict::new();
    crate::native::youtube_music_metadata(
        &mut info,
        "Track Title · Artist Name\nAlbum Name\n℗ 2024\n\nReleased on: 2024-03-01\nAuto-generated by YouTube.",
    );
    assert_eq!(info.get_i64("release_year"), Some(2024));
    assert_eq!(
        info.get("artists").and_then(serde_json::Value::as_array),
        Some(&vec![serde_json::json!("Artist Name")])
    );

    // Non-music descriptions are left untouched.
    let mut info = yt_dlp_core::InfoDict::new();
    crate::native::youtube_music_metadata(&mut info, "Just a regular description.");
    assert!(info.get("track").is_none());
}

#[test]
fn youtube_trailer_response_redirects_to_trailer_video() {
    let extractor = YoutubeExtractor::new(ExtractorDescriptor::new(
        "YoutubeIE",
        "youtube",
        r"https?://(?:www\.)?youtube\.com/.*",
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            &youtube_page_context_for(serde_json::json!({
                "playabilityStatus": {
                    "status": "UNPLAYABLE",
                    "errorScreen": {
                        "playerLegacyDesktopYpcTrailerRenderer": {
                            "trailerVideoId": "a1b2c3d4e5f"
                        }
                    }
                },
                "videoDetails": {"videoId": YOUTUBE_FIXTURE_ID, "title": "Trailer"}
            })),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_str("_type"), Some("url"));
    assert_eq!(
        result.get_str("url"),
        Some("https://www.youtube.com/watch?v=a1b2c3d4e5f")
    );
    assert_eq!(result.get_str("ie_key"), Some("Youtube"));
}

#[test]
fn youtube_metadata_parity_covers_shorts_age_stretch_music_and_clips() {
    let extractor = YoutubeExtractor::new(ExtractorDescriptor::new(
        "YoutubeIE",
        "youtube",
        r"https?://(?:www\.)?youtube\.com/.*",
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=90",
            &youtube_page_context_for(serde_json::json!({
                "playabilityStatus": {"status": "OK"},
                "videoDetails": {
                    "videoId": YOUTUBE_FIXTURE_ID,
                    "title": "Shorts fixture",
                    "shortDescription": "Track Title · Artist Name\nAlbum Name\n℗ 2024\n\nReleased on: 2024-03-01\nAuto-generated by YouTube.",
                    "lengthSeconds": "30",
                    "author": "Rust Channel",
                    "channelId": "UCnativefixture",
                    "viewCount": "777",
                    "isLive": false,
                    "isLiveContent": false,
                    "keywords": ["yt:stretch=16:9"],
                    "thumbnail": {
                        "thumbnails": [
                            {"url": "https://i.ytimg.com/vi/dQw4w9WgXcQ/default.jpg", "width": 120, "height": 90}
                        ]
                    }
                },
                "microformat": {
                    "playerMicroformatRenderer": {
                        "publishDate": "2026-08-31",
                        "category": "Music",
                        "ownerChannelName": "Rust Channel",
                        "isFamilySafe": false,
                        "isShortsEligible": true
                    }
                },
                "captions": {
                    "playerCaptionsTracklistRenderer": {
                        "captionTracks": [
                            {
                                "baseUrl": "https://www.youtube.com/api/timedtext?v=dQw4w9WgXcQ&lang=en&xosf=1",
                                "languageCode": "en",
                                "name": {"simpleText": "English"}
                            }
                        ]
                    }
                },
                "streamingData": {
                    "formats": [
                        {
                            "itag": 18,
                            "url": "https://rr1---sn.example.googlevideo.com/videoplayback?itag=18&clen=123456",
                            "mimeType": "video/mp4; codecs=\"avc1.42001E, mp4a.40.2\"",
                            "quality": "medium",
                            "qualityLabel": "360p",
                            "width": 640,
                            "height": 360,
                            "bitrate": 600000,
                            "contentLength": "123456"
                        }
                    ]
                }
            })),
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("media_type"), Some("short"));
    assert_eq!(result.get_i64("age_limit"), Some(18));
    assert_eq!(result.get_i64("view_count"), Some(777));
    assert_eq!(result.get_f64("start_time"), Some(90.0));
    assert_eq!(result.get_str("track"), Some("Track Title"));
    assert_eq!(result.get_str("album"), Some("Album Name"));
    assert_eq!(result.get_str("release_year"), None);
    assert_eq!(result.get_i64("release_year"), Some(2024));
    let artists = result
        .get("artists")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(artists, &vec![serde_json::json!("Artist Name")]);
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 1);
    let ratio = formats[0]
        .get("stretched_ratio")
        .and_then(serde_json::Value::as_f64)
        .unwrap();
    assert!((ratio - 16.0 / 9.0).abs() < 1e-9);
    // One original plus the 38 synthesized candidates, minus the
    // `default.jpg` twin that deduplicates against the original.
    assert_eq!(
        result
            .get("thumbnails")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(38)
    );
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://i.ytimg.com/vi/dQw4w9WgXcQ/default.jpg")
    );
    let subs = result
        .get("subtitles")
        .and_then(|subs| subs.get("en"))
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(subs.len(), 7);
    assert!(subs.iter().all(|entry| {
        entry
            .get("impersonate")
            .and_then(serde_json::Value::as_bool)
            == Some(true)
            && entry
                .get("url")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|url| !url.contains("xosf"))
    }));
}

#[test]
fn youtube_pot_config_parsing_matches_python_selection() {
    use crate::native::{parse_config_po_token, PoTokenContext};

    let outcome = parse_config_po_token(&["web.gvs+QUJD"], "web", PoTokenContext::Gvs);
    assert_eq!(outcome.token.as_deref(), Some("QUJD"));
    assert!(outcome.warnings.is_empty());

    // Client and context match case-insensitively.
    let outcome = parse_config_po_token(&["WEB.GVS+QUJD"], "web", PoTokenContext::Gvs);
    assert_eq!(outcome.token.as_deref(), Some("QUJD"));

    // A bare `CLIENT+TOKEN` entry assumes the GVS context.
    let outcome = parse_config_po_token(&["web+QUJD"], "web", PoTokenContext::Gvs);
    assert_eq!(outcome.token.as_deref(), Some("QUJD"));
    let outcome = parse_config_po_token(&["web+QUJD"], "web", PoTokenContext::Player);
    assert_eq!(outcome.token, None);

    // Other clients, other contexts, and empty lists never match.
    assert_eq!(
        parse_config_po_token(&["web.player+QUJD"], "web", PoTokenContext::Gvs).token,
        None
    );
    assert_eq!(
        parse_config_po_token(&["android.gvs+QUJD"], "web", PoTokenContext::Gvs).token,
        None
    );
    assert_eq!(
        parse_config_po_token(&[], "web", PoTokenContext::Gvs).token,
        None
    );

    // The first matching entry wins, even when it canonicalizes padding.
    let outcome = parse_config_po_token(
        &["web.gvs+QUJD", "web.gvs+QUJDRA=="],
        "web",
        PoTokenContext::Gvs,
    );
    assert_eq!(outcome.token.as_deref(), Some("QUJD"));
    let outcome = parse_config_po_token(&["web.player+QUJDRA=="], "web", PoTokenContext::Player);
    assert_eq!(outcome.token.as_deref(), Some("QUJDRA=="));
    let outcome = parse_config_po_token(&["web.subs+QUJD"], "web", PoTokenContext::Subs);
    assert_eq!(outcome.token.as_deref(), Some("QUJD"));

    // Entries without `+` warn with the exact Python message and are skipped.
    let outcome = parse_config_po_token(&["web.gvsQUJD"], "web", PoTokenContext::Gvs);
    assert_eq!(outcome.token, None);
    assert_eq!(
        outcome.warnings,
        vec![
            "Invalid po_token configuration format. Expected \"CLIENT.CONTEXT+PO_TOKEN\", got \"web.gvsQUJD\""
                .to_owned()
        ]
    );

    // An entry with no decodable content canonicalizes to the empty token,
    // mirroring Python (callers treat it as absent via truthiness).
    let outcome = parse_config_po_token(&["web.gvs+!!!"], "web", PoTokenContext::Gvs);
    assert_eq!(outcome.token.as_deref(), Some(""));
    assert!(outcome.warnings.is_empty());
}

#[test]
fn youtube_pot_token_canonicalization_matches_python_base64() {
    use crate::native::{parse_config_po_token, PoTokenContext};

    // (raw token, expected canonical form or None when Python warns).
    let cases = vec![
        ("QUJD", Some("QUJD")),
        ("QUJDRA==", Some("QUJDRA==")),
        ("QUJD%52A%3D%3D", Some("QUJDRA==")),
        ("QUJDRA%3D%3D&next=123", None),
        ("ABC", None),
        ("A", None),
        ("", Some("")),
        // `unquote` keeps `+`, which still decodes (62) and re-encodes as `-`.
        ("QUJD+RA==", Some("QUJD-RA=")),
        ("QUJDRA", None),
        ("QUI", None),
        (" Q U J D \n", Some("QUJD")),
        ("QUJD\n", Some("QUJD")),
        ("QU=JD", Some("QUJD")),
        ("====", Some("")),
        ("QUJD===", Some("QUJD")),
        ("ab-cd_ef12", None),
        ("QUJDRA=", None),
        ("Q=UJD", Some("QUJD")),
        ("QQ==QQ==", Some("QQQQ")),
        ("QUJDQUJD", Some("QUJDQUJD")),
        ("QUJD====QUJD", Some("QUJDQUJD")),
        ("=QUJD", Some("QUJD")),
        ("QUJD=====", Some("QUJD")),
        ("QUJ=", Some("QUI=")),
        ("QU==JD", Some("QUJD")),
        ("QUJD=QUJD", Some("QUJDQUJD")),
        ("QUJDRA==QQ", Some("QUJDRAQQ")),
        ("QUJDQQ==", Some("QUJDQQ==")),
        ("QUJDQQ=", None),
        ("QUJD==QU", None),
        ("QUJD====QQ", None),
        ("QU JD", Some("QUJD")),
        ("Q U J", None),
        ("QUJDRA== ", Some("QUJDRA==")),
        ("QUJD QQ==", Some("QUJDQQ==")),
        ("Q==J=D", None),
        ("QU=JDRA==", Some("QUJDRA==")),
        ("=", Some("")),
        ("QUJDRA==QQ==", Some("QUJDRAQQ")),
        ("QU=J", None),
        ("Q=UJ=", Some("QUI=")),
        ("%FF", None),
        ("%C3%A9", None),
        ("%41", None),
        ("A%42C", None),
        ("%2D%5F", None),
        ("+", None),
        ("%20", Some("")),
        ("QUJD%00", Some("QUJD")),
    ];
    for (raw, expected) in cases {
        let entry = format!("web.gvs+{raw}");
        let outcome = parse_config_po_token(&[&entry], "web", PoTokenContext::Gvs);
        assert_eq!(outcome.token.as_deref(), expected, "token {raw:?}");
        assert_eq!(
            outcome.warnings.is_empty(),
            expected.is_some(),
            "token {raw:?}"
        );
    }
}

#[test]
fn youtube_pot_fetch_policy_matches_python_branches() {
    use crate::native::FetchPotPolicy;

    assert!(!FetchPotPolicy::parse("never").should_fetch(true));
    assert!(!FetchPotPolicy::parse("never").should_fetch(false));
    assert!(FetchPotPolicy::parse("always").should_fetch(false));
    assert!(FetchPotPolicy::parse("auto").should_fetch(true));
    assert!(!FetchPotPolicy::parse("auto").should_fetch(false));
    // Unknown values fall back to `auto`, mirroring `_fetch_po_token`.
    assert!(!FetchPotPolicy::parse("").should_fetch(false));
    assert!(!FetchPotPolicy::parse("sometimes").should_fetch(false));
    assert!(FetchPotPolicy::parse("sometimes").should_fetch(true));
}

#[test]
fn youtube_pot_visitor_id_matches_python_helper() {
    use crate::native::extract_visitor_id;

    assert_eq!(
        extract_visitor_id("CgtXdHlJc2x5RS1RZw%3D%3D").as_deref(),
        Some("WtyIslyE-Qg")
    );
    assert_eq!(
        extract_visitor_id("CgtXdHlJc2x5RS1RZw==").as_deref(),
        Some("WtyIslyE-Qg")
    );
    assert_eq!(extract_visitor_id("short"), None);
    assert_eq!(extract_visitor_id(""), None);
    assert_eq!(extract_visitor_id("!!!"), None);
    assert_eq!(extract_visitor_id("QUJD"), None);
}

#[test]
fn youtube_pot_content_binding_matches_python_rules() {
    use crate::native::{webpo_content_binding, PoTokenContentBindingType, PoTokenContext};

    const VISITOR: &str = "CgtXdHlJc2x5RS1RZw%3D%3D";
    const VIDEO: &str = "dQw4w9WgXcQ";
    let binding = |client: &str,
                   context: PoTokenContext,
                   auth: bool,
                   visitor: Option<&str>,
                   dsync: Option<&str>,
                   video: Option<&str>,
                   bind_video: bool,
                   bind_visitor: bool| {
        webpo_content_binding(
            Some(client),
            context,
            auth,
            visitor,
            dsync,
            video,
            bind_video,
            bind_visitor,
        )
    };

    assert_eq!(
        binding(
            "WEB",
            PoTokenContext::Gvs,
            false,
            Some(VISITOR),
            Some("DSYNC123"),
            Some(VIDEO),
            false,
            false
        ),
        Some((VISITOR.to_owned(), PoTokenContentBindingType::VisitorData))
    );
    assert_eq!(
        binding(
            "WEB",
            PoTokenContext::Gvs,
            true,
            Some(VISITOR),
            Some("DSYNC123"),
            Some(VIDEO),
            false,
            false
        ),
        Some(("DSYNC123".to_owned(), PoTokenContentBindingType::DatasyncId))
    );
    // Non-WebPO clients and unknown clients have no binding.
    assert_eq!(
        binding(
            "ANDROID",
            PoTokenContext::Gvs,
            false,
            Some(VISITOR),
            Some("DSYNC123"),
            Some(VIDEO),
            false,
            false
        ),
        None
    );
    assert_eq!(
        binding(
            "NOPE",
            PoTokenContext::Gvs,
            false,
            Some(VISITOR),
            Some("DSYNC123"),
            Some(VIDEO),
            false,
            false
        ),
        None
    );
    // Player and subs contexts bind to the video ID.
    assert_eq!(
        binding(
            "WEB",
            PoTokenContext::Player,
            false,
            Some(VISITOR),
            Some("DSYNC123"),
            Some(VIDEO),
            false,
            false
        ),
        Some((VIDEO.to_owned(), PoTokenContentBindingType::VideoId))
    );
    assert_eq!(
        binding(
            "WEB",
            PoTokenContext::Subs,
            false,
            Some(VISITOR),
            Some("DSYNC123"),
            Some(VIDEO),
            false,
            false
        ),
        Some((VIDEO.to_owned(), PoTokenContentBindingType::VideoId))
    );
    // The GVS-to-video-ID experiment overrides the visitor binding.
    assert_eq!(
        binding(
            "WEB",
            PoTokenContext::Gvs,
            false,
            Some(VISITOR),
            Some("DSYNC123"),
            Some(VIDEO),
            true,
            false
        ),
        Some((VIDEO.to_owned(), PoTokenContentBindingType::VideoId))
    );
    // WEB_REMIX GVS tokens bind like web even though the context is GVS.
    assert_eq!(
        binding(
            "WEB_REMIX",
            PoTokenContext::Gvs,
            false,
            Some(VISITOR),
            Some("DSYNC123"),
            Some(VIDEO),
            false,
            false
        ),
        Some((VISITOR.to_owned(), PoTokenContentBindingType::VisitorData))
    );
    // Opting into visitor-ID binding resolves the embedded visitor ID.
    assert_eq!(
        binding(
            "WEB",
            PoTokenContext::Gvs,
            false,
            Some(VISITOR),
            Some("DSYNC123"),
            Some(VIDEO),
            false,
            true
        ),
        Some((
            "WtyIslyE-Qg".to_owned(),
            PoTokenContentBindingType::VisitorId
        ))
    );
    // Missing bound values yield no binding, which the cache layer treats
    // the same as Python's `(None, type)` (no cache spec either way).
    assert_eq!(
        binding(
            "WEB",
            PoTokenContext::Gvs,
            false,
            None,
            Some("DSYNC123"),
            Some(VIDEO),
            false,
            false
        ),
        None
    );
    assert_eq!(
        binding(
            "WEB",
            PoTokenContext::Gvs,
            true,
            Some(VISITOR),
            None,
            Some(VIDEO),
            false,
            false
        ),
        None
    );
    assert_eq!(
        binding(
            "WEB",
            PoTokenContext::Player,
            false,
            Some(VISITOR),
            Some("DSYNC123"),
            None,
            false,
            false
        ),
        None
    );
}

#[test]
fn youtube_pot_cache_key_matches_python_derivation() {
    use crate::native::pot_cache_key;

    // Exact hash from the Python oracle for these bindings.
    let bindings = vec![
        ("t".to_owned(), Some("webpo".to_owned())),
        ("cb".to_owned(), Some("CgtXdHlJc2x5RS1RZw%3D%3D".to_owned())),
        ("cbt".to_owned(), Some("visitor_data".to_owned())),
        ("ip".to_owned(), Some("142.250.0.1".to_owned())),
        ("sa".to_owned(), None),
        ("px".to_owned(), None),
    ];
    assert_eq!(
        pot_cache_key("webpo", &bindings),
        "3bff3042003c90e4bd31a5c99e66991c9546f7df5e1f84cb24e17b5e6bda6e92"
    );
    // Binding order does not matter; the provider key and values do.
    let mut shuffled = bindings.clone();
    shuffled.reverse();
    assert_eq!(
        pot_cache_key("webpo", &shuffled),
        pot_cache_key("webpo", &bindings)
    );
    assert_ne!(
        pot_cache_key("other", &bindings),
        pot_cache_key("webpo", &bindings)
    );
}

#[test]
fn youtube_pot_webpo_cache_spec_matches_python_provider() {
    use crate::native::{
        webpo_cache_spec, PoTokenCacheWritePolicy, PoTokenContext, PoTokenRequest,
    };

    let request = PoTokenRequest {
        context: PoTokenContext::Gvs,
        client_name: "WEB".to_owned(),
        visitor_data: Some("CgtXdHlJc2x5RS1RZw%3D%3D".to_owned()),
        data_sync_id: None,
        video_id: Some("dQw4w9WgXcQ".to_owned()),
        session_index: None,
        player_url: None,
        is_authenticated: false,
        gvs_bind_to_video_id: false,
        bypass_cache: false,
    };
    let spec = webpo_cache_spec(&request, Some("142.250.0.1"), None, None, false).expect("spec");
    assert_eq!(spec.default_ttl_secs, 21600);
    assert_eq!(spec.write_policy, PoTokenCacheWritePolicy::WriteAll);
    assert_eq!(
        spec.bindings,
        vec![
            ("t".to_owned(), Some("webpo".to_owned())),
            ("cb".to_owned(), Some("CgtXdHlJc2x5RS1RZw%3D%3D".to_owned())),
            ("cbt".to_owned(), Some("visitor_data".to_owned())),
            ("ip".to_owned(), Some("142.250.0.1".to_owned())),
            ("sa".to_owned(), None),
            ("px".to_owned(), None),
        ]
    );

    // Video-ID bindings use the first-provider write policy.
    let player = PoTokenRequest {
        context: PoTokenContext::Player,
        ..request.clone()
    };
    let spec = webpo_cache_spec(&player, None, None, None, false).expect("spec");
    assert_eq!(spec.write_policy, PoTokenCacheWritePolicy::WriteFirst);

    // Requests without a binding produce no spec.
    let android = PoTokenRequest {
        client_name: "ANDROID".to_owned(),
        ..request.clone()
    };
    assert!(webpo_cache_spec(&android, None, None, None, false).is_none());
}

#[test]
fn youtube_pot_memory_cache_matches_python_lru() {
    use crate::native::MemoryPoTokenCache;

    let mut cache = MemoryPoTokenCache::new();
    assert_eq!(cache.get("missing", 1_000), None);

    // `expires_at == now` still counts as live, mirroring the `<` checks.
    cache.store("key", "token", 1_000, 1_000);
    assert_eq!(cache.get("key", 1_000), Some("token".to_owned()));
    assert_eq!(cache.get("key", 1_001), None);

    // Already-expired tokens are never stored.
    cache.store("stale", "token", 999, 1_000);
    assert_eq!(cache.get("stale", 1_000), None);

    // Reads refresh recency: re-reading `key0` makes `key1` the oldest entry,
    // so inserting `fresh` evicts `key1` while `key0` survives.
    for index in 0..25 {
        cache.store(&format!("key{index}"), "token", 2_000, 1_000);
    }
    assert_eq!(cache.len(), 25);
    assert_eq!(cache.get("key0", 1_000), Some("token".to_owned()));
    cache.store("fresh", "token", 2_000, 1_000);
    assert_eq!(cache.len(), 25);
    assert_eq!(cache.get("key0", 1_000), Some("token".to_owned()));
    assert_eq!(cache.get("key1", 1_000), None);

    cache.delete("key0");
    assert_eq!(cache.get("key0", 1_000), None);
    cache.delete("missing");
}

#[test]
fn youtube_pot_session_extraction_matches_python_traversal() {
    use crate::native::{youtube_data_sync_id, youtube_visitor_data};

    let ytcfg = serde_json::json!({
        "VISITOR_DATA": "from_ytcfg_top",
        "INNERTUBE_CONTEXT": {"client": {"visitorData": "from_innertube"}},
        "DATASYNC_ID": "dsync_top",
    });
    let player_response = serde_json::json!({
        "responseContext": {
            "visitorData": "from_pr_ctx",
            "mainAppWebResponseContext": {"datasyncId": "dsync_nested"},
        },
    });
    assert_eq!(
        youtube_visitor_data(&[ytcfg.clone(), player_response.clone()]).as_deref(),
        Some("from_ytcfg_top")
    );
    assert_eq!(
        youtube_visitor_data(&[serde_json::json!({}), player_response.clone()]).as_deref(),
        Some("from_pr_ctx")
    );
    assert_eq!(
        youtube_visitor_data(&[serde_json::json!({}), serde_json::json!({})]),
        None
    );
    // The innertube client path beats the response context of a later entry.
    assert_eq!(
        youtube_visitor_data(&[
            serde_json::json!({"INNERTUBE_CONTEXT": {"client": {"visitorData": "from_innertube"}}}),
            player_response.clone(),
        ])
        .as_deref(),
        Some("from_innertube")
    );
    assert_eq!(
        youtube_data_sync_id(&[ytcfg, player_response]).as_deref(),
        Some("dsync_top")
    );
    assert_eq!(
        youtube_data_sync_id(&[
            serde_json::json!({}),
            serde_json::json!({
                "responseContext": {
                    "mainAppWebResponseContext": {"datasyncId": "dsync_nested"},
                },
            }),
        ])
        .as_deref(),
        Some("dsync_nested")
    );
}

#[test]
fn youtube_player_payload_carries_service_integrity_token() {
    use crate::native::youtube_player_request_payload;

    let ytcfg = serde_json::json!({});
    let plain = youtube_player_request_payload(&ytcfg, "dQw4w9WgXcQ", None);
    assert_eq!(
        plain.get("videoId").and_then(serde_json::Value::as_str),
        Some("dQw4w9WgXcQ")
    );
    assert!(plain.get("serviceIntegrityDimensions").is_none());

    let guarded = youtube_player_request_payload(&ytcfg, "dQw4w9WgXcQ", Some("TOKEN123"));
    assert_eq!(
        guarded
            .get("serviceIntegrityDimensions")
            .and_then(|dimensions| dimensions.get("poToken"))
            .and_then(serde_json::Value::as_str),
        Some("TOKEN123")
    );
    // The token rides alongside the unchanged contract fields.
    assert_eq!(
        guarded.get("videoId").and_then(serde_json::Value::as_str),
        Some("dQw4w9WgXcQ")
    );
    assert_eq!(
        guarded
            .get("contentCheckOk")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
}
#[test]
fn youtube_extractor_args_parsing_matches_python_option() {
    use crate::ExtractorArgs;

    // (flag value, expected IE key, expected argument map).
    let cases = vec![
        (
            "youtube:po_token=web.gvs+QUJD",
            "youtube",
            vec![("po_token", vec!["web.gvs+QUJD"])],
        ),
        (
            "youtube:po_token=AAA;fetch_pot=always",
            "youtube",
            vec![("po_token", vec!["AAA"]), ("fetch_pot", vec!["always"])],
        ),
        (
            "YouTube:PO-Token=AAA;Visitor_Data=VVV",
            "youtube",
            vec![("po_token", vec!["AAA"]), ("visitor_data", vec!["VVV"])],
        ),
        (
            "youtube:po_token=AAA,BBB, CCC ",
            "youtube",
            vec![("po_token", vec!["AAA", "BBB", "CCC"])],
        ),
        (
            r"youtube:po_token=AA\,BB,CC",
            "youtube",
            vec![("po_token", vec!["AA,BB", "CC"])],
        ),
        (
            "youtube:player_skip",
            "youtube",
            vec![("player_skip", vec![""])],
        ),
        ("youtube:po_token=", "youtube", vec![("po_token", vec![""])]),
        (
            "youtube:po_token=AAA;;fetch_pot=never",
            "youtube",
            vec![
                ("po_token", vec!["AAA"]),
                ("", vec![""]),
                ("fetch_pot", vec!["never"]),
            ],
        ),
        (
            "YOUTUBE:po_token=AAA",
            "youtube",
            vec![("po_token", vec!["AAA"])],
        ),
        ("my-ie:key=1", "my-ie", vec![("key", vec!["1"])]),
        ("youtube:a=1;a=2", "youtube", vec![("a", vec!["2"])]),
        (
            "youtube:token=AB==CD",
            "youtube",
            vec![("token", vec!["AB==CD"])],
        ),
        (
            "youtube: po_token = AAA ; fetch_pot = always ",
            "youtube",
            vec![("po_token", vec!["AAA"]), ("fetch_pot", vec!["always"])],
        ),
    ];
    for (value, ie_key, expected) in cases {
        let (parsed_key, parsed) = ExtractorArgs::parse_cli_value(value)
            .unwrap_or_else(|error| panic!("flag value {value:?}: {error}"));
        assert_eq!(parsed_key, ie_key, "flag value {value:?}");
        let expected_map: std::collections::HashMap<String, Vec<String>> = expected
            .into_iter()
            .map(|(key, values)| {
                (
                    key.to_owned(),
                    values.into_iter().map(str::to_owned).collect(),
                )
            })
            .collect();
        assert_eq!(parsed, expected_map, "flag value {value:?}");
    }

    // Missing or invalid `IE_KEY:` prefixes error like optparse.
    for value in ["youtube", "", ":po_token=AAA", "you tube:po_token=AAA"] {
        let error = ExtractorArgs::parse_cli_value(value).expect_err(&format!("{value:?}"));
        assert_eq!(
            error,
            format!("wrong --extractor-args formatting; it should be IE_KEY:ARGS, not \"{value}\"")
        );
    }
}

#[test]
fn youtube_extractor_args_lookup_matches_configuration_arg() {
    use crate::{ExtractionContext, ExtractorArgs};

    let mut args = ExtractorArgs::new();
    // Missing keys yield empty lists before anything is stored.
    assert!(args
        .configuration_arg("Youtube", "po_token", true)
        .is_empty());
    let (ie_key, parsed) =
        ExtractorArgs::parse_cli_value("youtube:po_token=AAA;fetch_pot=Always").unwrap();
    args.insert_ie_args(ie_key, parsed);
    // The IE key matches case-insensitively; the arg key matches exactly.
    assert_eq!(
        args.configuration_arg("Youtube", "po_token", true),
        vec!["AAA".to_owned()]
    );
    assert_eq!(
        args.configuration_arg("YOUTUBE", "po_token", true),
        vec!["AAA".to_owned()]
    );
    assert!(args
        .configuration_arg("Youtube", "PO_TOKEN", true)
        .is_empty());
    assert!(args
        .configuration_arg("generic", "po_token", true)
        .is_empty());
    // Without casesense the values are lowercased.
    assert_eq!(
        args.configuration_arg("youtube", "fetch_pot", false),
        vec!["always".to_owned()]
    );
    assert_eq!(
        args.configuration_arg("youtube", "fetch_pot", true),
        vec!["Always".to_owned()]
    );
    // Repeating the flag replaces the whole per-IE map.
    let (ie_key, parsed) = ExtractorArgs::parse_cli_value("youtube:fetch_pot=never").unwrap();
    args.insert_ie_args(ie_key, parsed);
    assert!(args
        .configuration_arg("youtube", "po_token", true)
        .is_empty());
    assert_eq!(
        args.configuration_arg("youtube", "fetch_pot", true),
        vec!["never".to_owned()]
    );

    let context = ExtractionContext::native().with_extractor_args(args);
    assert_eq!(
        context.configuration_arg("Youtube", "fetch_pot", true),
        vec!["never".to_owned()]
    );
}

#[test]
fn youtube_configured_session_inputs_match_python_precedence() {
    use crate::native::{youtube_configured_player_po_token, youtube_configured_visitor_data};
    use crate::{ExtractionContext, ExtractorArgs};

    let candidates = vec![
        serde_json::json!({"VISITOR_DATA": "from_ytcfg"}),
        serde_json::json!({"responseContext": {"visitorData": "from_response"}}),
    ];
    let plain = ExtractionContext::native();
    assert_eq!(youtube_configured_player_po_token(&plain), None);
    assert_eq!(
        youtube_configured_visitor_data(&plain, &candidates).as_deref(),
        Some("from_ytcfg")
    );

    let mut args = ExtractorArgs::new();
    let (ie_key, parsed) = ExtractorArgs::parse_cli_value(
        "youtube:po_token=web.gvs+QUJD,web.player+QUJDRA==;visitor_data=VD_OVERRIDE",
    )
    .unwrap();
    args.insert_ie_args(ie_key, parsed);
    let context = ExtractionContext::native().with_extractor_args(args);
    // Only the PLAYER-context entry is selected for the player request.
    assert_eq!(
        youtube_configured_player_po_token(&context).as_deref(),
        Some("QUJDRA==")
    );
    // The configured visitor data beats every page candidate.
    assert_eq!(
        youtube_configured_visitor_data(&context, &candidates).as_deref(),
        Some("VD_OVERRIDE")
    );
}

type YoutubeSeenPlayerRequests =
    std::sync::Arc<std::sync::Mutex<Vec<(Vec<(String, String)>, Vec<u8>)>>>;

struct YoutubeRecordingHandler {
    page: Vec<u8>,
    api: Vec<u8>,
    seen: YoutubeSeenPlayerRequests,
}

impl yt_dlp_networking::RequestHandler for YoutubeRecordingHandler {
    fn name(&self) -> &str {
        "youtube-recording"
    }

    fn supports(
        &self,
        _request: &yt_dlp_networking::Request,
    ) -> Result<(), yt_dlp_networking::RequestError> {
        Ok(())
    }

    fn send(
        &self,
        request: &yt_dlp_networking::Request,
    ) -> Result<yt_dlp_networking::Response, yt_dlp_networking::RequestError> {
        let body = if request.url().contains("/youtubei/v1/player") {
            self.seen.lock().expect("recording lock").push((
                request
                    .headers()
                    .iter()
                    .map(|(name, value)| (name.to_owned(), value.to_owned()))
                    .collect(),
                request.data().unwrap_or_default().to_vec(),
            ));
            self.api.clone()
        } else {
            self.page.clone()
        };
        Ok(yt_dlp_networking::Response::new(
            request.url(),
            200,
            "OK",
            body,
        ))
    }
}

fn youtube_recording_context(
    args: ExtractorArgs,
    seen: &YoutubeSeenPlayerRequests,
) -> ExtractionContext {
    // The page carries no streaming data, forcing the player API fallback the
    // recording handler observes.
    let config = serde_json::json!({
        "INNERTUBE_API_KEY": "fixture-api-key",
        "INNERTUBE_CONTEXT": {
            "client": {
                "clientName": "WEB",
                "clientVersion": "2.20260708.00.00"
            }
        }
    });
    let page = serde_json::to_string(&youtube_fixture_player_response(false)).unwrap();
    let page = format!(
        "<html><script>ytcfg.set({config});</script><script>var ytInitialPlayerResponse = {page};</script></html>",
        config = config,
    )
    .into_bytes();
    let mut director = RequestDirector::new();
    director.add_handler(YoutubeRecordingHandler {
        page,
        api: serde_json::to_vec(&youtube_fixture_player_response(true)).unwrap(),
        seen: seen.clone(),
    });
    ExtractionContext::new(director, CookieJar::new().shared()).with_extractor_args(args)
}

fn youtube_test_extractor() -> YoutubeExtractor {
    YoutubeExtractor::new(ExtractorDescriptor::new(
        "YoutubeIE",
        "youtube",
        r"https?://(?:www\.)?youtube\.com/.*",
        true,
    ))
    .unwrap()
}

#[test]
fn youtube_configured_po_token_and_visitor_reach_player_request() {
    use crate::ExtractorArgs;

    let mut args = ExtractorArgs::new();
    let (ie_key, parsed) = ExtractorArgs::parse_cli_value(
        "youtube:po_token=web.player+QUJDRA==;visitor_data=VD_OVERRIDE",
    )
    .unwrap();
    args.insert_ie_args(ie_key, parsed);
    let seen: YoutubeSeenPlayerRequests = Default::default();
    let context = youtube_recording_context(args, &seen);
    let result = youtube_test_extractor()
        .extract_with_context("https://www.youtube.com/watch?v=dQw4w9WgXcQ", &context)
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_str("title"), Some("Native YouTube fixture"));

    let seen = seen.lock().expect("recording lock");
    assert_eq!(seen.len(), 1);
    let (headers, body) = &seen[0];
    let payload: serde_json::Value = serde_json::from_slice(body).unwrap();
    assert_eq!(
        payload
            .get("serviceIntegrityDimensions")
            .and_then(|dimensions| dimensions.get("poToken"))
            .and_then(serde_json::Value::as_str),
        Some("QUJDRA==")
    );
    assert!(headers
        .iter()
        .any(|(name, value)| name == "X-Goog-Visitor-Id" && value == "VD_OVERRIDE"));
}

#[test]
fn youtube_player_request_omits_absent_session_inputs() {
    use crate::ExtractorArgs;

    let seen: YoutubeSeenPlayerRequests = Default::default();
    let context = youtube_recording_context(ExtractorArgs::new(), &seen);
    youtube_test_extractor()
        .extract_with_context("https://www.youtube.com/watch?v=dQw4w9WgXcQ", &context)
        .unwrap();
    let seen = seen.lock().expect("recording lock");
    assert_eq!(seen.len(), 1);
    let (headers, body) = &seen[0];
    let payload: serde_json::Value = serde_json::from_slice(body).unwrap();
    assert!(payload.get("serviceIntegrityDimensions").is_none());
    assert!(headers.iter().all(|(name, _)| name != "X-Goog-Visitor-Id"));
}
#[test]
fn youtube_playlist_intake_matches_python_id_rules() {
    use crate::native::youtube_playlist_id;

    // Bare IDs mirror `re.match` with `_PLAYLIST_ID_RE`: the ID prefix wins,
    // trailing garbage is ignored, short IDs stand alone.
    let bare = vec![
        ("PLBB231211A4F62143", Some("PLBB231211A4F62143")),
        ("PLBB231211A4F62143!!!", Some("PLBB231211A4F62143")),
        ("PLshort", None),
        ("WL", Some("WL")),
        ("RDMM", Some("RDMM")),
        (
            "OLAK5uy_m4xAFdmMC5rX3Ji3g93pQe3hqLZw_9LhM",
            Some("OLAK5uy_m4xAFdmMC5rX3Ji3g93pQe3hqLZw_9LhM"),
        ),
    ];
    for (input, expected) in bare {
        assert_eq!(
            youtube_playlist_id(input).as_deref(),
            expected,
            "input {input:?}"
        );
    }

    // Playlist page URLs resolve their `list` value the same way.
    let urls = vec![
        (
            "https://www.youtube.com/playlist?list=PLBB231211A4F62143",
            Some("PLBB231211A4F62143"),
        ),
        (
            "https://www.youtube.com/playlist?list=PLBB231211A4F62143&si=abc",
            Some("PLBB231211A4F62143"),
        ),
        ("https://m.youtube.com/playlist?list=WL", Some("WL")),
        (
            "https://www.youtube.com/embed/videoseries?list=PL6IaIsEjSbf96XFRuNccS_RuEXwNdsoEu",
            Some("PL6IaIsEjSbf96XFRuNccS_RuEXwNdsoEu"),
        ),
        // A `watch` URL without a video ID falls back to its playlist.
        (
            "https://www.youtube.com/watch?list=PLBB231211A4F62143",
            Some("PLBB231211A4F62143"),
        ),
        ("https://www.youtube.com/playlist?list=notavalidid!!", None),
        ("https://www.youtube.com/playlist", None),
        (
            "https://example.test/playlist?list=PLBB231211A4F62143",
            None,
        ),
        // Video, channel, and mixed URLs keep their existing routing.
        (
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ&list=PLBB231211A4F62143",
            None,
        ),
        ("https://www.youtube.com/watch?v=dQw4w9WgXcQ", None),
        (
            "https://www.youtube.com/channel/UCKSpbfbl5kRQpTdL7kMc-1Q",
            None,
        ),
    ];
    for (input, expected) in urls {
        assert_eq!(
            youtube_playlist_id(input).as_deref(),
            expected,
            "input {input:?}"
        );
    }
}

#[test]
fn youtube_playlist_continuation_matches_python_shapes() {
    use crate::native::youtube_extract_continuation;

    let cases = vec![
        (
            serde_json::json!({"continuations": [{"nextContinuationData": {
                "continuation": "TOKEN123", "clickTrackingParams": "CTP456"}}]}),
            Some(serde_json::json!({"continuation": "TOKEN123",
                "clickTracking": {"clickTrackingParams": "CTP456"}})),
        ),
        (
            serde_json::json!({"continuations": [{"nextContinuationData": {
                "continuation": "TOKEN123"}}]}),
            Some(serde_json::json!({"continuation": "TOKEN123"})),
        ),
        (
            serde_json::json!({"continuations": [{"nextContinuationData": {
                "continuation": ""}}]}),
            None,
        ),
        (
            serde_json::json!({"continuation": {"reloadContinuationData": {
                "continuation": "RELOAD1", "clickTrackingParams": "CTP7"}}}),
            Some(serde_json::json!({"continuation": "RELOAD1",
                "clickTracking": {"clickTrackingParams": "CTP7"}})),
        ),
        (
            serde_json::json!({"contents": [{
                "continuationItemRenderer": {
                    "continuationEndpoint": {
                        "continuationCommand": {"token": "EP_TOKEN"},
                    },
                },
            }]}),
            Some(serde_json::json!({"continuation": "EP_TOKEN"})),
        ),
        (
            serde_json::json!({"contents": [{
                "continuationItemRenderer": {
                    "button": {"buttonRenderer": {"command": {
                        "continuationCommand": {"token": "BTN_TOKEN"},
                    }}},
                },
            }]}),
            Some(serde_json::json!({"continuation": "BTN_TOKEN"})),
        ),
        (
            serde_json::json!({"contents": [{
                "continuationItemViewModel": {
                    "continuationCommand": {
                        "innertubeCommand": {
                            "continuationCommand": {"token": "VM_TOKEN"},
                            "clickTrackingParams": "VM_CTP",
                        },
                    },
                },
            }]}),
            Some(serde_json::json!({"continuation": "VM_TOKEN",
                "clickTracking": {"clickTrackingParams": "VM_CTP"}})),
        ),
        (
            serde_json::json!({"contents": [{"playlistVideoRenderer": {"videoId": "abc"}}]}),
            None,
        ),
        (serde_json::json!({}), None),
    ];
    for (index, (renderer, expected)) in cases.into_iter().enumerate() {
        assert_eq!(
            youtube_extract_continuation(&renderer),
            expected,
            "continuation case {index}"
        );
    }
}

fn youtube_playlist_item_fixture(video_id: &str, title: &str) -> serde_json::Value {
    serde_json::json!({"playlistVideoRenderer": {
        "videoId": video_id,
        "title": {"runs": [{"text": title}]},
        "lengthSeconds": "42",
    }})
}

fn youtube_playlist_continuation_tail(token: &str) -> serde_json::Value {
    serde_json::json!({"continuationItemRenderer": {
        "continuationEndpoint": {"continuationCommand": {
            "token": token, "request": "CONTINUATION_REQUEST_TYPE_BROWSE"}}}})
}

fn youtube_playlist_initial_data(
    list_contents: serde_json::Value,
    metadata: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "contents": {"twoColumnBrowseResultsRenderer": {"tabs": [
            {"tabRenderer": {"title": {"simpleText": "Playlist"}, "selected": true,
                "content": {"sectionListRenderer": {"contents": [
                    {"itemSectionRenderer": {"contents": [list_contents]}},
                ]}}}},
        ]}},
        "metadata": {"playlistMetadataRenderer": metadata},
        "microformat": {"microformatDataRenderer": {"tags": ["t1", "t2"]}},
    })
}

fn youtube_playlist_page_fixture(initial_data: &serde_json::Value) -> Vec<u8> {
    let config = serde_json::json!({
        "INNERTUBE_API_KEY": "fixture-api-key",
        "INNERTUBE_CONTEXT": {
            "client": {"clientName": "WEB", "clientVersion": "2.20260708.00.00"},
        },
    });
    let data = serde_json::to_string(initial_data).unwrap();
    format!(
        "<html><script>ytcfg.set({config});</script><script>var ytInitialData = {data};</script></html>",
        config = config,
    )
    .into_bytes()
}

struct YoutubePlaylistSpyState {
    page: Vec<u8>,
    browse_pages: std::sync::Mutex<std::collections::VecDeque<Vec<u8>>>,
    seen: std::sync::Mutex<Vec<(Vec<(String, String)>, serde_json::Value)>>,
}

struct YoutubePlaylistSpy {
    state: std::sync::Arc<YoutubePlaylistSpyState>,
}

impl yt_dlp_networking::RequestHandler for YoutubePlaylistSpy {
    fn name(&self) -> &str {
        "youtube-playlist-spy"
    }

    fn supports(
        &self,
        _request: &yt_dlp_networking::Request,
    ) -> Result<(), yt_dlp_networking::RequestError> {
        Ok(())
    }

    fn send(
        &self,
        request: &yt_dlp_networking::Request,
    ) -> Result<yt_dlp_networking::Response, yt_dlp_networking::RequestError> {
        let body = if request.url().contains("/youtubei/v1/browse") {
            let payload: serde_json::Value =
                serde_json::from_slice(request.data().unwrap_or_default()).unwrap();
            // The POST body merges the Innertube context with the query.
            assert!(payload.get("context").is_some());
            let headers = request
                .headers()
                .iter()
                .map(|(name, value)| (name.to_owned(), value.to_owned()))
                .collect();
            self.state
                .seen
                .lock()
                .expect("spy lock")
                .push((headers, payload));
            self.state
                .browse_pages
                .lock()
                .expect("spy lock")
                .pop_front()
                .unwrap_or_else(|| b"{}".to_vec())
        } else {
            self.state.page.clone()
        };
        Ok(yt_dlp_networking::Response::new(
            request.url(),
            200,
            "OK",
            body,
        ))
    }
}

fn youtube_playlist_spy_context(
    page: Vec<u8>,
    browse_pages: Vec<serde_json::Value>,
) -> (ExtractionContext, std::sync::Arc<YoutubePlaylistSpyState>) {
    let state = std::sync::Arc::new(YoutubePlaylistSpyState {
        page,
        browse_pages: std::sync::Mutex::new(
            browse_pages
                .into_iter()
                .map(|page| serde_json::to_vec(&page).unwrap())
                .collect(),
        ),
        seen: std::sync::Mutex::new(Vec::new()),
    });
    let mut director = RequestDirector::new();
    director.add_handler(YoutubePlaylistSpy {
        state: state.clone(),
    });
    (
        ExtractionContext::new(director, CookieJar::new().shared()),
        state,
    )
}

fn youtube_playlist_test_extractor() -> YoutubePlaylistExtractor {
    YoutubePlaylistExtractor::new(ExtractorDescriptor::new(
        "YoutubePlaylistIE",
        "youtube:playlist",
        "placeholder",
        true,
    ))
    .unwrap()
}

fn youtube_browse_page(
    items: Vec<serde_json::Value>,
    visitor_data: Option<&str>,
) -> serde_json::Value {
    let mut response = serde_json::json!({"onResponseReceivedActions": [{
        "appendContinuationItemsAction": {"continuationItems": items},
    }]});
    if let Some(visitor_data) = visitor_data {
        response["responseContext"] = serde_json::json!({ "visitorData": visitor_data });
    }
    response
}

#[test]
fn youtube_playlist_pagination_matches_python_entries() {
    let first = serde_json::json!({"playlistVideoListRenderer": {
        "contents": [youtube_playlist_item_fixture("vid1", "One")],
        "continuations": [{"nextContinuationData": {
            "continuation": "TOK1", "clickTrackingParams": "CTP1"}}],
    }});
    let page = youtube_playlist_page_fixture(&youtube_playlist_initial_data(
        first,
        serde_json::json!({"title": "Fixture Mix", "description": "Every side"}),
    ));
    let browse = vec![
        // The refreshed visitor data below must ride the next request.
        youtube_browse_page(
            vec![
                youtube_playlist_item_fixture("vid2", "Two"),
                youtube_playlist_continuation_tail("TOK2"),
            ],
            Some("VD_PAGE2"),
        ),
        youtube_browse_page(vec![youtube_playlist_item_fixture("vid3", "Three")], None),
    ];
    let (context, state) = youtube_playlist_spy_context(page, browse);
    let result = youtube_playlist_test_extractor()
        .extract_with_context(
            "https://www.youtube.com/playlist?list=PLBB231211A4F62143",
            &context,
        )
        .unwrap();
    let ExtractorResult::Playlist { info, entries } = result else {
        panic!("expected playlist result");
    };
    assert_eq!(info.get_str("id"), Some("PLBB231211A4F62143"));
    assert_eq!(info.get_str("title"), Some("Fixture Mix"));
    assert_eq!(info.get_str("description"), Some("Every side"));
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.get_str("id").unwrap_or_default())
            .collect::<Vec<_>>(),
        vec!["vid1", "vid2", "vid3"]
    );
    assert!(entries.iter().all(|entry| {
        entry.get_str("ie_key") == Some("Youtube") && entry.get_str("_type") == Some("url")
    }));
    assert_eq!(
        entries[1].get_str("url"),
        Some("https://www.youtube.com/watch?v=vid2")
    );
    // Runs-joined titles survive the entry mapping.
    assert_eq!(entries[0].get_str("title"), Some("One"));

    // Both browse POSTs carry the continuation query over the context.
    let seen = state.seen.lock().expect("spy lock");
    assert_eq!(seen.len(), 2);
    assert_eq!(
        seen[0]
            .1
            .get("continuation")
            .and_then(serde_json::Value::as_str),
        Some("TOK1")
    );
    assert_eq!(
        seen[0]
            .1
            .get("clickTracking")
            .and_then(|tracking| tracking.get("clickTrackingParams"))
            .and_then(serde_json::Value::as_str),
        Some("CTP1")
    );
    assert_eq!(
        seen[1]
            .1
            .get("continuation")
            .and_then(serde_json::Value::as_str),
        Some("TOK2")
    );
    // No visitor data is known up front, but the refreshed value from the
    // first browse page rides the second request.
    assert!(seen[0]
        .0
        .iter()
        .all(|(name, _)| name != "X-Goog-Visitor-Id"));
    assert!(seen[1]
        .0
        .iter()
        .any(|(name, value)| name == "X-Goog-Visitor-Id" && value == "VD_PAGE2"));
}

#[test]
fn youtube_playlist_loop_guard_stops_repeated_tokens() {
    let first = serde_json::json!({"playlistVideoListRenderer": {
        "contents": [youtube_playlist_item_fixture("vid1", "One")],
        "continuations": [{"nextContinuationData": {"continuation": "LOOP"}}],
    }});
    let page = youtube_playlist_page_fixture(&youtube_playlist_initial_data(
        first,
        serde_json::json!({"title": "Loop Mix"}),
    ));
    // The second browse page repeats the first page's token: the third POST
    // must never happen.
    let browse = vec![
        youtube_browse_page(
            vec![
                youtube_playlist_item_fixture("vid2", "Two"),
                youtube_playlist_continuation_tail("FRESH"),
            ],
            None,
        ),
        youtube_browse_page(
            vec![
                youtube_playlist_item_fixture("vid3", "Three"),
                youtube_playlist_continuation_tail("LOOP"),
            ],
            None,
        ),
    ];
    let (context, state) = youtube_playlist_spy_context(page, browse);
    let result = youtube_playlist_test_extractor()
        .extract_with_context("PLBB231211A4F62143", &context)
        .unwrap();
    let ExtractorResult::Playlist { entries, .. } = result else {
        panic!("expected playlist result");
    };
    assert_eq!(entries.len(), 3);
    assert_eq!(state.seen.lock().expect("spy lock").len(), 2);
}

#[test]
fn youtube_playlist_metadata_falls_back_to_id() {
    // No title anywhere: the playlist ID is the title, the description
    // defaults to empty, and missing tags default to an empty list.
    let first = serde_json::json!({"playlistVideoListRenderer": {"contents": []}});
    let mut data = youtube_playlist_initial_data(first, serde_json::json!({}));
    data.as_object_mut()
        .expect("fixture object")
        .remove("microformat");
    let page = youtube_playlist_page_fixture(&data);
    let (context, _) = youtube_playlist_spy_context(page, Vec::new());
    let result = youtube_playlist_test_extractor()
        .extract_with_context("https://www.youtube.com/playlist?list=WL", &context)
        .unwrap();
    let ExtractorResult::Playlist { info, entries } = result else {
        panic!("expected playlist result");
    };
    assert_eq!(info.get_str("id"), Some("WL"));
    assert_eq!(info.get_str("title"), Some("WL"));
    assert_eq!(info.get_str("description"), Some(""));
    assert!(entries.is_empty());
}

#[test]
fn youtube_playlist_registry_routes_playlist_urls_natively() {
    let registry = ExtractorRegistry::generated().unwrap();
    for url in [
        "https://www.youtube.com/playlist?list=PLBB231211A4F62143",
        "PLBB231211A4F62143",
    ] {
        let extractor = registry.find(url).expect("playlist route");
        assert_eq!(extractor.descriptor().key, "YoutubePlaylistIE");
        assert!(extractor.is_native());
    }
    // Video and channel URLs keep their existing routes.
    assert_eq!(
        registry
            .find("https://www.youtube.com/watch?v=dQw4w9WgXcQ")
            .expect("video route")
            .descriptor()
            .key,
        "YoutubeIE"
    );
}

// Tab-entries slice: every expectation below is the Python oracle output
// (`/tmp/ytvideo_oracle.py`) driving the real `YoutubeTabIE._extract_video`
// and its helpers over the mirrored fixtures.

#[test]
fn youtube_tab_parse_count_matches_oracle() {
    for (text, expected) in [
        ("1,234", Some(1234)),
        ("1.2K views", Some(1200)),
        ("3.45M", Some(3_450_000)),
        ("7B", Some(7_000_000_000)),
        ("0", Some(0)),
        ("12 waiting", Some(12)),
        ("1,5K", Some(1500)),
        ("2kk", Some(2_000_000)),
        ("918K subscribers", Some(918_000)),
        ("", None),
    ] {
        assert_eq!(
            crate::native::youtube_parse_count(text),
            expected,
            "parse_count {text:?}"
        );
    }
}

#[test]
fn youtube_tab_parse_duration_matches_oracle() {
    for (text, expected) in [
        ("12:34", Some(754.0)),
        ("1:02:03", Some(3723.0)),
        ("754", Some(754.0)),
        ("12:34.500", Some(754.5)),
        ("PT1H2M3S", Some(3723.0)),
        ("3 hours", Some(10_800.0)),
        ("45 mins", Some(2700.0)),
        ("90s", Some(90.0)),
        ("1:02", Some(62.0)),
        ("0:15", Some(15.0)),
        ("2 days", None),
        ("", None),
    ] {
        assert_eq!(
            crate::native::youtube_parse_duration(text),
            expected,
            "parse_duration {text:?}"
        );
    }
}

#[test]
fn youtube_tab_renderer_text_matches_oracle() {
    let renderer_text = |data: &serde_json::Value, paths: &[&[&str]]| {
        crate::native::youtube_renderer_text(data, paths)
    };
    assert_eq!(
        renderer_text(
            &serde_json::json!({"title": {"simpleText": "Hi"}}),
            &[&["title"]]
        ),
        Some("Hi".to_owned())
    );
    assert_eq!(
        renderer_text(
            &serde_json::json!({"title": {"runs": [{"text": "A"}, {"text": "B"}]}}),
            &[&["title"]]
        ),
        Some("AB".to_owned())
    );
    assert_eq!(
        renderer_text(
            &serde_json::json!({"headline": {"simpleText": "Head"}}),
            &[&["title"], &["headline"]]
        ),
        Some("Head".to_owned())
    );
    // Bare strings never count as text.
    assert_eq!(
        renderer_text(
            &serde_json::json!({"videoInfo": "50K views"}),
            &[&["viewCountText"], &["videoInfo"]]
        ),
        None
    );
    assert_eq!(
        renderer_text(
            &serde_json::json!({"overlays": [{"overlayTimeStatus": {"text": {"simpleText": "8:20"}}}]}),
            &[&["overlays", "...", "overlayTimeStatus", "text"]]
        ),
        Some("8:20".to_owned())
    );
    assert_eq!(
        renderer_text(&serde_json::json!({"title": {"runs": []}}), &[&["title"]]),
        None
    );
}

#[test]
fn youtube_tab_badges_match_oracle() {
    fn badge_names(badges: &[crate::native::YoutubeBadge]) -> Vec<&'static str> {
        badges
            .iter()
            .map(|badge| match badge {
                crate::native::YoutubeBadge::AvailabilityUnlisted => {
                    "BadgeType.AVAILABILITY_UNLISTED"
                }
                crate::native::YoutubeBadge::AvailabilityPrivate => {
                    "BadgeType.AVAILABILITY_PRIVATE"
                }
                crate::native::YoutubeBadge::AvailabilityPublic => "BadgeType.AVAILABILITY_PUBLIC",
                crate::native::YoutubeBadge::AvailabilityPremium => {
                    "BadgeType.AVAILABILITY_PREMIUM"
                }
                crate::native::YoutubeBadge::AvailabilitySubscription => {
                    "BadgeType.AVAILABILITY_SUBSCRIPTION"
                }
                crate::native::YoutubeBadge::LiveNow => "BadgeType.LIVE_NOW",
                crate::native::YoutubeBadge::Verified => "BadgeType.VERIFIED",
            })
            .collect()
    }
    let names = |list: serde_json::Value| badge_names(&crate::native::youtube_badges(&list));
    assert_eq!(
        names(
            serde_json::json!([{"metadataBadgeRenderer": {"style": "BADGE_STYLE_TYPE_LIVE_NOW"}}])
        ),
        ["BadgeType.LIVE_NOW"]
    );
    assert_eq!(
        names(
            serde_json::json!([{"metadataBadgeRenderer": {"icon": {"iconType": "PRIVACY_UNLISTED"}}}])
        ),
        ["BadgeType.AVAILABILITY_UNLISTED"]
    );
    assert_eq!(
        names(serde_json::json!([{"metadataBadgeRenderer": {"label": "Members only"}}])),
        ["BadgeType.AVAILABILITY_SUBSCRIPTION"]
    );
    // Unknown style plus an unmapped label yields no badge.
    assert!(names(serde_json::json!([{"metadataBadgeRenderer": {"style": "UNKNOWN_STYLE", "label": "New"}}])).is_empty());
    assert!(names(serde_json::Value::Null).is_empty());
    // `_has_badge` over the extracted list.
    let badges = crate::native::youtube_badges(
        &serde_json::json!([{"metadataBadgeRenderer": {"style": "BADGE_STYLE_TYPE_VERIFIED"}}]),
    );
    assert!(crate::native::youtube_has_badge(
        &badges,
        crate::native::YoutubeBadge::Verified
    ));
    assert!(!crate::native::youtube_has_badge(
        &badges,
        crate::native::YoutubeBadge::LiveNow
    ));
}

#[test]
fn youtube_tab_thumbnails_match_oracle() {
    let data = serde_json::json!({"thumbnail": {"thumbnails": [
        {"url": "https://i.ytimg.com/vi/x/default.jpg", "width": 120, "height": 90},
        {"url": "https://i.ytimg.com/vi/x/maxresdefault.jpg?sqp=abc", "width": 1280, "height": 720},
        {"url": "not a url", "width": 10, "height": 10},
        {"url": "https://i.ytimg.com/vi/x/hq.jpg", "width": "480", "height": "360"},
    ]}});
    assert_eq!(
        serde_json::Value::Array(crate::native::youtube_entry_thumbnails(&data)),
        serde_json::json!([
            {"url": "https://i.ytimg.com/vi/x/default.jpg", "width": 120, "height": 90},
            {"url": "https://i.ytimg.com/vi/x/maxresdefault.jpg", "width": 1280, "height": 720},
            {"url": "https://i.ytimg.com/vi/x/hq.jpg", "width": 480, "height": 360},
        ])
    );
}

#[test]
fn youtube_tab_get_count_matches_oracle() {
    let paths: &[&[&str]] = &[&["viewCountText"]];
    assert_eq!(
        crate::native::youtube_get_count(
            &serde_json::json!({"viewCountText": {"simpleText": "1,234 views"}}),
            paths
        ),
        Some(1234)
    );
    assert_eq!(
        crate::native::youtube_get_count(
            &serde_json::json!({"viewCountText": {"simpleText": "No views"}}),
            paths
        ),
        Some(0)
    );
    let paths: &[&[&str]] = &[&["viewCountText"], &["shortViewCountText"]];
    assert_eq!(
        crate::native::youtube_get_count(
            &serde_json::json!({"shortViewCountText": {"simpleText": "45 waiting"}}),
            paths
        ),
        Some(45)
    );
    assert_eq!(
        crate::native::youtube_get_count(&serde_json::json!({}), paths),
        None
    );
}

#[test]
fn youtube_tab_ucid_and_handle_match_oracle() {
    assert_eq!(
        crate::native::youtube_ucid(Some("UCuAXFkgpKOUxaRXCkBosP9w")).as_deref(),
        Some("UCuAXFkgpKOUxaRXCkBosP9w")
    );
    assert_eq!(crate::native::youtube_ucid(Some("not-a-ucid")), None);
    assert_eq!(crate::native::youtube_ucid(None), None);
    assert_eq!(
        crate::native::youtube_handle_from_url(Some("https://www.youtube.com/@RickAstley/videos"))
            .as_deref(),
        Some("@RickAstley")
    );
    assert_eq!(
        crate::native::youtube_handle_from_url(Some("/@RickAstley")).as_deref(),
        Some("@RickAstley")
    );
    assert_eq!(crate::native::youtube_handle_from_url(None), None);
}

/// Mirror of the oracle `video_renderer()` defaults.
fn youtube_tab_standard_renderer() -> serde_json::Value {
    serde_json::json!({
        "videoId": "dQw4w9WgXcQ",
        "title": {"runs": [{"text": "Never Gonna Give You Up"}]},
        "navigationEndpoint": {
            "commandMetadata": {"webCommandMetadata": {"url": "/watch?v=dQw4w9WgXcQ"}},
        },
        "lengthSeconds": "212",
        "shortBylineText": {"runs": [{"text": "Rick Astley",
            "navigationEndpoint": {"browseEndpoint": {"browseId": "UCuAXFkgpKOUxaRXCkBosP9w"}}}]},
        "ownerText": {"runs": [{"text": "Rick Astley"}]},
        "viewCountText": {"simpleText": "1,234,567 views"},
        "publishedTimeText": {"simpleText": "14 years ago"},
        "thumbnail": {"thumbnails": [
            {"url": "https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg", "width": 480, "height": 360},
            {"url": "https://i.ytimg.com/vi/dQw4w9WgXcQ/maxresdefault.jpg?sqp=-oaymwEmCIAKENAF8quKqQMa8AEB-AH-CYAC0AWKAgwIABABGGUgZShlMA8=&rs=AOn4CLBwNo9i8sV06a0_UVuaqyfbaa2g",
             "width": 1280, "height": 720},
        ]},
    })
}

/// Normalize an entry for oracle comparison: every oracle key present,
/// missing ≡ null (the port omits unset fields instead of storing nulls).
fn youtube_tab_normalized_entry(entry: &yt_dlp_core::InfoDict) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for key in [
        "_type",
        "ie_key",
        "id",
        "url",
        "title",
        "description",
        "duration",
        "channel_id",
        "channel",
        "channel_url",
        "uploader",
        "uploader_id",
        "uploader_url",
        "thumbnails",
        "timestamp",
        "release_timestamp",
        "availability",
        "view_count",
        "concurrent_view_count",
        "live_status",
        "channel_is_verified",
    ] {
        map.insert(
            key.to_owned(),
            entry.get(key).cloned().unwrap_or(serde_json::Value::Null),
        );
    }
    serde_json::Value::Object(map)
}

fn youtube_tab_standard_expected() -> serde_json::Value {
    serde_json::json!({
        "_type": "url",
        "availability": null,
        "channel": "Rick Astley",
        "channel_id": "UCuAXFkgpKOUxaRXCkBosP9w",
        "channel_is_verified": null,
        "channel_url": "https://www.youtube.com/channel/UCuAXFkgpKOUxaRXCkBosP9w",
        "description": null,
        "duration": 212,
        "id": "dQw4w9WgXcQ",
        "ie_key": "Youtube",
        "live_status": null,
        "release_timestamp": null,
        "thumbnails": [
            {"height": 360, "url": "https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg", "width": 480},
            {"height": 720, "url": "https://i.ytimg.com/vi/dQw4w9WgXcQ/maxresdefault.jpg", "width": 1280},
        ],
        "timestamp": null,
        "title": "Never Gonna Give You Up",
        "uploader": "Rick Astley",
        "uploader_id": null,
        "uploader_url": null,
        "url": "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
        "view_count": 1234567,
        "concurrent_view_count": null,
    })
}

#[test]
fn youtube_tab_extract_video_standard_matches_oracle() {
    let entry = crate::native::youtube_extract_video(&youtube_tab_standard_renderer())
        .expect("standard renderer extracts");
    assert_eq!(
        youtube_tab_normalized_entry(&entry),
        youtube_tab_standard_expected()
    );
}

#[test]
fn youtube_tab_extract_video_variants_match_oracle() {
    // lengthText fallback plus unlisted badge and verified owner badge.
    let mut renderer = youtube_tab_standard_renderer();
    renderer["lengthSeconds"] = serde_json::Value::Null;
    renderer["lengthText"] = serde_json::json!({"simpleText": "1:02:03"});
    renderer["badges"] =
        serde_json::json!([{"metadataBadgeRenderer": {"icon": {"iconType": "PRIVACY_UNLISTED"}}}]);
    renderer["ownerBadges"] = serde_json::json!([{"metadataBadgeRenderer": {"icon": {"iconType": "CHECK_CIRCLE_THICK"}}}]);
    let entry = crate::native::youtube_extract_video(&renderer).expect("text duration extracts");
    let mut expected = youtube_tab_standard_expected();
    expected["duration"] = serde_json::json!(3723);
    expected["availability"] = serde_json::json!("unlisted");
    expected["channel_is_verified"] = serde_json::json!(true);
    assert_eq!(youtube_tab_normalized_entry(&entry), expected);

    // Thumbnail-overlay duration fallback.
    let mut renderer = youtube_tab_standard_renderer();
    renderer["lengthSeconds"] = serde_json::Value::Null;
    renderer["thumbnailOverlays"] = serde_json::json!([{"thumbnailOverlayTimeStatusRenderer": {
        "text": {"runs": [{"text": "8:20"}]}, "style": "DEFAULT"}}]);
    let entry = crate::native::youtube_extract_video(&renderer).expect("overlay duration extracts");
    let mut expected = youtube_tab_standard_expected();
    expected["duration"] = serde_json::json!(500);
    assert_eq!(youtube_tab_normalized_entry(&entry), expected);

    // Shorts: accessibility-label duration fallback and /shorts/ URL.
    let mut renderer = youtube_tab_standard_renderer();
    renderer["lengthSeconds"] = serde_json::Value::Null;
    renderer["title"] = serde_json::json!({"runs": [{"text": "A short"}],
        "accessibility": {"accessibilityData": {
            "label": "A short 5 years ago 15 seconds 1,234 views - play short"}}});
    renderer["navigationEndpoint"] = serde_json::json!({"commandMetadata":
        {"webCommandMetadata": {"url": "/shorts/dQw4w9WgXcQ"}}});
    renderer["thumbnailOverlays"] = serde_json::json!([{"thumbnailOverlayTimeStatusRenderer": {
        "text": {"simpleText": "0:15"}, "style": "SHORTS"}}]);
    let entry = crate::native::youtube_extract_video(&renderer).expect("shorts extracts");
    let mut expected = youtube_tab_standard_expected();
    expected["duration"] = serde_json::json!(15);
    expected["title"] = serde_json::json!("A short");
    expected["url"] = serde_json::json!("https://www.youtube.com/shorts/dQw4w9WgXcQ");
    assert_eq!(youtube_tab_normalized_entry(&entry), expected);

    // Live: LIVE overlay plus live badge, watching count, no duration.
    let mut renderer = youtube_tab_standard_renderer();
    renderer["lengthSeconds"] = serde_json::Value::Null;
    renderer["viewCountText"] = serde_json::Value::Null;
    renderer["shortViewCountText"] = serde_json::json!({"simpleText": "1,234 watching"});
    renderer["badges"] =
        serde_json::json!([{"metadataBadgeRenderer": {"style": "BADGE_STYLE_TYPE_LIVE_NOW"}}]);
    renderer["thumbnailOverlays"] = serde_json::json!([{"thumbnailOverlayTimeStatusRenderer": {
        "text": {"simpleText": "LIVE"}, "style": "LIVE"}}]);
    let entry = crate::native::youtube_extract_video(&renderer).expect("live extracts");
    let mut expected = youtube_tab_standard_expected();
    expected["duration"] = serde_json::Value::Null;
    expected["live_status"] = serde_json::json!("is_live");
    expected["view_count"] = serde_json::Value::Null;
    expected["concurrent_view_count"] = serde_json::json!(1234);
    assert_eq!(youtube_tab_normalized_entry(&entry), expected);

    // Upcoming: scheduled timestamp drives is_upcoming and the release stamp.
    let mut renderer = youtube_tab_standard_renderer();
    renderer["lengthSeconds"] = serde_json::Value::Null;
    renderer["viewCountText"] = serde_json::Value::Null;
    renderer["shortViewCountText"] = serde_json::json!({"simpleText": "45 waiting"});
    renderer["publishedTimeText"] =
        serde_json::json!({"simpleText": "Scheduled for 1/1/30, 12:00 AM"});
    renderer["upcomingEventData"] = serde_json::json!({"startTime": "1893456000"});
    let entry = crate::native::youtube_extract_video(&renderer).expect("upcoming extracts");
    let mut expected = youtube_tab_standard_expected();
    expected["duration"] = serde_json::Value::Null;
    expected["live_status"] = serde_json::json!("is_upcoming");
    expected["release_timestamp"] = serde_json::json!(1893456000);
    expected["view_count"] = serde_json::Value::Null;
    expected["concurrent_view_count"] = serde_json::json!(45);
    assert_eq!(youtube_tab_normalized_entry(&entry), expected);

    // Premium badge plus handle-only byline: canonical handle, no channel id.
    let mut renderer = youtube_tab_standard_renderer();
    renderer["shortBylineText"] = serde_json::json!({"runs": [{"text": "Vevo",
        "navigationEndpoint": {"commandMetadata": {"webCommandMetadata": {"url": "/@Vevo"}}}}]});
    renderer["badges"] =
        serde_json::json!([{"metadataBadgeRenderer": {"style": "BADGE_STYLE_TYPE_PREMIUM"}}]);
    renderer["ownerBadges"] = serde_json::json!([{"metadataBadgeRenderer": {"icon": {"iconType": "OFFICIAL_ARTIST_BADGE"}}}]);
    let entry = crate::native::youtube_extract_video(&renderer).expect("premium extracts");
    let mut expected = youtube_tab_standard_expected();
    expected["availability"] = serde_json::json!("premium_only");
    expected["channel_id"] = serde_json::Value::Null;
    expected["channel_url"] = serde_json::Value::Null;
    expected["channel_is_verified"] = serde_json::json!(true);
    expected["uploader_id"] = serde_json::json!("@Vevo");
    expected["uploader_url"] = serde_json::json!("https://www.youtube.com/@Vevo");
    assert_eq!(youtube_tab_normalized_entry(&entry), expected);

    // Headline title fallback and snippet description.
    let mut renderer = youtube_tab_standard_renderer();
    renderer["title"] = serde_json::Value::Null;
    renderer["headline"] = serde_json::json!({"simpleText": "Headline title"});
    renderer["detailedMetadataSnippets"] =
        serde_json::json!([{"snippetText": {"runs": [{"text": "Desc here"}]}}]);
    let entry = crate::native::youtube_extract_video(&renderer).expect("headline extracts");
    let mut expected = youtube_tab_standard_expected();
    expected["title"] = serde_json::json!("Headline title");
    expected["description"] = serde_json::json!("Desc here");
    assert_eq!(youtube_tab_normalized_entry(&entry), expected);

    // Missing video ID: skipped, matching the playlist-entry convention.
    let mut renderer = youtube_tab_standard_renderer();
    renderer["videoId"] = serde_json::Value::Null;
    assert!(crate::native::youtube_extract_video(&renderer).is_none());
}

// Tab claiming/dispatch slice: expectations are the Python oracle output
// (`/tmp/yttab_oracle.py`) driving the real `YoutubeTabIE` URL matcher,
// tab helpers, and first-page `_extract_entries` over mirrored fixtures.

#[test]
fn youtube_tab_suitable_matches_oracle() {
    for (url, expected) in [
        (
            "https://www.youtube.com/channel/UCuAXFkgpKOUxaRXCkBosP9w",
            true,
        ),
        (
            "https://www.youtube.com/channel/UCuAXFkgpKOUxaRXCkBosP9w/videos",
            true,
        ),
        (
            "https://www.youtube.com/c/SomeChannel/shorts?view=0&sort=dd",
            true,
        ),
        ("https://www.youtube.com/user/oldname/streams", true),
        ("https://www.youtube.com/browse/UCxxxx/playlists", true),
        ("https://m.youtube.com/channel/UCxxxx/videos", true),
        ("http://www.youtube.com/@Handle/videos", true),
        ("https://www.youtube.com/@Handle", true),
        ("https://www.youtube.com/feed/subscriptions", true),
        ("https://www.youtube.com/hashtag/music", true),
        (
            "https://www.youtube.com/playlist?list=PL1234567890abcdef",
            true,
        ),
        (
            "https://www.youtube.com/watch?list=PL1234567890abcdef",
            true,
        ),
        ("https://consent.youtube.com/channel/UCxxxx", false),
        ("https://www.youtube.com/watch?v=dQw4w9WgXcQ", false),
        ("https://youtu.be/dQw4w9WgXcQ", false),
        ("https://www.youtube.com/trending", false),
        ("https://www.youtube-nocookie.com/channel/UCxxxx", false),
    ] {
        assert_eq!(
            crate::native::youtube_tab_suitable(url),
            expected,
            "tab suitable {url:?}"
        );
    }
}

#[test]
fn youtube_tab_url_parts_match_oracle() {
    fn parts(url: &str) -> serde_json::Value {
        let mobj = crate::native::youtube_tab_url_parts(url).expect("tab url parses");
        serde_json::json!({
            "pre": mobj.pre,
            "tab": mobj.tab,
            "post": mobj.post,
            "not_channel": mobj.not_channel,
            "channel_type": mobj.channel_type,
            "id": mobj.id,
        })
    }
    assert_eq!(
        parts("https://www.youtube.com/channel/UCuAXFkgpKOUxaRXCkBosP9w/videos"),
        serde_json::json!({
            "pre": "https://www.youtube.com/channel/UCuAXFkgpKOUxaRXCkBosP9w",
            "tab": "/videos", "post": "", "not_channel": "",
            "channel_type": "channel", "id": "UCuAXFkgpKOUxaRXCkBosP9w",
        })
    );
    assert_eq!(
        parts("https://www.youtube.com/c/SomeChannel/shorts?view=0&sort=dd"),
        serde_json::json!({
            "pre": "https://www.youtube.com/c/SomeChannel",
            "tab": "/shorts", "post": "?view=0&sort=dd", "not_channel": "",
            "channel_type": "c", "id": "SomeChannel",
        })
    );
    assert_eq!(
        parts("https://www.youtube.com/feed/subscriptions"),
        serde_json::json!({
            "pre": "https://www.youtube.com/feed/subscriptions",
            "tab": "", "post": "", "not_channel": "feed/",
            "channel_type": "", "id": "subscriptions",
        })
    );
    assert_eq!(
        parts("https://www.youtube.com/playlist?list=PL1234567890abcdef"),
        serde_json::json!({
            "pre": "https://www.youtube.com/playlist?list=PL1234567890abcdef",
            "tab": "", "post": "", "not_channel": "playlist?list=",
            "channel_type": "", "id": "PL1234567890abcdef",
        })
    );
    assert_eq!(
        parts("http://www.youtube.com/@Handle/videos"),
        serde_json::json!({
            "pre": "http://www.youtube.com/@Handle",
            "tab": "/videos", "post": "", "not_channel": "",
            "channel_type": "", "id": "@Handle",
        })
    );
    assert_eq!(
        parts("https://www.youtube.com/channel/UCuAXFkgpKOUxaRXCkBosP9w"),
        serde_json::json!({
            "pre": "https://www.youtube.com/channel/UCuAXFkgpKOUxaRXCkBosP9w",
            "tab": "", "post": "", "not_channel": "",
            "channel_type": "channel", "id": "UCuAXFkgpKOUxaRXCkBosP9w",
        })
    );
    assert_eq!(
        parts("https://www.youtube.com/user/oldname/streams"),
        serde_json::json!({
            "pre": "https://www.youtube.com/user/oldname",
            "tab": "/streams", "post": "", "not_channel": "",
            "channel_type": "user", "id": "oldname",
        })
    );
    assert_eq!(
        parts("https://www.youtube.com/browse/UCxxxx/playlists"),
        serde_json::json!({
            "pre": "https://www.youtube.com/browse/UCxxxx",
            "tab": "/playlists", "post": "", "not_channel": "",
            "channel_type": "browse", "id": "UCxxxx",
        })
    );
    assert_eq!(
        parts("https://m.youtube.com/channel/UCxxxx/videos"),
        serde_json::json!({
            "pre": "https://m.youtube.com/channel/UCxxxx",
            "tab": "/videos", "post": "", "not_channel": "",
            "channel_type": "channel", "id": "UCxxxx",
        })
    );
    assert_eq!(
        parts("https://www.youtube.com/@Handle"),
        serde_json::json!({
            "pre": "https://www.youtube.com/@Handle",
            "tab": "", "post": "", "not_channel": "",
            "channel_type": "", "id": "@Handle",
        })
    );
    assert_eq!(
        parts("https://www.youtube.com/hashtag/music"),
        serde_json::json!({
            "pre": "https://www.youtube.com/hashtag/music",
            "tab": "", "post": "", "not_channel": "hashtag/",
            "channel_type": "", "id": "music",
        })
    );
    assert_eq!(
        parts("https://www.youtube.com/watch?list=PL1234567890abcdef"),
        serde_json::json!({
            "pre": "https://www.youtube.com/watch?list=PL1234567890abcdef",
            "tab": "", "post": "", "not_channel": "watch?list=",
            "channel_type": "", "id": "PL1234567890abcdef",
        })
    );
    assert!(crate::native::youtube_tab_url_parts("https://www.youtube.com/trending").is_none());
    assert!(
        crate::native::youtube_tab_url_parts("https://www.youtube.com/watch?v=dQw4w9WgXcQ")
            .is_none()
    );
}

#[test]
fn youtube_tab_id_and_name_matches_oracle() {
    fn tab(title: Option<&str>, url: Option<&str>, identifier: Option<&str>) -> serde_json::Value {
        let mut renderer = serde_json::Map::new();
        if let Some(title) = title {
            renderer.insert("title".to_owned(), serde_json::json!(title));
        }
        if let Some(url) = url {
            renderer.insert(
                "endpoint".to_owned(),
                serde_json::json!({"commandMetadata": {"webCommandMetadata": {"url": url}}}),
            );
        }
        if let Some(identifier) = identifier {
            renderer.insert("tabIdentifier".to_owned(), serde_json::json!(identifier));
        }
        serde_json::Value::Object(renderer)
    }
    let id_and_name = |tab: &serde_json::Value| {
        let (id, name) = crate::native::youtube_tab_id_and_name(tab, "https://www.youtube.com");
        (id, name)
    };
    // Endpoint tab segments win over names.
    assert_eq!(
        id_and_name(&tab(Some("Videos"), Some("/channel/UCxxxx/videos"), None)),
        ("videos".to_owned(), "videos".to_owned())
    );
    assert_eq!(
        id_and_name(&tab(Some("Home"), Some("/channel/UCxxxx/featured"), None)),
        ("featured".to_owned(), "home".to_owned())
    );
    // Identifier mapping and name fallbacks.
    assert_eq!(
        id_and_name(&tab(None, None, Some("TAB_ID_SPONSORSHIPS"))),
        ("membership".to_owned(), String::new())
    );
    assert_eq!(
        id_and_name(&tab(Some("Home"), None, None)),
        ("featured".to_owned(), "home".to_owned())
    );
    assert_eq!(
        id_and_name(&tab(Some("Live"), None, None)),
        ("streams".to_owned(), "live".to_owned())
    );
    assert_eq!(
        id_and_name(&tab(Some("Videos"), None, None)),
        ("videos".to_owned(), "videos".to_owned())
    );
    assert_eq!(
        id_and_name(&serde_json::json!({})),
        (String::new(), String::new())
    );
    let tabs = [
        tab(Some("Videos"), Some("/channel/UCxxxx/videos"), None),
        tab(Some("Shorts"), Some("/channel/UCxxxx/shorts"), None),
        tab(Some("Home"), Some("/channel/UCxxxx/featured"), None),
    ];
    assert!(crate::native::youtube_has_tab(&tabs, "shorts"));
    assert!(!crate::native::youtube_has_tab(&tabs, "playlists"));
}

#[test]
fn youtube_tab_selected_tab_matches_oracle() {
    let tabs = [
        serde_json::json!({"title": "Videos", "selected": true}),
        serde_json::json!({"title": "Shorts"}),
    ];
    let selected = crate::native::youtube_selected_tab(&tabs, true)
        .expect("selected tab")
        .expect("selected tab present");
    assert_eq!(
        selected.get("title").and_then(|title| title.as_str()),
        Some("Videos")
    );
    // Missing selection is fatal with the source message.
    let error = crate::native::youtube_selected_tab(&tabs[1..], true).unwrap_err();
    assert_eq!(error.message, "Unable to find selected tab");
    // ... but optional when not fatal.
    assert!(
        crate::native::youtube_selected_tab(&tabs[1..], false)
            .unwrap()
            .is_none()
    );
}

fn youtube_tab_video_fixture(video_id: &str, title: &str) -> serde_json::Value {
    serde_json::json!({"videoRenderer": {
        "videoId": video_id,
        "title": {"runs": [{"text": title}]},
        "lengthText": {"simpleText": "3:21"},
        "viewCountText": {"simpleText": "1,000 views"},
        "navigationEndpoint": {
            "commandMetadata": {"webCommandMetadata": {"url": format!("/watch?v={video_id}")}}},
        "thumbnail": {"thumbnails": []},
    }})
}

fn youtube_tab_entry_ids(entries: &[yt_dlp_core::InfoDict]) -> Vec<String> {
    entries
        .iter()
        .filter_map(|entry| entry.get_str("id"))
        .map(str::to_owned)
        .collect()
}

#[test]
fn youtube_tab_first_page_videos_match_oracle() {
    let tab = serde_json::json!({"content": {"sectionListRenderer": {"contents": [
        {"itemSectionRenderer": {"contents": [
            youtube_tab_video_fixture("id000000001", "First"),
            youtube_tab_video_fixture("id000000002", "Second"),
            {"shelfRenderer": {"title": {"simpleText": "Skip me"}}},
        ]}},
    ]}}});
    let (entries, continuation) = crate::native::youtube_tab_first_page(&tab);
    // Unknown shelf children are skipped; no continuation anywhere.
    assert_eq!(
        youtube_tab_entry_ids(&entries),
        ["id000000001", "id000000002"]
    );
    assert_eq!(entries[0].get_str("title"), Some("First"));
    assert!(continuation.is_none());

    // A continuation item sets the next query and contributes no entry.
    let tab = serde_json::json!({"content": {"sectionListRenderer": {"contents": [
        {"itemSectionRenderer": {"contents": [
            youtube_tab_video_fixture("id000000003", "Third"),
            {"continuationItemRenderer": {"continuationEndpoint": {
                "continuationCommand": {"token": "NEXTPAGE"},
                "clickTrackingParams": "CTP"}}},
        ]}},
    ]}}});
    let (entries, continuation) = crate::native::youtube_tab_first_page(&tab);
    assert_eq!(youtube_tab_entry_ids(&entries), ["id000000003"]);
    assert_eq!(
        continuation,
        Some(serde_json::json!({
            "continuation": "NEXTPAGE",
            "clickTracking": {"clickTrackingParams": "CTP"},
        }))
    );

    // Rich-grid parents dispatch rich items: video, playlist, and reel.
    let tab = serde_json::json!({"content": {"richGridRenderer": {"contents": [
        {"richItemRenderer": {"content": youtube_tab_video_fixture("id000000004", "Rich")}},
        {"richItemRenderer": {"content": {"playlistRenderer": {
            "playlistId": "PLrichplaylist00000000000001",
            "title": {"simpleText": "Rich playlist"}}}}},
        {"richItemRenderer": {"content": {"reelItemRenderer": {
            "videoId": "id000000005",
            "headline": {"simpleText": "A reel"},
            "navigationEndpoint": {"commandMetadata":
                {"webCommandMetadata": {"url": "/shorts/id000000005"}}},
            "thumbnail": {"thumbnails": []}}}}},
        {"continuationItemRenderer": {"continuationEndpoint": {
            "continuationCommand": {"token": "RICHNEXT"}}}},
    ]}}});
    let (entries, continuation) = crate::native::youtube_tab_first_page(&tab);
    let summary: Vec<serde_json::Value> = entries
        .iter()
        .map(|entry| {
            serde_json::json!({
                "id": entry.get_str("id"),
                "title": entry.get_str("title"),
                "url": entry.get_str("url"),
                "ie_key": entry.get_str("ie_key"),
            })
        })
        .collect();
    assert_eq!(
        serde_json::Value::Array(summary),
        serde_json::json!([
            {"id": "id000000004", "title": "Rich",
             "url": "https://www.youtube.com/watch?v=id000000004", "ie_key": "Youtube"},
            {"id": "PLrichplaylist00000000000001", "title": "Rich playlist",
             "url": "https://www.youtube.com/playlist?list=PLrichplaylist00000000000001",
             "ie_key": "YoutubeTab"},
            {"id": "id000000005", "title": "A reel",
             "url": "https://www.youtube.com/shorts/id000000005", "ie_key": "Youtube"},
        ])
    );
    assert_eq!(
        continuation,
        Some(serde_json::json!({"continuation": "RICHNEXT"}))
    );

    // No content at all: no entries, no continuation.
    let (entries, continuation) = crate::native::youtube_tab_first_page(&serde_json::json!({}));
    assert!(entries.is_empty());
    assert!(continuation.is_none());
}

// Tab composition slice: expectations are the Python oracle output
// (`/tmp/yttab2_oracle.py`) driving the real tab metadata, availability,
// mocked multi-page `_entries`, and `_extract_from_tabs` over mirrored
// fixtures.

fn youtube_tab_channel_data() -> serde_json::Value {
    serde_json::json!({
        "metadata": {"channelMetadataRenderer": {
            "title": "Rick Astley",
            "externalId": "UCuAXFkgpKOUxaRXCkBosP9w",
            "vanityChannelUrl": "https://www.youtube.com/@RickAstley",
            "description": "Official channel",
            "keywords": "rick astley music",
            "avatar": {"thumbnails": [
                {"url": "https://yt3.googleusercontent.com/avatar=s176", "width": 176, "height": 176},
            ]},
        }},
        "header": {
            "pageHeaderRenderer": {"content": {"pageHeaderViewModel": {
                "banner": {"imageBannerViewModel": {"image": {"sources": [
                    {"url": "https://yt3.googleusercontent.com/banner", "width": 1280, "height": 248},
                ]}}},
                "metadata": {"contentMetadataViewModel": {"metadataRows": [
                    {"metadataParts": [{"text": {"content": "3.1M subscribers"}}]},
                ]}},
                "title": {"dynamicTextViewModel": {"text": {"attachmentRuns": [{"element": {"type": {
                    "imageType": {"image": {"sources": [
                        {"clientResource": {"imageName": "CHECK_CIRCLE_FILLED"}},
                    ]}}}}}]}}},
            }}},
        },
        "microformat": {"microformatDataRenderer": {"tags": ["music", "pop"]}},
        "contents": {"twoColumnBrowseResultsRenderer": {"tabs": [
            {"tabRenderer": {"title": "Videos", "selected": true}},
            {"tabRenderer": {"title": "Shorts"}},
        ]}},
    })
}

/// Normalize tab metadata for oracle comparison (missing ≡ null; the
/// port omits unset fields instead of storing nulls).
fn youtube_tab_metadata_value(info: &yt_dlp_core::InfoDict) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for key in [
        "id",
        "title",
        "description",
        "channel",
        "channel_id",
        "channel_url",
        "uploader",
        "uploader_id",
        "uploader_url",
        "tags",
        "thumbnails",
        "availability",
        "channel_follower_count",
        "channel_is_verified",
        "view_count",
        "playlist_count",
        "modified_date",
    ] {
        map.insert(
            key.to_owned(),
            info.get(key).cloned().unwrap_or(serde_json::Value::Null),
        );
    }
    serde_json::Value::Object(map)
}

#[test]
fn youtube_tab_metadata_channel_matches_oracle() {
    let info = crate::native::youtube_tab_metadata("UCxxxx", &youtube_tab_channel_data());
    let value = youtube_tab_metadata_value(&info);
    assert_eq!(
        value,
        serde_json::json!({
            "availability": null,
            "channel": "Rick Astley",
            "channel_follower_count": 3100000,
            "channel_id": "UCuAXFkgpKOUxaRXCkBosP9w",
            "channel_is_verified": true,
            "channel_url": "https://www.youtube.com/channel/UCuAXFkgpKOUxaRXCkBosP9w",
            "description": "Official channel",
            "id": "UCuAXFkgpKOUxaRXCkBosP9w",
            "modified_date": null,
            "playlist_count": null,
            "tags": ["music", "pop"],
            "thumbnails": [
                {"height": 176, "url": "https://yt3.googleusercontent.com/avatar=s176", "width": 176},
                {"id": "avatar_uncropped", "preference": 1,
                 "url": "https://yt3.googleusercontent.com/avatar=s0"},
                {"height": 248, "preference": -10,
                 "url": "https://yt3.googleusercontent.com/banner", "width": 1280},
                {"id": "banner_uncropped", "preference": -5,
                 "url": "https://yt3.googleusercontent.com/banner=s0"},
            ],
            "title": "Rick Astley",
            "uploader": "Rick Astley",
            "uploader_id": "@RickAstley",
            "uploader_url": "https://www.youtube.com/@RickAstley",
            "view_count": null,
        })
    );
}

fn youtube_tab_playlist_data() -> serde_json::Value {
    serde_json::json!({
        "metadata": {"playlistMetadataRenderer": {"title": "My mix"}},
        "header": {"playlistHeaderRenderer": {
            "playlistId": "PLtest123",
            "privacy": "PUBLIC",
            "byline": [
                {"playlistBylineRenderer": {"text": {"runs": [{"text": "10 videos"}]}}},
                {"playlistBylineRenderer": {"text": {"runs": [{"text": "Updated 2 days ago"}]}}},
            ],
            "viewCountText": {"simpleText": "1M views"},
            "ownerText": {"runs": [{
                "text": "by Owner and 3 others",
                "navigationEndpoint": {"browseEndpoint": {
                    "browseId": "UCuAXFkgpKOUxaRXCkBosP9w",
                    "canonicalBaseUrl": "/@Owner"}}}]},
            "playlistHeaderBanner": {"heroPlaylistThumbnailRenderer": {"thumbnail": {"thumbnails": [
                {"url": "https://i.ytimg.com/vi/x/hqdefault.jpg", "width": 480, "height": 360},
            ]}}},
        }},
        "sidebar": {"playlistSidebarRenderer": {"items": [
            {"playlistSidebarPrimaryInfoRenderer": {
                "title": {"runs": [{"text": "My mix"}]},
                "stats": [
                    {"simpleText": "10 videos"},
                    {"simpleText": "1M views"},
                    {"simpleText": "Updated yesterday"},
                ],
                "badges": [{"metadataBadgeRenderer": {"icon": {"iconType": "PRIVACY_PUBLIC"}}}],
                "thumbnailRenderer": {"playlistVideoThumbnailRenderer": {"thumbnail": {"thumbnails": [
                    {"url": "https://i.ytimg.com/vi/y/hqdefault.jpg", "width": 480, "height": 360},
                ]}}},
            }},
        ]}},
        "microformat": {"microformatDataRenderer": {}},
    })
}

/// Yesterday's `YYYYMMDD` from the system clock (the oracle pins the same
/// relative shape; the absolute stamp depends on the run date).
fn youtube_tab_yesterday_yyyymmdd() -> String {
    let yesterday: i64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs() as i64 / 86_400)
        .unwrap_or(0)
        - 1;
    // March-based civil conversion for the test clock only.
    let shifted = yesterday + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    format!("{:04}{:02}{:02}", year + i64::from(month <= 2), month, day)
}

#[test]
fn youtube_tab_metadata_playlist_matches_oracle() {
    let info = crate::native::youtube_tab_metadata("PLtest123", &youtube_tab_playlist_data());
    let value = youtube_tab_metadata_value(&info);
    let mut expected = serde_json::json!({
        "availability": "public",
        "channel": "Owner",
        "channel_follower_count": null,
        "channel_id": "UCuAXFkgpKOUxaRXCkBosP9w",
        "channel_is_verified": null,
        "channel_url": "https://www.youtube.com/channel/UCuAXFkgpKOUxaRXCkBosP9w",
        "description": "",
        "id": "PLtest123",
        "modified_date": null,
        "playlist_count": 10,
        "tags": [],
        "thumbnails": [
            {"height": 360, "url": "https://i.ytimg.com/vi/y/hqdefault.jpg", "width": 480},
        ],
        "title": "My mix",
        "uploader": "Owner",
        "uploader_id": "@Owner",
        "uploader_url": "https://www.youtube.com/@Owner",
        "view_count": 1000000,
    });
    // "Updated yesterday" resolves against the run date (oracle: 20260902).
    expected["modified_date"] = serde_json::json!(youtube_tab_yesterday_yyyymmdd());
    assert_eq!(value, expected);
}

#[test]
fn youtube_tab_metadata_edge_cases_match_oracle() {
    // Keywords split when microformat tags are absent.
    let info = crate::native::youtube_tab_metadata(
        "UCxxxx",
        &serde_json::json!({"metadata": {"channelMetadataRenderer": {
            "title": "Kw Channel",
            "externalId": "UCuAXFkgpKOUxaRXCkBosP9w",
            "keywords": "rick astley \"never gonna\" music",
        }}}),
    );
    assert_eq!(
        info.get("tags"),
        Some(&serde_json::json!([
            "rick",
            "astley",
            "never gonna",
            "music"
        ]))
    );
    assert_eq!(info.get_str("channel"), Some("Kw Channel"));
    // Secondary sidebar owner fills channel fields when ownerText is absent.
    let info = crate::native::youtube_tab_metadata(
        "PLsecondary",
        &serde_json::json!({
            "metadata": {"playlistMetadataRenderer": {"title": "Secondary mix"}},
            "header": {"playlistHeaderRenderer": {"playlistId": "PLsecondary"}},
            "sidebar": {"playlistSidebarRenderer": {"items": [
                {"playlistSidebarSecondaryInfoRenderer": {"videoOwner": {"videoOwnerRenderer": {
                    "title": {"runs": [{"text": "Secondary Owner"}]}}}}},
            ]}},
        }),
    );
    assert_eq!(info.get("tags"), Some(&serde_json::json!([])));
    assert_eq!(info.get_str("channel"), Some("Secondary Owner"));
    assert_eq!(info.get_str("uploader"), Some("Secondary Owner"));
    assert!(info.get("channel_id").is_none());
}

#[test]
fn youtube_tab_availability_matches_oracle() {
    let availability = |data: serde_json::Value| crate::native::youtube_tab_availability(&data);
    assert_eq!(availability(serde_json::json!({})), None);
    // Sidebar badges never count (tuple-key lookup swallows TypeError).
    assert_eq!(
        availability(
            serde_json::json!({"sidebar": {"playlistSidebarRenderer": {"items": [
            {"playlistSidebarPrimaryInfoRenderer": {
                "badges": [{"metadataBadgeRenderer": {"icon": {"iconType": "PRIVACY_PRIVATE"}}}]}}]}}})
        ),
        None
    );
    assert_eq!(
        availability(
            serde_json::json!({"header": {"playlistHeaderRenderer": {"privacy": "PRIVATE"}}})
        ),
        Some("private".to_owned())
    );
    assert_eq!(
        availability(
            serde_json::json!({"header": {"playlistHeaderRenderer": {"privacy": "UNLISTED"}}})
        ),
        Some("unlisted".to_owned())
    );
    assert_eq!(
        availability(
            serde_json::json!({"microformat": {"microformatDataRenderer": {"unlisted": true}}})
        ),
        Some("unlisted".to_owned())
    );
    assert_eq!(
        availability(
            serde_json::json!({"microformat": {"microformatDataRenderer": {"noindex": false}}})
        ),
        Some("public".to_owned())
    );
    assert_eq!(
        availability(
            serde_json::json!({"header": {"playlistHeaderRenderer": {"privacyForm": {
            "dropdownFormFieldRenderer": {"dropdown": {"dropdownRenderer": {"entries": [
                {"privacyDropdownItemRenderer": {
                    "isSelected": true, "icon": {"iconType": "PRIVACY_PUBLIC"}}},
            ]}}}}}}})
        ),
        Some("public".to_owned())
    );
}

#[test]
fn youtube_tab_time_text_matches_oracle() {
    // Fixed civil pairs (oracle-pinned): epoch 0 and absolute-date shapes.
    assert_eq!(
        crate::native::youtube_upload_date(0),
        Some("19700101".to_owned())
    );
    assert_eq!(
        crate::native::youtube_upload_date(1788307200),
        Some("20260902".to_owned())
    );
    assert_eq!(
        crate::native::youtube_upload_date(1704412800),
        Some("20240105".to_owned())
    );
    // Garbage stays unset (absolute parsing is a TODO).
    assert_eq!(
        crate::native::youtube_parse_time_text("not a date at all"),
        None
    );
    // "Updated yesterday" lands within a day of now and formats as yesterday.
    let timestamp =
        crate::native::youtube_parse_time_text("Updated yesterday").expect("relative parses");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs() as i64)
        .unwrap_or(0);
    assert!((86399..=86401).contains(&(now - timestamp)));
    assert_eq!(
        crate::native::youtube_upload_date(timestamp),
        Some(youtube_tab_yesterday_yyyymmdd())
    );
}

#[test]
fn youtube_tab_split_words_matches_oracle() {
    assert_eq!(
        crate::native::youtube_split_words("rick astley \"never gonna\" music"),
        ["rick", "astley", "never gonna", "music"]
    );
    assert_eq!(
        crate::native::youtube_split_words("  a  'b c'  "),
        ["a", "b c"]
    );
    assert!(crate::native::youtube_split_words("").is_empty());
}

#[test]
fn youtube_tab_ucid_handle_extras_match_oracle() {
    // Full channel URLs do not match (verified against the source);
    // bare `/UC...` paths do.
    assert_eq!(
        crate::native::youtube_ucid_from_url(Some(
            "https://www.youtube.com/channel/UCuAXFkgpKOUxaRXCkBosP9w"
        )),
        None
    );
    assert_eq!(
        crate::native::youtube_ucid_from_url(Some("/UCuAXFkgpKOUxaRXCkBosP9w")).as_deref(),
        Some("UCuAXFkgpKOUxaRXCkBosP9w")
    );
    assert_eq!(
        crate::native::youtube_ucid_from_url(Some("https://example.test/UCxxxx")),
        None
    );
    assert_eq!(crate::native::youtube_ucid_from_url(None), None);
    assert_eq!(
        crate::native::youtube_handle_or_none(Some("@RickAstley")).as_deref(),
        Some("@RickAstley")
    );
    assert_eq!(
        crate::native::youtube_handle_or_none(Some("https://www.youtube.com/@RickAstley/videos")),
        None
    );
    assert_eq!(crate::native::youtube_handle_or_none(None), None);
}

fn youtube_tab_continuation_response(items: Vec<serde_json::Value>) -> serde_json::Value {
    serde_json::json!({"onResponseReceivedActions": [{"appendContinuationItemsAction": {
        "continuationItems": items}}]})
}

fn youtube_tab_page(video_ids: &[&str], token: Option<&str>) -> serde_json::Value {
    let mut items: Vec<serde_json::Value> = video_ids
        .iter()
        .map(|id| {
            serde_json::json!({"playlistVideoRenderer": {
                "videoId": id,
                "title": {"runs": [{"text": format!("V{id}")}]},
                "lengthText": {"simpleText": "1:00"},
                "navigationEndpoint": {
                    "commandMetadata": {"webCommandMetadata": {"url": format!("/watch?v={id}")}}},
                "thumbnail": {"thumbnails": []},
            }})
        })
        .collect();
    if let Some(token) = token {
        items.push(
            serde_json::json!({"continuationItemRenderer": {"continuationEndpoint": {
            "continuationCommand": {"token": token}}}}),
        );
    }
    youtube_tab_continuation_response(items)
}

fn youtube_tab_first_rich(video_id: &str, token: &str) -> serde_json::Value {
    serde_json::json!({"content": {"richGridRenderer": {"contents": [
        {"richItemRenderer": {"content": {"videoRenderer": {
            "videoId": video_id,
            "title": {"runs": [{"text": "A0"}]},
            "lengthText": {"simpleText": "1:00"},
            "navigationEndpoint": {
                "commandMetadata": {"webCommandMetadata": {"url": format!("/watch?v={video_id}")}}},
            "thumbnail": {"thumbnails": []},
        }}}},
        {"continuationItemRenderer": {"continuationEndpoint": {
            "continuationCommand": {"token": token}}}},
    ]}}})
}

#[test]
fn youtube_tab_collect_entries_matches_oracle() {
    use std::collections::HashMap;
    // Unwrapped playlist pages refetch on a repeated token, then stop.
    let mut pages = HashMap::new();
    pages.insert("P2".to_owned(), youtube_tab_page(&["b1", "b2"], Some("P3")));
    pages.insert("P3".to_owned(), youtube_tab_page(&["b1", "b2"], Some("P3")));
    let mut fetch = |query: &serde_json::Value, _page: u32| {
        query
            .get("continuation")
            .and_then(|token| token.as_str())
            .and_then(|token| pages.get(token))
            .cloned()
    };
    let first = crate::native::youtube_tab_first_page(&youtube_tab_first_rich("a0", "P2"));
    let entries = crate::native::youtube_collect_tab_entries(first, &mut fetch);
    assert_eq!(
        youtube_tab_entry_ids(&entries),
        ["a0", "b1", "b2", "b1", "b2"]
    );

    // Section-wrapped pages dispatch back through section contents.
    let section = youtube_tab_continuation_response(vec![
        serde_json::json!({"itemSectionRenderer": {"contents": [{
                "videoRenderer": {
                    "videoId": "c1",
                    "title": {"runs": [{"text": "C1"}]},
                    "lengthText": {"simpleText": "2:00"},
                    "navigationEndpoint": {"commandMetadata":
                        {"webCommandMetadata": {"url": "/watch?v=c1"}}}},
                    "thumbnail": {"thumbnails": []},
        }}]}})),
        serde_json::json!({"continuationItemRenderer": {"continuationEndpoint": {
            "continuationCommand": {"token": "P3"}}}}),
    ]);
    let mut pages = HashMap::new();
    pages.insert("P2".to_owned(), section.clone());
    pages.insert("P3".to_owned(), section);
    let mut fetch = |query: &serde_json::Value, _page: u32| {
        query
            .get("continuation")
            .and_then(|token| token.as_str())
            .and_then(|token| pages.get(token))
            .cloned()
    };
    let first = crate::native::youtube_tab_first_page(&youtube_tab_first_rich("a0", "P2"));
    let entries = crate::native::youtube_collect_tab_entries(first, &mut fetch);
    assert_eq!(youtube_tab_entry_ids(&entries), ["a0", "c1", "c1"]);

    // Empty responses end pagination after the first page.
    let mut fetch = |_query: &serde_json::Value, _page: u32| Some(serde_json::json!({}));
    let first = crate::native::youtube_tab_first_page(&youtube_tab_first_rich("a0", "P2"));
    let entries = crate::native::youtube_collect_tab_entries(first, &mut fetch);
    assert_eq!(youtube_tab_entry_ids(&entries), ["a0"]);

    // Bare videoRenderer continuation items extract like grid members.
    let bare = youtube_tab_continuation_response(vec![
        serde_json::json!({"videoRenderer": {
            "videoId": "d1",
            "title": {"runs": [{"text": "D1"}]},
            "lengthText": {"simpleText": "4:00"},
            "navigationEndpoint": {"commandMetadata":
                {"webCommandMetadata": {"url": "/watch?v=d1"}}},
            "thumbnail": {"thumbnails": []},
        }}),
    ]);
    let mut pages = HashMap::new();
    pages.insert("P2".to_owned(), bare);
    let mut fetch = |query: &serde_json::Value, _page: u32| {
        query
            .get("continuation")
            .and_then(|token| token.as_str())
            .and_then(|token| pages.get(token))
            .cloned()
    };
    let first = crate::native::youtube_tab_first_page(&youtube_tab_first_rich("a0", "P2"));
    let entries = crate::native::youtube_collect_tab_entries(first, &mut fetch);
    assert_eq!(youtube_tab_entry_ids(&entries), ["a0", "d1"]);

    // Wrapped playlist continuations degrade gracefully (the source raises
    // TypeError here): no entries, pagination ends.
    let wrapped = youtube_tab_continuation_response(vec![
        serde_json::json!({"playlistVideoListContinuation": {
            "contents": [],
            "continuations": [{"nextContinuationData": {"continuation": "P3"}}],
        }}),
    ]);
    let mut pages = HashMap::new();
    pages.insert("P2".to_owned(), wrapped);
    let mut fetch = |query: &serde_json::Value, _page: u32| {
        query
            .get("continuation")
            .and_then(|token| token.as_str())
            .and_then(|token| pages.get(token))
            .cloned()
    };
    let first = crate::native::youtube_tab_first_page(&youtube_tab_first_rich("a0", "P2"));
    let entries = crate::native::youtube_collect_tab_entries(first, &mut fetch);
    assert_eq!(youtube_tab_entry_ids(&entries), ["a0"]);
}

#[test]
fn youtube_tab_playlist_composes_first_page_matches_oracle() {
    let data = youtube_tab_channel_data();
    let tabs = [
        serde_json::json!({"title": "Videos", "selected": true, "content": {"sectionListRenderer": {
            "contents": [{"itemSectionRenderer": {"contents": [
                youtube_tab_video_fixture("f1", "F1"),
            ]}}],
        }}}),
    ];
    let result = crate::native::youtube_tab_playlist("UCxxxx", &data, &tabs).expect("tab playlist");
    let crate::ExtractorResult::Playlist { info, entries } = result else {
        panic!("expected playlist result");
    };
    // Selected-tab title suffix on the channel metadata title.
    assert_eq!(info.get_str("title"), Some("Rick Astley - Videos"));
    assert_eq!(info.get_str("id"), Some("UCuAXFkgpKOUxaRXCkBosP9w"));
    assert_eq!(youtube_tab_entry_ids(&entries), ["f1"]);
    // Missing selection stays fatal with the source message.
    let error = crate::native::youtube_tab_playlist("UCxxxx", &data, &[]).unwrap_err();
    assert_eq!(error.message, "Unable to find selected tab");
}
