#[test]
fn germanupa_native_extractor_redirects_vimeo_oembed() {
    let extractor = GermanupaExtractor::new(ExtractorDescriptor::new(
        "GermanupaIE",
        "germanupa.de",
        r#"https?://germanupa\.de/mediathek/(?P<id>[\w-]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "germanupa.de/mediathek/native-video".to_owned(),
            br#"<iframe data-src="https://germanupa.de/media/oembed?url=https%3A%2F%2Fvimeo.com%2F909179246"></iframe>"#.to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let ExtractorResult::Redirect { url, ie_key } = extractor
        .extract_with_context(
            "https://germanupa.de/mediathek/native-video",
            &context,
        )
        .unwrap()
    else {
        panic!("expected German UPA Vimeo redirect");
    };
    assert_eq!(ie_key.as_deref(), Some("Vimeo"));
    assert_eq!(url, "https://player.vimeo.com/video/909179246");
}

#[test]
fn germanupa_native_extractor_marks_member_only_page_todo() {
    let extractor = GermanupaExtractor::new(ExtractorDescriptor::new(
        "GermanupaIE",
        "germanupa.de",
        r#"https?://germanupa\.de/mediathek/(?P<id>[\w-]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "germanupa.de/mediathek/members-only".to_owned(),
            br#"<div class="login-wrapper">members</div>"#.to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context(
            "https://germanupa.de/mediathek/members-only",
            &context,
        )
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}
