#[test]
fn bongacams_native_extractor_maps_room_api_to_live_hls() {
    let extractor = BongaCamsExtractor::new(ExtractorDescriptor::new(
        "BongaCamsIE",
        "BongaCams",
        r"https?://(?P<host>(?:[^/]+\.)?bongacams\d*\.(?:com|net))/(?P<id>[^/?&#]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "de.bongacams.net/tools/amf.php".to_owned(),
            br#"{
                "localData": {"videoServerUrl": "https://stream.example/live"},
                "performerData": {
                    "username": "ClaireAshton",
                    "displayName": "Claire Ashton",
                    "loversCount": 1234
                }
            }"#
            .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://de.bongacams.net/claireashton", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("claireashton"));
    assert_eq!(result.get_str("title"), Some("Claire Ashton"));
    assert_eq!(result.get_str("uploader"), Some("Claire Ashton"));
    assert_eq!(result.get_str("uploader_id"), Some("ClaireAshton"));
    assert_eq!(result.get_i64("like_count"), Some(1234));
    assert_eq!(result.get_i64("age_limit"), Some(18));
    assert_eq!(result.get("is_live"), Some(&serde_json::json!(true)));
    assert_eq!(
        result.get_str("url"),
        Some("https://stream.example/live/hls/stream_ClaireAshton/playlist.m3u8")
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
