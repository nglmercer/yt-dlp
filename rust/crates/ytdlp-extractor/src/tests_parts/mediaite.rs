#[test]
fn mediaite_native_extractor_delegates_embedded_jwplatform_media() {
    let extractor = MediaiteExtractor::new(ExtractorDescriptor::new(
        "MediaiteIE",
        "Mediaite",
        r#"https?://(?:www\.)?mediaite\.com(?!/category)(?:/[\w-]+){2}"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "mediaite.com/sports/native-story".to_owned(),
                br#"<a href="https://cdn.jwplayer.com/players/nPripu9l/embed"></a>"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());

    assert_eq!(
        extractor
            .extract_with_context(
                "https://www.mediaite.com/sports/native-story",
                &context,
            )
            .unwrap(),
        ExtractorResult::Redirect {
            url: "jwplatform:nPripu9l".to_owned(),
            ie_key: Some("JWPlatform".to_owned()),
        }
    );
}

#[test]
fn mediaite_native_extractor_accepts_data_video_id_markup() {
    let extractor = MediaiteExtractor::new(ExtractorDescriptor::new(
        "MediaiteIE",
        "Mediaite",
        r#"https?://(?:www\.)?mediaite\.com(?!/category)(?:/[\w-]+){2}"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<div data-video-id="E6EhDX5z"></div>"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());

    assert_eq!(
        extractor
            .extract_with_context(
                "https://mediaite.com/politics/native-story",
                &context,
            )
            .unwrap(),
        ExtractorResult::Redirect {
            url: "jwplatform:E6EhDX5z".to_owned(),
            ie_key: Some("JWPlatform".to_owned()),
        }
    );
}
