#[test]
fn charlierose_native_extractor_maps_html5_sources_and_subtitles() {
    let extractor = CharlieRoseExtractor::new(ExtractorDescriptor::new(
        "CharlieRoseIE",
        "CharlieRose",
        r"https?://(?:www\.)?charlierose\.com/(?:video|episode)(?:s|/player)/(?P<id>\d+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "charlierose.com/video/player/27996".to_owned(),
            br#"<html><head>
                <meta property="og:title" content="Remembering Zaha Hadid - Charlie Rose">
                <meta property="og:image" content="https://cdn.example/rose/poster.jpg">
                <meta property="og:description" content="Native Charlie Rose description">
            </head><body>
                <video>
                    <source src="/media/27996/master.m3u8">
                    <source src="https://cdn.example/rose/27996.mp4">
                    <track kind="subtitles" srclang="en" src="/captions/27996-en.vtt">
                </video>
            </body></html>"#
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://charlierose.com/videos/27996", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("27996"));
    assert_eq!(result.get_str("title"), Some("Remembering Zaha Hadid"));
    assert_eq!(
        result.get_str("description"),
        Some("Native Charlie Rose description")
    );
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/rose/poster.jpg")
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://charlierose.com/media/27996/master.m3u8")
    );
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(
        formats[0].get("protocol"),
        Some(&serde_json::json!("m3u8_native"))
    );
    assert_eq!(
        result
            .get("subtitles")
            .and_then(|subtitles| subtitles.get("en"))
            .and_then(serde_json::Value::as_array)
            .and_then(|entries| entries.first())
            .and_then(|entry| entry.get("url")),
        Some(&serde_json::json!("https://charlierose.com/captions/27996-en.vtt"))
    );
}
