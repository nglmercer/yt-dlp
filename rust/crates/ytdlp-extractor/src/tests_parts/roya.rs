#[test]
fn roya_live_native_extractor_maps_stream_and_channel_title() {
    let extractor = RoyaLiveExtractor::new(ExtractorDescriptor::new(
        "RoyaLiveIE",
        "RoyaLive",
        r"https?://(?:en\.)?roya\.tv/live-stream/(?P<id>\d+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "ticket.roya-tv.com/api/v5/fastchannel/21".to_owned(),
                br#"{"data":{"secured_url":"https://cdn.example/roya/channel-21/index.m3u8"}}"#.to_vec(),
            ),
            (
                "backend.roya.tv/api/v01/channels/schedule-pagination".to_owned(),
                br#"{"data":[{"channel":{"id":21,"title":"Roya News"}}]}"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://roya.tv/live-stream/21", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("21"));
    assert_eq!(result.get_str("title"), Some("Roya News"));
    assert_eq!(result.get("is_live"), Some(&serde_json::json!(true)));
    assert_eq!(result.get_str("live_status"), Some("is_live"));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/roya/channel-21/index.m3u8")
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("is_live")),
        Some(&serde_json::json!(true))
    );
}

#[test]
fn roya_live_native_extractor_allows_missing_schedule_title() {
    let extractor = RoyaLiveExtractor::new(ExtractorDescriptor::new(
        "RoyaLiveIE",
        "RoyaLive",
        r"https?://(?:en\.)?roya\.tv/live-stream/(?P<id>\d+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "ticket.roya-tv.com/api/v5/fastchannel/1".to_owned(),
                br#"{"data":{"secured_url":"https://cdn.example/roya/channel-1/live.m3u8"}}"#.to_vec(),
            ),
            (
                "backend.roya.tv/api/v01/channels/schedule-pagination".to_owned(),
                b"not-json".to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://en.roya.tv/live-stream/1", &context)
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_str("id"), Some("1"));
    assert!(result.get("title").is_none());
}
