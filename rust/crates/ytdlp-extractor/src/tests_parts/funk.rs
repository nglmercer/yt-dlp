#[test]
fn funk_native_extractor_redirects_to_nexx() {
    let extractor = FunkExtractor::new(ExtractorDescriptor::new(
        "FunkIE",
        "Funk",
        r#"https?://(?:(?:www|origin|play)\.)?funk\.net/(?:channel|playlist)/[^/?#]+/(?P<display_id>[0-9a-z-]+)-(?P<id>\d+)"#,
        true,
    ))
    .unwrap();
    let context = ExtractionContext::native();
    let ExtractorResult::Redirect { url, ie_key } = extractor
        .extract_with_context(
            "https://www.funk.net/channel/ba-793/native-video-1155821",
            &context,
        )
        .unwrap()
    else {
        panic!("expected Funk redirect");
    };

    assert_eq!(url, "nexx:741:1155821");
    assert_eq!(ie_key.as_deref(), Some("Nexx"));
}
