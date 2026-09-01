#[test]
fn hgtv_native_extractor_builds_configured_playlist() {
    let extractor = HgtvComShowExtractor::new(ExtractorDescriptor::new(
        "HGTVComShowIE",
        "hgtv.com:show",
        r#"https?://(?:www\.)?hgtv\.com/shows/[^/]+/(?P<id>[^/?#&]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "hgtv.com/shows/native/native-season".to_owned(),
            br#"<div data-module="video"><script type="text/x-config">{"channels":[{"title":"Native HGTV show","description":"Native show description","videos":[{"releaseUrl":"https://video.example/episode-1"},{"releaseUrl":"https://video.example/episode-2"}]}]}</script></div>"#.to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let ExtractorResult::Playlist { info, entries } = extractor
        .extract_with_context(
            "https://www.hgtv.com/shows/native/native-season",
            &context,
        )
        .unwrap()
    else {
        panic!("expected HGTV playlist");
    };

    assert_eq!(info.get_str("id"), Some("native-season"));
    assert_eq!(info.get_str("title"), Some("Native HGTV show"));
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get_str("url"), Some("https://video.example/episode-1"));
}
