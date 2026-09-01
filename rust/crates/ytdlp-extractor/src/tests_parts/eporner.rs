#[test]
fn eporner_native_extractor_maps_xhr_sources_hash_and_page_metadata() {
    let extractor = EpornerExtractor::new(ExtractorDescriptor::new(
        "EpornerIE",
        "Eporner",
        r"https?://(?:www\.)?eporner\.com/(?:(?:hd-porn|embed)/|video-)(?P<id>\w+)(?:/(?P<display_id>[\w-]+))?",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "xhr/video/nativeid".to_owned(),
                br#"{"available":true,"sources":{
                    "hls":{"master":{"src":"https://cdn.example/master.m3u8"}},
                    "mp4":{"720p":{"src":"https://cdn.example/720.mp4"},
                           "1080p60fps":{"src":"https://cdn.example/1080.mp4"}}
                }}"#
                .to_vec(),
            ),
            (
                "eporner.com/hd-porn/nativeid".to_owned(),
                br#"<html>
                    <meta property="og:title" content="Native Eporner">
                    <meta property="og:description" content="Native description">
                    <meta name="duration" content="1838">
                    <div id="cinemaviews1">12,345</div>
                    <script type="application/ld+json">{"description":"JSON description","thumbnailUrl":"https://cdn.example/poster.jpg"}</script>
                    <script>var hash = "0123456789abcdef0123456789abcdef";</script>
                    <a class="download-av1">AV1</a>
                </html>"#
                .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.eporner.com/hd-porn/nativeid/native-display/",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("nativeid"));
    assert_eq!(result.get_str("display_id"), Some("native-display"));
    assert_eq!(result.get_str("title"), Some("Native Eporner"));
    assert_eq!(result.get_str("description"), Some("JSON description"));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/poster.jpg")
    );
    assert_eq!(result.get_f64("duration"), Some(1838.0));
    assert_eq!(result.get_i64("view_count"), Some(12345));
    assert_eq!(result.get_i64("age_limit"), Some(18));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(5)
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.iter().find(|format| {
                format.get("format_id") == Some(&serde_json::json!("1080p60fps"))
            }))
            .and_then(|format| format.get("fps"))
            .and_then(serde_json::Value::as_i64),
        Some(60)
    );
}

#[test]
fn eporner_native_extractor_reports_unavailable_xhr_video() {
    let extractor = EpornerExtractor::new(ExtractorDescriptor::new(
        "EpornerIE",
        "Eporner",
        r"https?://(?:www\.)?eporner\.com/(?:(?:hd-porn|embed)/|video-)(?P<id>\w+)(?:/(?P<display_id>[\w-]+))?",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "xhr/video/unavailable".to_owned(),
                br#"{"available":false,"message":"Native unavailable"}"#.to_vec(),
            ),
            (
                "eporner.com/hd-porn/unavailable".to_owned(),
                br#"<script>var hash = "0123456789abcdef0123456789abcdef";</script>"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context(
            "https://www.eporner.com/hd-porn/unavailable/native-display",
            &context,
        )
        .unwrap_err();

    assert_eq!(error.kind, ExtractorErrorKind::Extraction);
    assert!(error.message.contains("Native unavailable"));
}
