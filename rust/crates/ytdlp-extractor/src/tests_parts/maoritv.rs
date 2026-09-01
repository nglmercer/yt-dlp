#[test]
fn maori_tv_native_extractor_redirects_to_brightcove() {
    let extractor = MaoriTvExtractor::new(ExtractorDescriptor::new(
        "MaoriTVIE",
        "MaoriTV",
        r#"https?://(?:www\.)?maoritelevision\.com/shows/(?:[^/]+/)+(?P<id>[^/?&#]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<div class="player" data-main-video-id="4774724855001"></div>"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());

    assert_eq!(
        extractor
            .extract_with_context(
                "https://www.maoritelevision.com/shows/korero-mai/S01E054/korero-mai-series-1-episode-54",
                &context,
            )
            .unwrap(),
        ExtractorResult::Redirect {
            url: "http://players.brightcove.net/1614493167001/HJlhIQhQf_default/index.html?videoId=4774724855001".to_owned(),
            ie_key: Some("BrightcoveNew".to_owned()),
        }
    );
}

#[test]
fn maori_tv_native_extractor_requires_brightcove_id() {
    let extractor = MaoriTvExtractor::new(ExtractorDescriptor::new(
        "MaoriTVIE",
        "MaoriTV",
        r#"https?://(?:www\.)?maoritelevision\.com/shows/(?:[^/]+/)+(?P<id>[^/?&#]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: b"<html>no player</html>".to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context(
            "https://maoritelevision.com/shows/korero-mai/missing",
            &context,
        )
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Extraction);
    assert!(error.message.contains("no Brightcove video ID"));
}
