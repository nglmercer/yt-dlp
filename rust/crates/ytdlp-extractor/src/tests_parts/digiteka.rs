#[test]
fn digiteka_native_extractor_maps_hls_and_mp4_player_sources() {
    let extractor = DigitekaExtractor::new(ExtractorDescriptor::new(
        "DigitekaIE",
        "Digiteka",
        r"(?x)
        https?://(?:www\.)?(?:digiteka\.net|ultimedia\.com)/
        (?:
            deliver/(?:generic|musique)(?:/[^/]+)*/(?:src|article)|
            default/index/video(?:generic|music)/id
        )/(?P<id>[\d+a-z]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "ultimedia.com/player/getConf/01836272/1/native55".to_owned(),
            br#"{
                "video":{
                    "title":"Native Digiteka",
                    "image":"https://cdn.example/digiteka.jpg",
                    "duration":89,
                    "creationDate":1760285363,
                    "ownerId":"native-owner",
                    "media_sources":{
                        "hls":{"hls_auto":"https://cdn.example/digiteka/native.m3u8"},
                        "mp4":{
                            "mp4_360":"https://cdn.example/digiteka/native-360.mp4",
                            "mp4_720":"https://cdn.example/digiteka/native-720.mp4"
                        }
                    }
                }
            }"#
            .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.ultimedia.com/default/index/videogeneric/id/native55",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("native55"));
    assert_eq!(result.get_str("title"), Some("Native Digiteka"));
    assert_eq!(result.get_str("uploader_id"), Some("native-owner"));
    assert_eq!(result.get_i64("duration"), Some(89));
    assert_eq!(result.get_i64("timestamp"), Some(1_760_285_363));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/digiteka/native.m3u8")
    );
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 3);
    assert_eq!(formats[0].get("protocol"), Some(&serde_json::json!("m3u8_native")));
    assert_eq!(formats[2].get("height"), Some(&serde_json::json!(720)));
}
