#[test]
fn hollywood_reporter_native_extractor_redirects_jwplatform_video() {
    let extractor = HollywoodReporterExtractor::new(ExtractorDescriptor::new(
        "HollywoodReporterIE",
        "HollywoodReporter",
        r#"https?://(?:www\.)?hollywoodreporter\.com/video/(?P<id>[\w-]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<a class="vlanding-video-card__link" data-video-showcase-trigger="zH4jZaR5" data-video-showcase-type="jwplayer"></a>"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    assert_eq!(
        extractor
            .extract_with_context(
                "https://www.hollywoodreporter.com/video/native-video/",
                &context,
            )
            .unwrap(),
        ExtractorResult::Redirect {
            url: "jwplatform:zH4jZaR5".to_owned(),
            ie_key: Some("JWPlatform".to_owned()),
        }
    );
}

#[test]
fn hollywood_reporter_native_extractor_redirects_youtube_video() {
    let extractor = HollywoodReporterExtractor::new(ExtractorDescriptor::new(
        "HollywoodReporterIE",
        "HollywoodReporter",
        r#"https?://(?:www\.)?hollywoodreporter\.com/video/(?P<id>[\w-]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<a class='vlanding-video-card__link' data-video-showcase-trigger='native-youtube' data-video-showcase-type='youtube'></a>"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    assert_eq!(
        extractor
            .extract_with_context(
                "https://www.hollywoodreporter.com/video/native-video/",
                &context,
            )
            .unwrap(),
        ExtractorResult::Redirect {
            url: "https://www.youtube.com/watch?v=native-youtube".to_owned(),
            ie_key: Some("Youtube".to_owned()),
        }
    );
}

#[test]
fn hollywood_reporter_native_playlist_expands_pages() {
    let extractor = HollywoodReporterPlaylistExtractor::new(ExtractorDescriptor::new(
        "HollywoodReporterPlaylistIE",
        "HollywoodReporterPlaylist",
        r#"https?://(?:www\.)?hollywoodreporter\.com/vcategory/(?P<slug>[\w-]+)-(?P<id>\d+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "/vcategory/native-category-57822/page/2/".to_owned(),
                br#"<section class="video-playlist-river"></section>"#.to_vec(),
            ),
            (
                "/vcategory/native-category-57822/page/1/".to_owned(),
                br#"<section class="video-playlist-river">
                    <a href="/video/native-one/" class="c-title__link">One</a>
                    <a class='c-title__link' href='https://www.hollywoodreporter.com/video/native-two/'>Two</a>
                    <a href="/video/ignored/" class="other-link">Ignored</a>
                </section>"#
                .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let ExtractorResult::Playlist { info, entries } = extractor
        .extract_with_context(
            "https://www.hollywoodreporter.com/vcategory/native-category-57822/",
            &context,
        )
        .unwrap()
    else {
        panic!("expected Hollywood Reporter playlist");
    };

    assert_eq!(info.get_str("id"), Some("57822"));
    assert_eq!(info.get_str("title"), Some("native-category"));
    assert_eq!(entries.len(), 2);
    assert_eq!(
        entries[0].get_str("url"),
        Some("https://www.hollywoodreporter.com/video/native-one/")
    );
    assert_eq!(entries[1].get_str("ie_key"), Some("HollywoodReporter"));
}

#[test]
fn hollywood_reporter_native_extractor_marks_unknown_showcase_as_todo() {
    let extractor = HollywoodReporterExtractor::new(ExtractorDescriptor::new(
        "HollywoodReporterIE",
        "HollywoodReporter",
        r#"https?://(?:www\.)?hollywoodreporter\.com/video/(?P<id>[\w-]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<a class="vlanding-video-card__link" data-video-showcase-trigger="native-media" data-video-showcase-type="unknown"></a>"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context(
            "https://www.hollywoodreporter.com/video/native-video/",
            &context,
        )
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}
