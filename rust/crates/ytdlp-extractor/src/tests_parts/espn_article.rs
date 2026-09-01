#[test]
fn espn_article_native_extractor_redirects_embedded_clip() {
    let extractor = EspnArticleExtractor::new(ExtractorDescriptor::new(
        "ESPNArticleIE",
        "ESPNArticle",
        r"https?://(?:espn\.go|(?:www\.)?espn)\.com/(?:[^/]+/)*(?P<id>[^/]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<div class='hero video-play-button' data-id='10365079'></div>"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    assert!(extractor.suitable("https://www.espn.com/nba/recap?gameId=400793786"));
    assert!(!extractor.suitable("https://www.espn.com/video/clip?id=10365079"));
    assert!(!extractor.suitable(
        "https://www.espn.com/watch/player/_/id/01234567-89ab-cdef-0123-456789abcdef"
    ));
    assert_eq!(
        extractor
            .extract_with_context(
                "https://www.espn.com/nba/recap?gameId=400793786",
                &context,
            )
            .unwrap(),
        ExtractorResult::Redirect {
            url: "http://espn.go.com/video/clip?id=10365079".to_owned(),
            ie_key: Some("ESPN".to_owned()),
        }
    );
}

#[test]
fn espn_article_native_extractor_requires_embedded_clip_id() {
    let extractor = EspnArticleExtractor::new(ExtractorDescriptor::new(
        "ESPNArticleIE",
        "ESPNArticle",
        r"https?://(?:espn\.go|(?:www\.)?espn)\.com/(?:[^/]+/)*(?P<id>[^/]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<article class='video-play-button'></article>"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context("https://www.espn.com/story/article", &context)
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Extraction);
    assert!(error.message.contains("embedded video ID"));
}
