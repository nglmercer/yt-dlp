#[test]
fn hbo_native_extractor_maps_page_state_and_xml_sources() {
    let extractor = HboExtractor::new(ExtractorDescriptor::new(
        "HBOIE",
        "hbo",
        r#"https?://(?:www\.)?hbo\.com/(?:video|embed)(?:/[^/]+)*/(?P<id>[^/?#]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "/video/native.xml".to_owned(),
                br#"<video>
                    <id>22113301</id>
                    <title>Native Episode</title>
                    <program>Native Series</program>
                    <duration><tv14>01:02</tv14></duration>
                    <videos><sources>
                        <size width="1920"><path>https://cdn.example/hbo/native.mp4</path></size>
                        <hls>https://cdn.example/hbo/native.tar</hls>
                    </sources></videos>
                    <titleCardSizes>
                        <size width="640"><path>https://cdn.example/hbo/card.jpg</path></size>
                    </titleCardSizes>
                    <captionUrl>https://cdn.example/hbo/native.ttml</captionUrl>
                </video>"#
                    .to_vec(),
            ),
            (
                "hbo.com/video/native".to_owned(),
                br#"<div data-state='{"video":{"locationUrl":"/video/native.xml"}}'></div>"#
                    .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://www.hbo.com/video/native", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("22113301"));
    assert_eq!(result.get_str("title"), Some("Native Series - Native Episode"));
    assert_eq!(result.get_str("series"), Some("Native Series"));
    assert_eq!(result.get_str("episode"), Some("Native Episode"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(62.0)));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/hbo/native.mp4")
    );
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/hbo/card.jpg")
    );
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
            .and_then(|format| format.get("url")),
        Some(&serde_json::json!("https://cdn.example/hbo/native/base_index.m3u8"))
    );
    assert_eq!(
        result
            .get("subtitles")
            .and_then(|subtitles| subtitles.get("en"))
            .and_then(serde_json::Value::as_array)
            .and_then(|entries| entries.first())
            .and_then(|entry| entry.get("ext")),
        Some(&serde_json::json!("ttml"))
    );
}

#[test]
fn hbo_native_extractor_requires_page_location_state() {
    let extractor = HboExtractor::new(ExtractorDescriptor::new(
        "HBOIE",
        "hbo",
        r#"https?://(?:www\.)?hbo\.com/(?:video|embed)(?:/[^/]+)*/(?P<id>[^/?#]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: b"<html><body>No player state</body></html>".to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context("https://www.hbo.com/video/native", &context)
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Extraction);
    assert!(error.message.contains("no page state"));
}
