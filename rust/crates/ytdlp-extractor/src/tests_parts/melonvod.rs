#[test]
fn melon_vod_native_extractor_maps_player_and_streaming_apis() {
    let extractor = MelonVodExtractor::new(ExtractorDescriptor::new(
        "MelonVODIE",
        "MelonVOD",
        r#"https?://vod\.melon\.com/video/detail2\.html?\?.*?mvId=(?P<id>[0-9]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "/video/playerInfo.json".to_owned(),
                r#"{"mvInfo":{"MVTITLE":"Native Melon MV"},"artistList":[{"ARTISTNAMEWEBLIST":"Jessica"},{"ARTISTNAMEWEBLIST":"Guest"}]}"#.as_bytes().to_vec(),
            ),
            (
                "/delivery/streamingInfo.json".to_owned(),
                r#"{"staticDomain":"https://cdn.example/","streamingInfo":{"encUrl":"https://cdn.example/melon/master.m3u8","imgPath":"thumb.jpg","playTime":"203","mvSvcOpenDt":"20161212000000"}}"#.as_bytes().to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "http://vod.melon.com/video/detail2.htm?mvId=50158734",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("50158734"));
    assert_eq!(result.get_str("title"), Some("Native Melon MV"));
    assert_eq!(result.get_str("artist"), Some("Jessica, Guest"));
    assert_eq!(result.get_str("thumbnail"), Some("https://cdn.example/thumb.jpg"));
    assert_eq!(result.get_str("upload_date"), Some("20161212"));
    assert_eq!(result.get_i64("duration"), Some(203));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/melon/master.m3u8"));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("protocol")),
        Some(&serde_json::json!("m3u8_native"))
    );
}
