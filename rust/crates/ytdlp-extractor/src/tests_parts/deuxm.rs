#[test]
fn deuxm_native_extractor_maps_replay_api() {
    let extractor = DeuxMExtractor::new(ExtractorDescriptor::new(
        "DeuxMIE",
        "DeuxM",
        r"https?://(?:www\.)?2m\.ma/[^/]+/replay/single/(?P<id>([\w.]{1,24})+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "2m.ma/api/watchDetail/6351d439b15e1a613b3debe8".to_owned(),
            br#"{"response":{"News":{"titre":"Native replay","url":"https://cdn.example/replay.mp4","description":"Replay description","image":"https://cdn.example/replay.jpg"}}}"#.to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://2m.ma/fr/replay/single/6351d439b15e1a613b3debe8",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("6351d439b15e1a613b3debe8"));
    assert_eq!(result.get_str("title"), Some("Native replay"));
    assert_eq!(result.get_str("description"), Some("Replay description"));
    assert_eq!(result.get_str("thumbnail"), Some("https://cdn.example/replay.jpg"));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/replay.mp4"));
}

#[test]
fn deuxm_news_native_extractor_maps_article_api() {
    let extractor = DeuxMExtractor::new(ExtractorDescriptor::new(
        "DeuxMNewsIE",
        "DeuxMNews",
        r"https?://(?:www\.)?2m\.ma/(?P<lang>\w+)/news/(?P<id>[^/#?]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "2m.ma/api/articlesByUrl".to_owned(),
            br#"{"response":{"article":[{"id":"native-article","title":"Native article","image":["https://cdn.example/article.mp4"],"content":"Article description","cover":"https://cdn.example/article.jpg"}]}}"#.to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://2m.ma/fr/news/native-article",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("native-article"));
    assert_eq!(result.get_str("title"), Some("Native article"));
    assert_eq!(result.get_str("description"), Some("Article description"));
    assert_eq!(result.get_str("thumbnail"), Some("https://cdn.example/article.jpg"));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/article.mp4"));
}
