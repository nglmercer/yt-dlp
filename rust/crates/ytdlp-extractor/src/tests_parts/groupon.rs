#[test]
fn groupon_native_extractor_maps_deal_video_playlist() {
    let extractor = GrouponExtractor::new(ExtractorDescriptor::new(
        "GrouponIE",
        "Groupon",
        r#"https?://(?:www\.)?groupon\.com/deals/(?P<id>[^/?#&]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "groupon.com/deals/native-deal".to_owned(),
            br#"<html><meta property="og:title" content="Native Groupon deal"><meta property="og:description" content="A native deal description"><script>window.payload = {"carousel":{"dealVideos":[{"provider":"youtube","media":"native-video"},{"provider":"vimeo","media":"ignored-video"},{"provider":"youtube","id":"https://www.youtube.com/watch?v=full-url"}]}};</script></html>"#.to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let ExtractorResult::Playlist { info, entries } = extractor
        .extract_with_context("https://www.groupon.com/deals/native-deal", &context)
        .unwrap()
    else {
        panic!("expected Groupon playlist");
    };

    assert_eq!(info.get_str("id"), Some("native-deal"));
    assert_eq!(info.get_str("title"), Some("Native Groupon deal"));
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get_str("ie_key"), Some("Youtube"));
    assert_eq!(
        entries[0].get_str("url"),
        Some("https://www.youtube.com/watch?v=native-video")
    );
    assert_eq!(
        entries[1].get_str("url"),
        Some("https://www.youtube.com/watch?v=full-url")
    );
}
