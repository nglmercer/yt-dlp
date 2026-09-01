#[test]
fn gbnews_native_extractor_resolves_simplestream_hls() {
    let extractor = GbNewsExtractor::new(ExtractorDescriptor::new(
        "GBNewsIE",
        "GB News",
        r#"https?://(?:www\.)?gbnews\.(?:uk|com)/(?:\w+/)?(?P<id>[^#?]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "gbnews.com/news/native-story".to_owned(),
                br#"<meta property="og:title" content="Native GB News title">
                    <meta property="og:description" content="Native GB News description">
                    <meta property="og:image" content="https://cdn.example/gbnews.jpg">
                    <div class="simplestream" data-id="GB003" data-env="production"
                         data-uvid="uvid-1" data-type="vod" data-key="key-1"></div>"#
                    .to_vec(),
            ),
            (
                "mm-v2.simplestream.com/ssmp/api.php?id=GB003&env=production".to_owned(),
                br#"{"response":{"api_hostname":"https://api.example"}}"#.to_vec(),
            ),
            (
                "api.example/api/show/stream/uvid-1?key=key-1&platform=safari".to_owned(),
                br#"{"response":{"stream":"https://cdn.example/gbnews/master.m3u8"}}"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://www.gbnews.com/news/native-story", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("uvid-1"));
    assert_eq!(result.get_str("display_id"), Some("native-story"));
    assert_eq!(result.get_str("title"), Some("Native GB News title"));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/gbnews/master.m3u8")
    );
    assert_eq!(result.get_bool("is_live"), Some(false));
}

#[test]
fn gbnews_native_extractor_marks_drm_as_todo() {
    let extractor = GbNewsExtractor::new(ExtractorDescriptor::new(
        "GBNewsIE",
        "GB News",
        r#"https?://(?:www\.)?gbnews\.(?:uk|com)/(?:\w+/)?(?P<id>[^#?]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "gbnews.com/news/drm-story".to_owned(),
                br#"<div class="simplestream" data-uvid="drm-1"></div>"#.to_vec(),
            ),
            (
                "mm-v2.simplestream.com/ssmp/api.php".to_owned(),
                br#"{"response":{"api_hostname":"https://api.example"}}"#.to_vec(),
            ),
            (
                "api.example/api/show/stream/drm-1".to_owned(),
                br#"{"drm":true}"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context("https://www.gbnews.com/news/drm-story", &context)
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}
