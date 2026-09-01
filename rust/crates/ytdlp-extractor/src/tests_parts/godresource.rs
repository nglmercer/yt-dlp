#[test]
fn godresource_native_extractor_maps_hls_stream_metadata() {
    let extractor = GodResourceExtractor::new(ExtractorDescriptor::new(
        "GodResourceIE",
        "GodResource",
        r#"https?://new\.godresource\.com/video/(?P<id>\w+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{"streamUrl":"https://cdn.example/native/master.m3u8",
            "isLive":true,"title":"Native GodResource video",
            "thumbnail":"https://cdn.example/native.jpg","views":99,
            "channelName":"Native Channel","channelId":5,
            "streamDateCreated":"2024-03-20T12:31:06Z",
            "streamDataModified":"2024-03-20T12:32:06Z"}"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://new.godresource.com/video/A01mTKjyf6w", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("A01mTKjyf6w"));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/native/master.m3u8")
    );
    assert_eq!(result.get_str("title"), Some("Native GodResource video"));
    assert_eq!(result.get_str("channel"), Some("Native Channel"));
    assert_eq!(result.get_str("channel_id"), Some("5"));
    assert_eq!(result.get_i64("view_count"), Some(99));
    assert_eq!(result.get_bool("is_live"), Some(true));
    assert_eq!(result.get_i64("timestamp"), Some(1_710_937_866));
    assert_eq!(result.get_i64("modified_timestamp"), Some(1_710_937_926));
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
fn godresource_native_extractor_maps_mp4_stream() {
    let extractor = GodResourceExtractor::new(ExtractorDescriptor::new(
        "GodResourceIE",
        "GodResource",
        r#"https?://new\.godresource\.com/video/(?P<id>\w+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{"streamUrl":"https://cdn.example/native/video.mp4",
            "isLive":false,"title":"Native MP4 video"}"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://new.godresource.com/video/01DXmBbQv_X", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("url"), Some("https://cdn.example/native/video.mp4"));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("protocol"))
            .and_then(serde_json::Value::as_str),
        Some("http")
    );
}
