#[test]
fn muenchentv_native_extractor_maps_live_playlist_sources() {
    let extractor = MuenchenTvExtractor::new(ExtractorDescriptor::new(
        "MuenchenTVIE",
        "MuenchenTV",
        r"https?://(?:www\.)?muenchen\.tv/livestream",
        false,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "www.muenchen.tv/livestream".to_owned(),
            r#"<html><head><meta property="og:title" content="münchen.tv-Livestream 2026-08-31 12:00"></head>
                <script>
                    playlist: [{
                        mediaid: '5334',
                        image: '//cdn.example/muenchen/live.jpg',
                        sources: [
                            {file: 'https://cdn.example/muenchen/live.m3u8', label: '720'},
                            {file: 'https://cdn.example/muenchen/live.smil', label: '360'},
                        ],
                    }],
                </script></html>"#
            .as_bytes()
            .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("http://www.muenchen.tv/livestream/", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("5334"));
    assert_eq!(result.get_str("display_id"), Some("live"));
    assert_eq!(
        result.get_str("title"),
        Some("münchen.tv-Livestream 2026-08-31 12:00")
    );
    assert_eq!(result.get_str("thumbnail"), Some("https://cdn.example/muenchen/live.jpg"));
    assert_eq!(result.get("is_live"), Some(&serde_json::json!(true)));
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(formats[0].get("protocol"), Some(&serde_json::json!("m3u8_native")));
    assert_eq!(formats[1].get("protocol"), Some(&serde_json::json!("smil")));
    assert_eq!(formats[1].get("preference"), Some(&serde_json::json!(-100)));
}
