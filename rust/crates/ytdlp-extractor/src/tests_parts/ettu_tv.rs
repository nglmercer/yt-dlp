#[test]
fn ettutv_native_extractor_posts_for_stream_and_maps_metadata() {
    let extractor = EttuTvExtractor::new(ExtractorDescriptor::new(
        "EttuTvIE",
        "EttuTv",
        r"https?://(?:www\.)?ettu\.tv/[^?#]+/playerpage/(?P<id>[0-9]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "www.ettu.tv/api/v3/contents/1573849/player-settings".to_owned(),
                br#"{
                    "streamAccess":"https://stream.example/ettu/access/1573849",
                    "title":"Ni Xia Lian - Shao Jieni",
                    "metaInformation":{"competition":"ITTF Europe Top 16 Cup"},
                    "image":"https://img.example/ettu/1573849.jpg",
                    "date":"2023-02-25T12:00:00Z",
                    "isLivestream":false
                }"#
                .to_vec(),
            ),
            (
                "stream.example/ettu/access/1573849".to_owned(),
                br#"{"data":{"stream":"https://cdn.example/ettu/1573849/playlist.m3u8"}}"#
                    .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://www.ettu.tv/en-int/playerpage/1573849", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("1573849"));
    assert_eq!(result.get_str("title"), Some("Ni Xia Lian - Shao Jieni"));
    assert_eq!(
        result.get_str("description"),
        Some("ITTF Europe Top 16 Cup")
    );
    assert_eq!(result.get_i64("timestamp"), Some(1677326400));
    assert_eq!(result.get("is_live"), Some(&serde_json::json!(false)));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/ettu/1573849/playlist.m3u8")
    );
    assert_eq!(result.get_str("ext"), Some("mp4"));
}
