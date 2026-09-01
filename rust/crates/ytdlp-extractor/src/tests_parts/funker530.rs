#[test]
fn funker530_native_extractor_redirects_to_rumble_and_preserves_description() {
    let extractor = Funker530Extractor::new(ExtractorDescriptor::new(
        "Funker530IE",
        "Funker530",
        r"https?://(?:www\.)?funker530\.com/video/(?P<id>[^/?#]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<div class="video-desc-paragraph">
            Native description.<br>About the Author ignored.
        </div>
        <iframe src="//rumble.com/embed/v2qbmu4/?pub=abc"></iframe>"#
        .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://funker530.com/video/native-video/",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("_type"), Some("url_transparent"));
    assert_eq!(
        result.get_str("url"),
        Some("https://rumble.com/embed/v2qbmu4/?pub=abc")
    );
    assert_eq!(result.get_str("ie_key"), Some("RumbleEmbed"));
    assert_eq!(
        result.get_str("description"),
        Some("Native description.")
    );
}

#[test]
fn funker530_native_extractor_redirects_to_youtube_when_rumble_is_absent() {
    let extractor = Funker530Extractor::new(ExtractorDescriptor::new(
        "Funker530IE",
        "Funker530",
        r"https?://(?:www\.)?funker530\.com/video/(?P<id>[^/?#]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<iframe src="https://www.youtube.com/embed/Native12345"></iframe>"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.funker530.com/video/native-video",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("url"), Some("https://www.youtube.com/embed/Native12345"));
    assert_eq!(result.get_str("ie_key"), Some("Youtube"));
}
