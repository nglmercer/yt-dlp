#[test]
fn btvplus_native_extractor_maps_player_config_and_hls() {
    let extractor = BtvPlusExtractor::new(ExtractorDescriptor::new(
        "BTVPlusIE",
        "BTVPlus",
        r"https?://(?:www\.)?btvplus\.bg/produkt/(?:predavaniya|seriali|novini)/(?P<id>\d+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "btvplus.bg/player/config/67271".to_owned(),
                br#"{"config":"videojs('player', {sources: [{src: 'https://cdn.example/btv/67271.m3u8', type: 'application/x-mpegURL'}, {src: 'https://cdn.example/btv/67271.mp4', type: 'video/mp4'}]})"}"#
                    .to_vec(),
            ),
            (
                "btvplus.bg/produkt/predavaniya/67271".to_owned(),
                br#"<meta property="og:title" content="Native bTV title">
                    <meta property="og:image" content="https://cdn.example/btv/poster.jpg">
                    <meta property="og:description" content="Native bTV description">
                    <script>var videoUrl = '/player/config/67271';</script>"#
                    .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.btvplus.bg/produkt/predavaniya/67271/title",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("67271"));
    assert_eq!(result.get_str("title"), Some("Native bTV title"));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/btv/poster.jpg")
    );
    assert_eq!(result.get_str("description"), Some("Native bTV description"));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/btv/67271.m3u8")
    );
    let formats = result.get("formats").and_then(serde_json::Value::as_array).unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(formats[0].get("format_id"), Some(&serde_json::json!("hls")));
    assert_eq!(formats[1].get("ext"), Some(&serde_json::json!("mp4")));
}

#[test]
fn btvplus_native_extractor_marks_unknown_source_types_as_todo() {
    let extractor = BtvPlusExtractor::new(ExtractorDescriptor::new(
        "BTVPlusIE",
        "BTVPlus",
        r"https?://(?:www\.)?btvplus\.bg/produkt/(?:predavaniya|seriali|novini)/(?P<id>\d+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "btvplus.bg/player/config/99".to_owned(),
                br#"{"config":"videojs('player', {sources: [{src: 'https://cdn.example/btv/99.bin', type: 'video/x-unsupported'}]})"}"#
                    .to_vec(),
            ),
            (
                "btvplus.bg/produkt/seriali/99".to_owned(),
                br#"<script>var videoUrl = '/player/config/99';</script>"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context("https://btvplus.bg/produkt/seriali/99", &context)
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}
