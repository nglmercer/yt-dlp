#[test]
fn mangomolo_native_video_extractor_maps_hls_player_config() {
    let extractor = MangomoloVideoExtractor::new(ExtractorDescriptor::new(
        "MangomoloVideoIE",
        "mangomolo:video",
        r#"(?:https?:)?//(?:admin\.mangomolo\.com/analytics/index\.php/customers/embed/|player\.mangomolo\.com/v1/)video\?.*?\bid=(?P<id>\d+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "/v1/video?".to_owned(),
            br#"<input type="hidden" name="userid" value="168">
                <input type="hidden" name="duration" value="2760">
                <script>var file: "https://cdn.example/mangomolo/playlist.m3u8";</script>"#
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://player.mangomolo.com/v1/video?id=29431242&signature=abc",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("29431242"));
    assert_eq!(result.get_str("title"), Some("29431242"));
    assert_eq!(result.get_str("uploader_id"), Some("168"));
    assert_eq!(result.get_i64("duration"), Some(2760));
    assert_eq!(result.get_bool("is_live"), Some(false));
    assert_eq!(result.get_str("live_status"), Some("not_live"));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/mangomolo/playlist.m3u8")
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

#[test]
fn mangomolo_native_live_extractor_decodes_channel_id_and_uses_live_hls() {
    let extractor = MangomoloLiveExtractor::new(ExtractorDescriptor::new(
        "MangomoloLiveIE",
        "mangomolo:live",
        r#"(?:https?:)?//(?:admin\.mangomolo\.com/analytics/index\.php/customers/embed/|player\.mangomolo\.com/v1/)(?:live|index)\?.*?\bchannelid=(?P<id>(?:[A-Za-z0-9+/=]|%2B|%2F|%3D)+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "/v1/live?".to_owned(),
            br#"<input type="hidden" name="userid" value="streamer">
                <script>var src: "https://cdn.example/mangomolo/live/playlist.m3u8";</script>"#
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://player.mangomolo.com/v1/live?channelid=Q0M%3D&autoplay=true",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("CC"));
    assert_eq!(result.get_bool("is_live"), Some(true));
    assert_eq!(result.get_str("live_status"), Some("is_live"));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("protocol")),
        Some(&serde_json::json!("m3u8"))
    );
}
