#[test]
fn fox9_native_video_extractor_redirects_to_anvato() {
    let extractor = Fox9Extractor::new(ExtractorDescriptor::new(
        "FOX9IE",
        "FOX9",
        r"https?://(?:www\.)?fox9\.com/video/(?P<id>\d+)",
        true,
    ))
    .unwrap();
    assert_eq!(
        extractor
            .extract_with_context(
                "https://www.fox9.com/video/314473",
                &ExtractionContext::native(),
            )
            .unwrap(),
        ExtractorResult::Redirect {
            url: "anvato:anvato_epfox_app_web_prod_b3373168e12f423f41504f207000188daf88251b:314473"
                .to_owned(),
            ie_key: Some("Anvato".to_owned()),
        }
    );
}

#[test]
fn fox9_native_news_extractor_finds_anvato_id() {
    let extractor = Fox9NewsExtractor::new(ExtractorDescriptor::new(
        "FOX9NewsIE",
        "FOX9News",
        r"https?://(?:www\.)?fox9\.com/news/(?P<id>[^/?&#]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<script>window.player = {anvatoId: '314473'};</script>"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    assert_eq!(
        extractor
            .extract_with_context(
                "https://www.fox9.com/news/black-bear-in-tree",
                &context,
            )
            .unwrap(),
        ExtractorResult::Redirect {
            url: "https://www.fox9.com/video/314473".to_owned(),
            ie_key: Some("FOX9".to_owned()),
        }
    );
}
