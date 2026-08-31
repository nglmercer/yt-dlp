#[test]
fn myvidster_native_extractor_returns_resolved_videolink_redirect() {
    let extractor = MyVidsterExtractor::new(ExtractorDescriptor::new(
        "MyVidsterIE",
        "MyVidster",
        r"https?://(?:www\.)?myvidster\.com/video/(?P<id>\d+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "myvidster.com/video/32059805".to_owned(),
            br#"<html><body><a rel="videolink" href="/media/video.mp4?token=one&amp;part=two">watch</a></body></html>"#
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://www.myvidster.com/video/32059805", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("_type"), Some("url"));
    assert_eq!(
        result.get_str("url"),
        Some("https://www.myvidster.com/media/video.mp4?token=one&part=two")
    );
}
