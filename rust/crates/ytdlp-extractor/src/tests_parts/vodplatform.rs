#[test]
fn vod_platform_native_extractor_maps_hidden_hls_input() {
    let extractor = VodPlatformExtractor::new(ExtractorDescriptor::new(
        "VODPlatformIE",
        "VODPlatform",
        r"https?://(?:(?:www\.)?vod-platform\.net|embed\.kwikmotion\.com)/[eE]mbed/(?P<id>[^/?#]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "vod-platform.net/embed/RufMcytHDolTH1MuKHY9Fw".to_owned(),
            r#"<html><head>
                <meta property="og:title" content="LBCi News_ &quot;Native title&quot;">
                <meta property="og:image" content="https://cdn.example/vod/poster.jpg">
            </head><body>
                <input type="hidden" name="HiddenmyhHlsLink" value="https://cdn.example/vod/master.m3u8">
                <input type="hidden" name="HiddenThumbnail" value="https://cdn.example/vod/hidden.jpg">
            </body></html>"#
                .as_bytes()
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "http://vod-platform.net/embed/RufMcytHDolTH1MuKHY9Fw",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("RufMcytHDolTH1MuKHY9Fw"));
    assert_eq!(
        result.get_str("title"),
        Some("LBCi News_ \"Native title\"")
    );
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/vod/hidden.jpg")
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/vod/master.m3u8")
    );
    let format = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .and_then(|formats| formats.first())
        .unwrap();
    assert_eq!(
        format.get("protocol"),
        Some(&serde_json::json!("m3u8_native"))
    );
}

#[test]
fn vod_platform_native_extractor_marks_wowza_manifest_as_todo() {
    let extractor = VodPlatformExtractor::new(ExtractorDescriptor::new(
        "VODPlatformIE",
        "VODPlatform",
        r"https?://vod-platform\.net/[eE]mbed/(?P<id>[^/?#]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<input type="hidden" name="HiddenmyDashLink" value="https://cdn.example/vod/video.smil">"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context("https://vod-platform.net/embed/native", &context)
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}
