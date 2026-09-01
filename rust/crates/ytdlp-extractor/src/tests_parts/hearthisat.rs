#[test]
fn hearthisat_native_extractor_maps_stream_and_download_formats() {
    let extractor = HearThisAtExtractor::new(ExtractorDescriptor::new(
        "HearThisAtIE",
        "HearThisAt",
        r#"https?://(?:www\.)?hearthis\.at/(?P<artist>[^/?#]+)/(?P<title>[\w.-]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "api-v2.hearthis.at/native/native-track".to_owned(),
            br#"{
                "id": 150939,
                "user": {"username": "Native Artist"},
                "title": "Native Track",
                "genre": "Experimental",
                "description": "Native description",
                "artwork_url": "https://cdn.example/native.jpg",
                "playback_count": "1,234",
                "duration": "70",
                "release_timestamp": 1421564134,
                "stream_url": "https://cdn.example/native.mp3",
                "download_url": "https://cdn.example/native.wav",
                "download_filename": "native.wav"
            }"#
            .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://hearthis.at/native/native-track",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("150939"));
    assert_eq!(
        result.get_str("display_id"),
        Some("native - native-track")
    );
    assert_eq!(result.get_str("title"), Some("Native Artist - Native Track"));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/native.mp3"));
    assert_eq!(result.get_str("ext"), Some("mp3"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(70)));
    assert_eq!(result.get("timestamp"), Some(&serde_json::json!(1421564134)));
    assert_eq!(result.get("view_count"), Some(&serde_json::json!(1234)));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.get(1))
            .and_then(|format| format.get("ext")),
        Some(&serde_json::json!("wav"))
    );
}

#[test]
fn hearthisat_native_extractor_rejects_missing_formats() {
    let extractor = HearThisAtExtractor::new(ExtractorDescriptor::new(
        "HearThisAtIE",
        "HearThisAt",
        r#"https?://(?:www\.)?hearthis\.at/(?P<artist>[^/?#]+)/(?P<title>[\w.-]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{"id":150939,"user":{"username":"Native Artist"},"title":"Native Track"}"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context("https://hearthis.at/native/native-track", &context)
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Extraction);
    assert!(error.message.contains("no playable audio formats"));
}
