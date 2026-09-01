#[test]
fn mocha_native_extractor_maps_api_metadata_and_media_variants() {
    let extractor = MochaVideoExtractor::new(ExtractorDescriptor::new(
        "MochaVideoIE",
        "MochaVideo",
        r#"https?://video\.mocha\.com\.vn/(?P<video_slug>[\w-]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "/getVideoDetail".to_owned(),
            r#"{"data":{"videoDetail":{"id":18694039,"slug":"native-mocha","name":"Native Mocha","description":"A native description","durationS":70,"total_view":11,"total_like":7,"total_unlike":1,"total_comment":3,"image_path_thumb":"https://cdn.example/mocha.jpg","publish_time":1652254203000,"isLive":false,"channels":[{"id":42,"name":"Kids","numfollow":99}],"categories":[{"categoryname":"Kids"}],"list_resolution":[{"resolution":"720p","video_path":"https://cdn.example/mocha/master.m3u8"}],"original_path":"https://cdn.example/mocha/original.mp4"}}}"#.as_bytes().to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "http://video.mocha.com.vn/native-mocha-v18694039",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("18694039"));
    assert_eq!(result.get_str("display_id"), Some("native-mocha"));
    assert_eq!(result.get_str("title"), Some("Native Mocha"));
    assert_eq!(result.get_i64("duration"), Some(70));
    assert_eq!(result.get_i64("timestamp"), Some(1652254203));
    assert_eq!(result.get_str("channel"), Some("Kids"));
    assert_eq!(result.get_i64("channel_id"), Some(42));
    assert_eq!(result.get("categories"), Some(&serde_json::json!(["Kids"])));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
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
