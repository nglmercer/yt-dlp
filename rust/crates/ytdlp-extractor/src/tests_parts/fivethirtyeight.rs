#[test]
fn fivethirtyeight_native_extractor_redirects_to_abc_news_video() {
    let extractor = FiveThirtyEightExtractor::new(ExtractorDescriptor::new(
        "FiveThirtyEightIE",
        "FiveThirtyEight",
        r"https?://(?:www\.)?fivethirtyeight\.com/features/(?P<id>[^/?#]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<article>
            <iframe class="video" src="https://fivethirtyeight.abcnews.go.com/video/embed/123/456"></iframe>
        </article>"#
        .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    assert_eq!(
        extractor
            .extract_with_context(
                "https://fivethirtyeight.com/features/native-feature",
                &context,
            )
            .unwrap(),
        ExtractorResult::Redirect {
            url: "https://fivethirtyeight.abcnews.go.com/video/embed/123/456".to_owned(),
            ie_key: Some("AbcNewsVideo".to_owned()),
        }
    );
}

#[test]
fn fivethirtyeight_native_extractor_requires_abc_news_embed() {
    let extractor = FiveThirtyEightExtractor::new(ExtractorDescriptor::new(
        "FiveThirtyEightIE",
        "FiveThirtyEight",
        r"https?://(?:www\.)?fivethirtyeight\.com/features/(?P<id>[^/?#]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<article><p>No video</p></article>"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context(
            "https://fivethirtyeight.com/features/native-feature",
            &context,
        )
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Extraction);
    assert!(error.message.contains("no ABC News video embed"));
}
