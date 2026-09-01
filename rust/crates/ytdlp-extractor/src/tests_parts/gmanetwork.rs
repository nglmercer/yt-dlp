#[test]
fn gmanetwork_native_extractor_redirects_page_youtube_target() {
    let extractor = GmaNetworkVideoExtractor::new(ExtractorDescriptor::new(
        "GMANetworkVideoIE",
        "GMANetworkVideo",
        r#"https?://(?:www)\.gmanetwork\.com/(?:\w+/){3}(?P<id>\d+)/(?P<display_id>[\w-]+)/video"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "gmanetwork.com/fullepisodes/home/show/168677/native-video/video".to_owned(),
            br#"<script>var YOUTUBE_VIDEO = 'native-yt';</script>"#.to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let ExtractorResult::Redirect { url, ie_key } = extractor
        .extract_with_context(
            "https://www.gmanetwork.com/fullepisodes/home/show/168677/native-video/video",
            &context,
        )
        .unwrap()
    else {
        panic!("expected GMA Network redirect");
    };
    assert_eq!(ie_key.as_deref(), Some("Youtube"));
    assert_eq!(url, "https://www.youtube.com/watch?v=native-yt");
}

#[test]
fn gmanetwork_native_extractor_uses_api_dailymotion_fallback() {
    let extractor = GmaNetworkVideoExtractor::new(ExtractorDescriptor::new(
        "GMANetworkVideoIE",
        "GMANetworkVideo",
        r#"https?://(?:www)\.gmanetwork\.com/(?:\w+/){3}(?P<id>\d+)/(?P<display_id>[\w-]+)/video"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "gmanetwork.com/fullepisodes/home/show/168677/api-video/video".to_owned(),
                br#"<script>var NETWORK_URL = 'https://api.gma.example/';</script>"#.to_vec(),
            ),
            (
                "api.gma.example/api/data/content/video/168677".to_owned(),
                br#"{"dailymotion_file":"https://www.dailymotion.com/video/native-dm"}"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let ExtractorResult::Redirect { url, ie_key } = extractor
        .extract_with_context(
            "https://www.gmanetwork.com/fullepisodes/home/show/168677/api-video/video",
            &context,
        )
        .unwrap()
    else {
        panic!("expected GMA Network API redirect");
    };
    assert_eq!(ie_key.as_deref(), Some("Dailymotion"));
    assert_eq!(url, "https://www.dailymotion.com/video/native-dm");
}
