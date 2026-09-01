struct LocoHandler;

impl RequestHandler for LocoHandler {
    fn name(&self) -> &str {
        "loco-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        if request.url().contains("loco.com/streamers/")
            || request.url().contains("loco.com/stream/")
        {
            let body = br#"<script id="__NEXT_DATA__">{
                "props": {
                    "pageProps": {
                        "ssrData": {
                            "liveStreamData": {
                                "stream": {
                                    "uid": "native-stream",
                                    "conf": {"hls": "https://cdn.example/loco/live.m3u8"},
                                    "title": "Native Loco stream",
                                    "description": "Native Loco description",
                                    "game_name": "Native Game",
                                    "user_uid": "native-user",
                                    "alias": "native-channel",
                                    "viewersCurrent": 12,
                                    "total_views": 34,
                                    "thumbnail_url_small": "//cdn.example/loco/thumb.jpg",
                                    "likes": 5,
                                    "tags": ["native", "gaming"],
                                    "started_at": 1704164645000,
                                    "updated_at": 1704168245000,
                                    "comments_count": 6,
                                    "followers_count": 7,
                                    "duration": 321
                                }
                            }
                        }
                    }
                }
            }</script>"#;
            return Ok(Response::new(
                request.url(),
                200,
                "OK",
                body.to_vec(),
            ));
        }
        if request
            .url()
            .contains("api.getloconow.com/v3/user/device_profile/")
        {
            return Ok(Response::new(
                request.url(),
                200,
                "OK",
                br#"{"access_token":"native-access-token"}"#.to_vec(),
            ));
        }
        if request.url().contains("drm.loco.com/v1/streams/playback/") {
            return Ok(Response::new(
                request.url(),
                200,
                "OK",
                br#"{}"#.to_vec(),
            ));
        }
        Err(RequestError::new(
            ErrorKind::Transport,
            format!("no Loco route for {}", request.url()),
        ))
    }
}

fn loco_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(LocoHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

fn loco_extractor() -> LocoExtractor {
    LocoExtractor::new(ExtractorDescriptor::new(
        "LocoIE",
        "Loco",
        r"https?://(?:www\.)?loco\.com/(?P<type>streamers|stream)/(?P<id>[^/?#]+)",
        true,
    ))
    .unwrap()
}

#[test]
fn loco_native_extractor_maps_live_stream_and_authorization() {
    let result = loco_extractor()
        .extract_with_context(
            "https://loco.com/streamers/native-channel",
            &loco_context(),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_str("id"), Some("native-channel"));
    assert_eq!(result.get_str("title"), Some("Native Loco stream"));
    assert_eq!(result.get_str("description"), Some("Native Loco description"));
    assert_eq!(result.get_str("series"), Some("Native Game"));
    assert_eq!(result.get_str("uploader_id"), Some("native-user"));
    assert_eq!(result.get_str("channel"), Some("native-channel"));
    assert_eq!(result.get_i64("concurrent_view_count"), Some(12));
    assert_eq!(result.get_i64("view_count"), Some(34));
    assert_eq!(result.get_i64("timestamp"), Some(1_704_164_645));
    assert_eq!(result.get_i64("modified_timestamp"), Some(1_704_168_245));
    assert_eq!(result.get_bool("is_live"), Some(true));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/loco/thumb.jpg")
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("protocol")),
        Some(&serde_json::json!("m3u8_native"))
    );
}

#[test]
fn loco_native_extractor_marks_vod_as_not_live() {
    let result = loco_extractor()
        .extract_with_context(
            "https://loco.com/stream/native-vod",
            &loco_context(),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_str("id"), Some("native-vod"));
    assert_eq!(result.get_bool("is_live"), Some(false));
    assert_eq!(result.get_i64("duration"), Some(321));
}
