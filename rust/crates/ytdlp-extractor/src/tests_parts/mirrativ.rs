#[test]
fn mirrativ_native_extractor_maps_live_api_and_page_metadata() {
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "mirrativ.com/live/native-live".to_owned(),
                r#"<meta property="og:title" content="Native Mirrativ live">"#.as_bytes().to_vec(),
            ),
            (
                "api/live/live?live_id=native-live".to_owned(),
                br#"{
                    "is_live": true,
                    "is_archive": false,
                    "streaming_url_hls": "https://cdn.example/mirrativ/native-live.m3u8",
                    "description": "Native Mirrativ description",
                    "image_url": "https://cdn.example/mirrativ/native-live.jpg",
                    "created_at": 1646229167,
                    "started_at": 1646229192,
                    "total_viewer_num": 1241,
                    "owner": {"name": "Native streamer", "user_id": 118572165}
                }"#
                .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = MirrativExtractor::new(ExtractorDescriptor::new(
        "MirrativIE",
        "mirrativ",
        r#"https?://(?:www\.)?mirrativ\.com/live/(?P<id>[^/?#&]+)"#,
        true,
    ))
    .unwrap()
    .extract_with_context(
        "https://mirrativ.com/live/native-live",
        &context,
    )
    .unwrap()
    .into_info_dict();

    assert_eq!(result.get_str("id"), Some("native-live"));
    assert_eq!(result.get_str("title"), Some("Native Mirrativ live"));
    assert_eq!(result.get_bool("is_live"), Some(true));
    assert_eq!(result.get_str("uploader"), Some("Native streamer"));
    assert_eq!(result.get_str("uploader_id"), Some("118572165"));
    assert_eq!(result.get_i64("view_count"), Some(1241));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/mirrativ/native-live.m3u8")
    );
}

#[test]
fn mirrativ_user_native_extractor_builds_history_playlist() {
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "api/user/profile?user_id=118572165".to_owned(),
                br#"{"name":"Native user","description":"Native profile"}"#.to_vec(),
            ),
            (
                "api/live/live_history?user_id=118572165&page=1".to_owned(),
                br#"{
                    "lives": [
                        {"live_id":"native-archive","title":"Native archive","is_archive":true,"is_live":false},
                        {"live_id":"native-skip","is_archive":false,"is_live":false}
                    ],
                    "next_page": null
                }"#
                .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = MirrativUserExtractor::new(ExtractorDescriptor::new(
        "MirrativUserIE",
        "mirrativ:user",
        r#"https?://(?:www\.)?mirrativ\.com/user/(?P<id>\d+)"#,
        true,
    ))
    .unwrap()
    .extract_with_context("https://www.mirrativ.com/user/118572165", &context)
    .unwrap()
    .into_info_dict();

    assert_eq!(result.get_str("id"), Some("118572165"));
    assert_eq!(result.get_str("title"), Some("Native user"));
    let entries = result
        .get("entries")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0].get("url"),
        Some(&serde_json::json!("https://www.mirrativ.com/live/native-archive"))
    );
    assert_eq!(
        entries[0].get("ie_key"),
        Some(&serde_json::json!("Mirrativ"))
    );
}
