#[test]
fn culture_unplugged_native_extractor_maps_movie_json_and_thumbnails() {
    let extractor = CultureUnpluggedExtractor::new(ExtractorDescriptor::new(
        "CultureUnpluggedIE",
        "CultureUnplugged",
        r"https?://(?:www\.)?cultureunplugged\.com/(?:documentary/watch-online/)?play/(?P<id>\d+)(?:/(?P<display_id>[^/#?]+))?",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "cultureunplugged.com/movie-data/cu-53662.json".to_owned(),
            br#"{
                "url":"https://cdn.example/culture/native-film.mp4",
                "title":"Native Culture Unplugged",
                "synopsis":"Native documentary synopsis",
                "producer":"Native Producer",
                "duration":2203,
                "views":9876,
                "small_thumb":"https://cdn.example/culture/small.jpg",
                "large_thumb":"https://cdn.example/culture/large.jpg"
            }"#
            .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.cultureunplugged.com/documentary/watch-online/play/53662/Native-Film",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("53662"));
    assert_eq!(result.get_str("display_id"), Some("Native-Film"));
    assert_eq!(result.get_str("title"), Some("Native Culture Unplugged"));
    assert_eq!(result.get_str("creator"), Some("Native Producer"));
    assert_eq!(result.get_i64("duration"), Some(2203));
    assert_eq!(result.get_i64("view_count"), Some(9876));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/culture/native-film.mp4")
    );
    let thumbnails = result
        .get("thumbnails")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(thumbnails.len(), 2);
    assert_eq!(thumbnails[0].get("id"), Some(&serde_json::json!("small")));
    assert_eq!(thumbnails[1].get("preference"), Some(&serde_json::json!(1)));
}
