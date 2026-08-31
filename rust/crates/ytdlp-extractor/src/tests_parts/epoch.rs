#[test]
fn epoch_native_extractor_maps_youmaker_hls_from_videobox() {
    let extractor = EpochExtractor::new(ExtractorDescriptor::new(
        "EpochIE",
        "Epoch",
        r"https?://www\.theepochtimes\.com/[\w-]+_(?P<id>\d+).html",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "theepochtimes.com/native-article_4661688.html".to_owned(),
            r#"<html><head><title>Native Epoch title</title></head>
                <body><div data-id="youmaker-native" id="videobox"></div></body></html>"#
                .as_bytes()
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.theepochtimes.com/native-article_4661688.html",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("youmaker-native"));
    assert_eq!(result.get_str("title"), Some("Native Epoch title"));
    assert_eq!(
        result.get_str("url"),
        Some("http://vs1.youmaker.com/assets/youmaker-native/playlist.m3u8")
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("protocol")),
        Some(&serde_json::json!("m3u8_native"))
    );
}
