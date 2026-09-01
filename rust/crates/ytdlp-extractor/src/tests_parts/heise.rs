#[test]
fn heise_native_extractor_maps_videoout_xml_sources() {
    let extractor = HeiseExtractor::new(ExtractorDescriptor::new(
        "HeiseIE",
        "Heise",
        r#"https?://(?:www\.)?heise\.de/(?:[^/]+/)+[^/]+-(?P<id>[0-9]+)\.html"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "/videout/feed".to_owned(),
                br#"<rss><channel>
                    <image>https://cdn.example/heise.jpg</image>
                    <source label="360p" file="https://cdn.example/heise-360.mp4"/>
                    <source label="720p" file="https://cdn.example/heise-720.m3u8"/>
                </channel></rss>"#
                    .to_vec(),
            ),
            (
                "/video/native-123456.html".to_owned(),
                br#"<html><head>
                    <meta name="fulltitle" content="Native Heise video">
                    <meta property="og:description" content="Native Heise description">
                    <meta name="date" content="2017-12-08">
                </head><body>
                    <div class="videoplayerjw" data-container="12" data-sequenz="34"></div>
                </body></html>"#
                    .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.heise.de/video/native-123456.html",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("123456"));
    assert_eq!(result.get_str("title"), Some("Native Heise video"));
    assert_eq!(
        result.get_str("description"),
        Some("Native Heise description")
    );
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/heise.jpg")
    );
    assert_eq!(result.get("timestamp"), Some(&serde_json::json!(1512691200)));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/heise-360.mp4")
    );
    assert_eq!(result.get_str("ext"), Some("mp4"));
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
            .and_then(|formats| formats.get(1))
            .and_then(|format| format.get("protocol")),
        Some(&serde_json::json!("m3u8_native"))
    );
}

#[test]
fn heise_native_extractor_marks_kaltura_as_todo() {
    let extractor = HeiseExtractor::new(ExtractorDescriptor::new(
        "HeiseIE",
        "Heise",
        r#"https?://(?:www\.)?heise\.de/(?:[^/]+/)+[^/]+-(?P<id>[0-9]+)\.html"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<div class="videoplayerjw" entry-id="native-kaltura"></div>"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context("https://www.heise.de/video/native-123456.html", &context)
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}

#[test]
fn heise_native_extractor_marks_youtube_as_todo() {
    let extractor = HeiseExtractor::new(ExtractorDescriptor::new(
        "HeiseIE",
        "Heise",
        r#"https?://(?:www\.)?heise\.de/(?:[^/]+/)+[^/]+-(?P<id>[0-9]+)\.html"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<iframe src="https://www.youtube.com/embed/native-youtube"></iframe>"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context("https://www.heise.de/video/native-123456.html", &context)
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}
