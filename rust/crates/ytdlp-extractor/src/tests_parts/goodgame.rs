#[test]
fn goodgame_native_extractor_maps_live_api_and_hls() {
    let extractor = GoodGameExtractor::new(ExtractorDescriptor::new(
        "GoodGameIE",
        "GoodGame",
        r#"https?://goodgame\.ru/(?!channel/)(?P<id>[\w.*-]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{"streamkey":"native-key","status":true,
            "title":"Native GoodGame stream","channelkey":"NativeChannel",
            "id":7998,"link":"https://goodgame.ru/NativeChannel",
            "streamer":{"username":"NativeUser","id":2899},
            "preview":"//hls.goodgame.ru/previews/native.jpg",
            "viewers":42,"followers":1234,"adult":true}"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://goodgame.ru/NativeChannel", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("native-key"));
    assert_eq!(result.get_str("title"), Some("Native GoodGame stream"));
    assert_eq!(result.get_str("channel"), Some("NativeChannel"));
    assert_eq!(result.get_str("channel_id"), Some("7998"));
    assert_eq!(result.get_str("uploader"), Some("NativeUser"));
    assert_eq!(result.get_str("uploader_id"), Some("2899"));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://hls.goodgame.ru/previews/native.jpg")
    );
    assert_eq!(result.get_i64("concurrent_view_count"), Some(42));
    assert_eq!(result.get_i64("channel_follower_count"), Some(1234));
    assert_eq!(result.get_i64("age_limit"), Some(18));
    assert_eq!(
        result.get_str("url"),
        Some("https://hls.goodgame.ru/manifest/native-key_master.m3u8")
    );
    assert_eq!(result.get_bool("is_live"), Some(true));
}

#[test]
fn goodgame_native_extractor_reports_offline_channel() {
    let extractor = GoodGameExtractor::new(ExtractorDescriptor::new(
        "GoodGameIE",
        "GoodGame",
        r#"https?://goodgame\.ru/(?!channel/)(?P<id>[\w.*-]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{"streamkey":"offline-key","status":false}"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context("https://goodgame.ru/Offline", &context)
        .unwrap_err();

    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.contains("offline"));
}
