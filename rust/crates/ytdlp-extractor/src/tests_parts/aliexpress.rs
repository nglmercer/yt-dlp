#[test]
fn aliexpress_live_native_extractor_maps_run_parameters_and_hls() {
    let extractor = AliExpressLiveExtractor::new(ExtractorDescriptor::new(
        "AliExpressLiveIE",
        "AliExpressLive",
        r"https?://live\.aliexpress\.com/live/(?P<id>\d+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "live.aliexpress.com/live/2800002704436634".to_owned(),
            br#"<html><script>
                var runParams = {
                    "title": "Native AliExpress Live",
                    "replyStreamUrl": "//cdn.example/aliexpress/live.m3u8",
                    "coverUrl": "https://cdn.example/aliexpress/poster.jpg",
                    "followBar": {"name": "Native Store"},
                    "startTimeLong": 1500717600000
                }; var next = true;
            </script></html>"#
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://live.aliexpress.com/live/2800002704436634",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("2800002704436634"));
    assert_eq!(result.get_str("title"), Some("Native AliExpress Live"));
    assert_eq!(result.get_str("uploader"), Some("Native Store"));
    assert_eq!(result.get("timestamp"), Some(&serde_json::json!(1500717600i64)));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/aliexpress/poster.jpg")
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/aliexpress/live.m3u8")
    );
    assert_eq!(result.get("is_live"), Some(&serde_json::json!(true)));
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
fn aliexpress_live_native_extractor_marks_non_hls_as_todo() {
    let extractor = AliExpressLiveExtractor::new(ExtractorDescriptor::new(
        "AliExpressLiveIE",
        "AliExpressLive",
        r"https?://live\.aliexpress\.com/live/(?P<id>\d+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"var runParams = {"replyStreamUrl":"https://cdn.example/live.mp4"};"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context("https://live.aliexpress.com/live/1", &context)
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}
