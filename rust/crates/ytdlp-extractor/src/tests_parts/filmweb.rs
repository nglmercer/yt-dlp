#[test]
fn filmweb_native_extractor_resolves_trailer_iframe() {
    let extractor = FilmwebExtractor::new(ExtractorDescriptor::new(
        "FilmwebIE",
        "Filmweb",
        r"https?://(?:www\.)?filmweb\.no/(?P<type>trailere|filmnytt)/article(?P<id>\d+)\.ece",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "json_trailerEmbed.jsp?articleId=1264921".to_owned(),
            br#"{"embedCode":"<iframe src=\"//video.example/filmweb/13033574\"></iframe>"}"#
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "http://www.filmweb.no/trailere/article1264921.ece",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("_type"), Some("url_transparent"));
    assert_eq!(result.get_str("id"), Some("1264921"));
    assert_eq!(
        result.get_str("url"),
        Some("https://video.example/filmweb/13033574")
    );
    assert_eq!(result.get_str("ie_key"), Some("TwentyThreeVideo"));
}

#[test]
fn filmweb_native_extractor_resolves_filmnytt_video_id_from_page() {
    let extractor = FilmwebExtractor::new(ExtractorDescriptor::new(
        "FilmwebIE",
        "Filmweb",
        r"https?://(?:www\.)?filmweb\.no/(?P<type>trailere|filmnytt)/article(?P<id>\d+)\.ece",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "filmnytt/article1264921.ece".to_owned(),
                br#"<div data-videoid="13033574"></div>"#.to_vec(),
            ),
            (
                "json_trailerEmbed.jsp?articleId=13033574".to_owned(),
                br#"{"embedCode":"<iframe src=\"https://video.example/filmweb/13033574\"></iframe>"}"#
                    .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.filmweb.no/filmnytt/article1264921.ece",
            &context,
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_str("id"), Some("13033574"));
    assert_eq!(
        result.get_str("url"),
        Some("https://video.example/filmweb/13033574")
    );
}
