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
        assert_eq!(crate::native::youtube_video_id(url).as_deref(), Some(YOUTUBE_FIXTURE_ID));
    }
    assert!(crate::native::youtube_video_id("https://www.youtube.com/playlist?list=PLfixture").is_none());
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
    assert_eq!(result.get_str("thumbnail"), Some("https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg"));
    let formats = result.get("formats").and_then(serde_json::Value::as_array).unwrap();
    assert_eq!(formats.len(), 3);
    assert_eq!(formats[0].get("format_id"), Some(&serde_json::json!("18")));
    assert_eq!(formats[0].get("vcodec"), Some(&serde_json::json!("avc1.42001E")));
    assert_eq!(formats[1].get("acodec"), Some(&serde_json::json!("none")));
    assert_eq!(formats[2].get("vcodec"), Some(&serde_json::json!("none")));
    assert_eq!(formats[2].get("language"), Some(&serde_json::json!("en")));
    assert!(result
        .get("subtitles")
        .and_then(|value| value.get("en"))
        .is_some());
    assert!(result
        .get("automatic_captions")
        .and_then(|value| value.get("es"))
        .is_some());
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
    assert_eq!(result.get("formats").and_then(serde_json::Value::as_array).map(Vec::len), Some(3));
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
    let (formats, todos) = crate::native::youtube_formats_and_todos(&[response]);
    assert_eq!(formats.len(), 2);
    assert!(formats
        .iter()
        .any(|format| format.get("rust_todo").is_some()));
    assert!(todos.iter().any(|todo| todo.contains("signatureCipher")));
    assert!(todos.iter().any(|todo| todo.contains("n challenge")));
}
