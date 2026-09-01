struct KickHandler;

impl RequestHandler for KickHandler {
    fn name(&self) -> &str {
        "kick-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        let url = request.url();
        let body = if url.contains("/v2/channels/native-channel") {
            r#"{"id":32807,"name":"Native Channel","user":{"id":33057,"username":"native-user","bio":"Native Kick bio"},"playback_url":"https://cdn.example/kick-live.m3u8","livestream":{"slug":"native-live","session_title":"Native Kick live","created_at":"2024-01-02T03:04:05Z","start_time":"2024-01-02T02:00:00Z","thumbnail":{"url":"https://cdn.example/kick-live.jpg"},"viewer_count":99,"is_mature":true,"channel_id":32807},"recent_categories":[{"name":"Gaming"}]}"#
        } else if url.contains("/v1/video/11111111-1111-4111-8111-111111111111") {
            r#"{"source":"https://cdn.example/kick-vod.m3u8","created_at":"2024-01-03T03:04:05Z","views":42,"livestream":{"session_title":"Native Kick VOD","duration":321500,"is_mature":false,"is_live":false,"thumbnail":"https://cdn.example/kick-vod.jpg","categories":[{"name":"News"}],"channel":{"id":32807,"slug":"native-channel","user_id":33057,"user":{"id":33057,"username":"native-user","bio":"Native Kick bio"}}}}"#
        } else if url.contains("/v2/clips/clip_native/play") {
            r#"{"clip":{"clip_url":"https://cdn.example/kick-clip.mp4","title":"Native Kick clip","channel":{"id":32807,"slug":"native-channel"},"creator":{"id":33057,"username":"native-creator"},"thumbnail_url":"https://cdn.example/kick-clip.jpg","duration":35,"category":{"name":"Gaming"},"created_at":"2024-01-04T03:04:05Z","views":12,"likes":3,"is_mature":true}}"#
        } else {
            return Err(RequestError::new(
                ErrorKind::Transport,
                format!("no Kick route for {url}"),
            ));
        };
        Ok(Response::new(url, 200, "OK", body.as_bytes().to_vec()))
    }
}

fn kick_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(KickHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

#[test]
fn kick_live_native_extractor_maps_channel_playback_and_metadata() {
    let extractor = KickExtractor::new(ExtractorDescriptor::new(
        "KickIE",
        "Kick",
        r#"https?://(?:www\.)?kick\.com/(?!(?:video|categories|search|auth)(?:[/?#]|$))(?P<id>[\w-]+)"#,
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context("https://kick.com/native-channel", &kick_context())
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("native-live"));
    assert_eq!(result.get_str("title"), Some("Native Kick live"));
    assert_eq!(result.get_str("channel_id"), Some("32807"));
    assert_eq!(result.get_str("uploader"), Some("Native Channel"));
    assert_eq!(result.get("age_limit"), Some(&serde_json::json!(18)));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/kick-live.m3u8")
    );
    assert_eq!(result.get("is_live"), Some(&serde_json::json!(true)));
}

#[test]
fn kick_vod_native_extractor_maps_hls_and_millisecond_duration() {
    let extractor = KickVodExtractor::new(ExtractorDescriptor::new(
        "KickVODIE",
        "KickVOD",
        r#"https?://(?:www\.)?kick\.com/[\w-]+/videos/(?P<id>[\da-f]{8}-(?:[\da-f]{4}-){3}[\da-f]{12})"#,
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://kick.com/native-channel/videos/11111111-1111-4111-8111-111111111111",
            &kick_context(),
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("title"), Some("Native Kick VOD"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(321.5)));
    assert_eq!(result.get("view_count"), Some(&serde_json::json!(42)));
    assert_eq!(result.get("age_limit"), Some(&serde_json::json!(0)));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("protocol"))
            .and_then(serde_json::Value::as_str),
        Some("m3u8_native")
    );
}

#[test]
fn kick_clip_native_extractor_maps_direct_clip_metadata() {
    let extractor = KickClipExtractor::new(ExtractorDescriptor::new(
        "KickClipIE",
        "KickClip",
        r#"https?://(?:www\.)?kick\.com/[\w-]+(?:/clips/|/?\?(?:[^#]+&)?clip=)(?P<id>clip_[\w-]+)"#,
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://kick.com/native-channel/clips/clip_native",
            &kick_context(),
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("clip_native"));
    assert_eq!(result.get_str("title"), Some("Native Kick clip"));
    assert_eq!(result.get_str("uploader_id"), Some("33057"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(35.0)));
    assert_eq!(result.get("like_count"), Some(&serde_json::json!(3)));
    assert_eq!(result.get("age_limit"), Some(&serde_json::json!(18)));
    assert_eq!(result.get_str("ext"), Some("mp4"));
}
