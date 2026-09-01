#[test]
fn foxsports_native_extractor_resolves_preplay_and_overrides_metadata() {
    let extractor = FoxSportsExtractor::new(ExtractorDescriptor::new(
        "FoxSportsIE",
        "FoxSports",
        r#"https?://(?:www\.)?foxsports\.com/watch/(?P<id>[\w-]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "foxsports.com/watch/play-native".to_owned(),
                br#"<script type="application/ld+json">{
                    "name":"JSON-LD title",
                    "description":"JSON-LD description",
                    "uploadDate":"2022-12-05T10:09:46Z",
                    "thumbnailUrl":"https://cdn.example/fox.jpg"
                }</script>"#
                .to_vec(),
            ),
            (
                "api3.fox.com/v2.0/vodplayer/sportsclip/play-native".to_owned(),
                br#"{
                    "url":"https://abc.uplynk.com/preplay/0123456789abcdef0123456789abcdef.json",
                    "name":"API title",
                    "description":"API description",
                    "durationInSeconds":"31.7317"
                }"#
                .to_vec(),
            ),
            (
                "abc.uplynk.com/preplay/0123456789abcdef0123456789abcdef.json".to_owned(),
                br#"{"sid":"fox-session"}"#.to_vec(),
            ),
            (
                "content.uplynk.com/player/assetinfo/0123456789abcdef0123456789abcdef.json"
                    .to_owned(),
                br#"{
                    "asset":"b72f5bd8658140baa5791bb676433733",
                    "desc":"Uplynk title",
                    "default_poster_url":"https://cdn.example/uplynk.jpg",
                    "duration":31.7,
                    "owner":"06b4a36349624051a9ba52ac3a91d268"
                }"#
                .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.foxsports.com/watch/play-native",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(
        result.get_str("id"),
        Some("b72f5bd8658140baa5791bb676433733")
    );
    assert_eq!(result.get_str("display_id"), Some("play-native"));
    assert_eq!(result.get_str("title"), Some("API title"));
    assert_eq!(result.get_str("description"), Some("API description"));
    assert_eq!(result.get_f64("duration"), Some(31.7317));
    assert_eq!(result.get_i64("timestamp"), Some(1670234986));
}
