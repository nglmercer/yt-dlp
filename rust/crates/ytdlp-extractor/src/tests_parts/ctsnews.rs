#[test]
fn ctsnews_native_extractor_maps_direct_feed_and_page_metadata() {
    let extractor = CtsNewsExtractor::new(ExtractorDescriptor::new(
        "CtsNewsIE",
        "CtsNews",
        r"https?://news\.cts\.com\.tw/[a-z]+/[a-z]+/\d+/(?P<id>\d+)\.html",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "news.cts.com.tw/cts/international/201501/201501291578109.html".to_owned(),
                r#"<html><head>
                    <meta name="title" content="Native CTS News title">
                    <meta name="description" content="Native CTS News description">
                    <meta name="image" content="https://cdn.example/cts.jpg">
                    <input type="hidden" name="get_id" value="native-feed-id">
                    <div>2015/01/29 08:30</div>
                </head></html>"#
                .as_bytes()
                .to_vec(),
            ),
            (
                "news.cts.com.tw/action/test_mp4feed.php?news_id=native-feed-id".to_owned(),
                br#"{"source_url":"https://cdn.example/cts/native.mp4"}"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://news.cts.com.tw/cts/international/201501/201501291578109.html",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("native-feed-id"));
    assert_eq!(result.get_str("title"), Some("Native CTS News title"));
    assert_eq!(result.get_str("description"), Some("Native CTS News description"));
    assert_eq!(result.get_str("thumbnail"), Some("https://cdn.example/cts.jpg"));
    assert_eq!(result.get_i64("timestamp"), Some(1_422_491_400));
    assert_eq!(result.get_str("upload_date"), Some("20150129"));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/cts/native.mp4"));
}

#[test]
fn ctsnews_native_extractor_marks_youtube_fallback_as_todo() {
    let extractor = CtsNewsExtractor::new(ExtractorDescriptor::new(
        "CtsNewsIE",
        "CtsNews",
        r"https?://news\.cts\.com\.tw/[a-z]+/[a-z]+/\d+/(?P<id>\d+)\.html",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "news.cts.com.tw/cts/money/201501/201501291578003.html".to_owned(),
            br#"<iframe src="https://www.youtube.com/embed/native-youtube"></iframe>"#.to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context(
            "https://news.cts.com.tw/cts/money/201501/201501291578003.html",
            &context,
        )
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}
