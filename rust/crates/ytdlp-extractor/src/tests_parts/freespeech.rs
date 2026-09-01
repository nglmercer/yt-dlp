#[test]
fn freespeech_native_extractor_redirects_to_youtube() {
    let extractor = FreespeechExtractor::new(ExtractorDescriptor::new(
        "FreespeechIE",
        "freespeech.org",
        r"https?://(?:www\.)?freespeech\.org/stories/(?P<id>.+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<div data-video-url="https://www.youtube.com/watch?v=waRk6IPqyWM"></div>"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    assert_eq!(
        extractor
            .extract_with_context(
                "https://www.freespeech.org/stories/native-story/",
                &context,
            )
            .unwrap(),
        ExtractorResult::Redirect {
            url: "https://www.youtube.com/watch?v=waRk6IPqyWM".to_owned(),
            ie_key: Some("Youtube".to_owned()),
        }
    );
}

#[test]
fn freespeech_native_extractor_requires_youtube_url() {
    let extractor = FreespeechExtractor::new(ExtractorDescriptor::new(
        "FreespeechIE",
        "freespeech.org",
        r"https?://(?:www\.)?freespeech\.org/stories/(?P<id>.+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<div class="story"></div>"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context(
            "https://www.freespeech.org/stories/native-story/",
            &context,
        )
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Extraction);
    assert!(error.message.contains("no YouTube URL"));
}
