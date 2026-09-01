#[test]
fn jove_native_extractor_maps_chapter_xml_and_page_metadata() {
    let extractor = JoveExtractor::new(ExtractorDescriptor::new(
        "JoveIE",
        "Jove",
        r#"https?://(?:www\.)?jove\.com/video/(?P<id>[0-9]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "www.jove.com/video/2744".to_owned(),
                br#"<html><head>
                    <meta name="citation_title" content="Native Jove title">
                    <meta property="og:image" content="https://cdn.example/jove.png">
                    <meta name="citation_publication_date" content="2011-05-23">
                    <meta name="num_comments" content="7 Comments">
                </head><body>
                    <div id="section_body_summary"><p class="jove_content">Native Jove description</p></div>
                    <a href="/video-chapters?videoid=2744">chapters</a>
                </body></html>"#
                    .to_vec(),
            ),
            (
                "www.jove.com/video-chapters?videoid=2744".to_owned(),
                br#"<chapters video="https://cdn.example/jove.mp4"></chapters>"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.jove.com/video/2744/electrode-positioning",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("2744"));
    assert_eq!(result.get_str("title"), Some("Native Jove title"));
    assert_eq!(result.get_str("description"), Some("Native Jove description"));
    assert_eq!(result.get_str("upload_date"), Some("20110523"));
    assert_eq!(result.get("comment_count"), Some(&serde_json::json!(7)));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/jove.mp4"));
}
