#[test]
fn dlive_vod_native_extractor_maps_graphql_broadcast() {
    let extractor = DliveExtractor::new(ExtractorDescriptor::new(
        "DLiveVODIE",
        "dlive:vod",
        r"https?://(?:www\.)?dlive\.tv/p/(?P<uploader_id>.+?)\+(?P<id>[^/?#&]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{"data":{"pastBroadcast":{"content":"Native VOD description","createdAt":1700000000123,"length":42,"playbackUrl":"https://cdn.example/dlive-vod.m3u8","title":"Native VOD","thumbnailUrl":"https://cdn.example/vod.jpg","viewCount":17}}}"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://dlive.tv/p/native-user+native-vod", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("native-vod"));
    assert_eq!(result.get_str("title"), Some("Native VOD"));
    assert_eq!(result.get_str("uploader_id"), Some("native-user"));
    assert_eq!(result.get_str("description"), Some("Native VOD description"));
    assert_eq!(result.get_str("thumbnail"), Some("https://cdn.example/vod.jpg"));
    assert_eq!(result.get_i64("timestamp"), Some(1_700_000_000));
    assert_eq!(result.get_i64("view_count"), Some(17));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/dlive-vod.m3u8"));
}

#[test]
fn dlive_stream_native_extractor_maps_live_user() {
    let extractor = DliveExtractor::new(ExtractorDescriptor::new(
        "DLiveStreamIE",
        "dlive:stream",
        r"https?://(?:www\.)?dlive\.tv/(?!p/)(?P<id>[\w.-]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{"data":{"userByDisplayName":{"livestream":{"content":"Native live description","createdAt":1700000000123,"title":"Native live","thumbnailUrl":"https://cdn.example/live.jpg","watchingCount":19},"username":"nativeuser"}}}"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://dlive.tv/native-display", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("native-display"));
    assert_eq!(result.get_str("title"), Some("Native live"));
    assert_eq!(result.get_str("uploader"), Some("native-display"));
    assert_eq!(result.get_str("uploader_id"), Some("nativeuser"));
    assert_eq!(result.get_str("url"), Some("https://live.prd.dlive.tv/hls/live/nativeuser.m3u8"));
    assert_eq!(result.get("is_live"), Some(&serde_json::json!(true)));
    assert_eq!(result.get_i64("view_count"), Some(19));
}
