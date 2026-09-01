#[test]
fn foxnews_video_native_extractor_redirects_to_feed_extractor() {
    let extractor = FoxNewsVideoExtractor::new(ExtractorDescriptor::new(
        "FoxNewsVideoIE",
        "FoxNewsVideo",
        r"https?://(?:www\.)?foxnews\.com/video/(?P<id>\d+)",
        true,
    ))
    .unwrap();
    assert_eq!(
        extractor
            .extract_with_context(
                "https://www.foxnews.com/video/6328632286112",
                &ExtractionContext::native(),
            )
            .unwrap(),
        ExtractorResult::Redirect {
            url: "https://video.foxnews.com/v/6328632286112".to_owned(),
            ie_key: Some("FoxNews".to_owned()),
        }
    );
}

#[test]
fn foxnews_article_native_extractor_handles_data_video_id_and_iframe() {
    let extractor = FoxNewsArticleExtractor::new(ExtractorDescriptor::new(
        "FoxNewsArticleIE",
        "foxnews:article",
        r"https?://(?:www\.)?(?:insider\.)?foxnews\.com/(?!v)([^/]+/)+(?P<id>[a-z-]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "data-id-article".to_owned(),
                br#"<div data-video-id="5116295019001"></div>"#.to_vec(),
            ),
            (
                "iframe-article".to_owned(),
                br#"<iframe src="//video.foxnews.com/v/video-embed.html?video_id=5748266721001&autoplay=true"></iframe>"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    assert_eq!(
        extractor
            .extract_with_context(
                "https://www.foxnews.com/politics/data-id-article",
                &context,
            )
            .unwrap(),
        ExtractorResult::Redirect {
            url: "http://video.foxnews.com/v/5116295019001".to_owned(),
            ie_key: Some("FoxNews".to_owned()),
        }
    );
    assert_eq!(
        extractor
            .extract_with_context(
                "https://www.foxnews.com/us/iframe-article",
                &context,
            )
            .unwrap(),
        ExtractorResult::Redirect {
            url: "https://video.foxnews.com/v/video-embed.html?video_id=5748266721001"
                .to_owned(),
            ie_key: Some("FoxNews".to_owned()),
        }
    );
}
