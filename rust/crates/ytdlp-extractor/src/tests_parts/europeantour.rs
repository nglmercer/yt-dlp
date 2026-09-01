#[test]
fn european_tour_native_extractor_redirects_to_brightcove() {
    let extractor = EuropeanTourExtractor::new(ExtractorDescriptor::new(
        "EuropeanTourIE",
        "EuropeanTour",
        r"https?://(?:www\.)?europeantour\.com/dpworld-tour/news/video/(?P<id>[^/&?#$]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<brightcove-player video-id="6287788195001">
            {"ACCOUNT_ID":"5136026580001"}
        </div>"#
        .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());

    assert_eq!(
        extractor
            .extract_with_context(
                "https://www.europeantour.com/dpworld-tour/news/video/the-best-shots-of-the-2021-seasons/",
                &context,
            )
            .unwrap(),
        ExtractorResult::Redirect {
            url: "http://players.brightcove.net/5136026580001/default_default/index.html?videoId=6287788195001".to_owned(),
            ie_key: Some("BrightcoveNew".to_owned()),
        }
    );
}

#[test]
fn european_tour_native_extractor_uses_default_brightcove_account() {
    let extractor = EuropeanTourExtractor::new(ExtractorDescriptor::new(
        "EuropeanTourIE",
        "EuropeanTour",
        r"https?://(?:www\.)?europeantour\.com/dpworld-tour/news/video/(?P<id>[^/&?#$]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<brightcove-player video-id="6287788195001">
            {"ACCOUNT_ID":""}
        </div>"#
        .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let ExtractorResult::Redirect { url, ie_key } = extractor
        .extract_with_context(
            "https://www.europeantour.com/dpworld-tour/news/video/example",
            &context,
        )
        .unwrap()
    else {
        panic!("European Tour must return a Brightcove redirect");
    };
    assert_eq!(
        url,
        "http://players.brightcove.net/5136026580001/default_default/index.html?videoId=6287788195001"
    );
    assert_eq!(ie_key.as_deref(), Some("BrightcoveNew"));
}
