#[test]
fn dailymail_native_extractor_maps_player_data_and_renditions() {
    let extractor = DailyMailExtractor::new(ExtractorDescriptor::new(
        "DailyMailIE",
        "DailyMail",
        r"https?://(?:www\.)?dailymail\.co\.uk/(?:video/[^/]+/video-|embed/video/)(?P<id>[0-9]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "dailymail.co.uk/video/tvshowbiz/video-1234567".to_owned(),
                br#"<div data-opts='{"title":"Native Daily Mail","descr":"Native &amp; description","poster":"https://cdn.example/dailymail.jpg","plugins":{"sources":{"url":"https://cdn.example/dailymail/sources.json"}}}'></div>"#.to_vec(),
            ),
            (
                "cdn.example/dailymail/sources.json".to_owned(),
                br#"{"renditions":[
                    {"url":"https://cdn.example/dailymail/720.m3u8","videoContainer":"M2TS","encodingRate":850000,"frameWidth":1280,"frameHeight":720,"videoCodec":"h264"},
                    {"url":"https://cdn.example/dailymail/480.mp4","videoContainer":"MP4","encodingRate":500000,"frameWidth":854,"frameHeight":480,"videoCodec":"h264"}
                ]}"#
                .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.dailymail.co.uk/video/tvshowbiz/video-1234567/native.html",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("1234567"));
    assert_eq!(result.get_str("title"), Some("Native Daily Mail"));
    assert_eq!(result.get_str("description"), Some("Native & description"));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/dailymail.jpg")
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/dailymail/720.m3u8")
    );
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(formats[0].get("format_id"), Some(&serde_json::json!("hls-850")));
    assert_eq!(formats[0].get("protocol"), Some(&serde_json::json!("m3u8_native")));
    assert_eq!(formats[0].get("tbr"), Some(&serde_json::json!(850)));
    assert_eq!(formats[1].get("format_id"), Some(&serde_json::json!("https-500")));
    assert_eq!(formats[1].get("width"), Some(&serde_json::json!(854)));
}
