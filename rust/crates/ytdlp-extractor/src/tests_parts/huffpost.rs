#[test]
fn huffpost_native_extractor_maps_segment_sources_and_thumbnails() {
    let extractor = HuffPostExtractor::new(ExtractorDescriptor::new(
        "HuffPostIE",
        "HuffPost",
        r#"(?x)https?://(embed\.)?live\.huffingtonpost\.com/(?:r/segment/[^/]+/|HPLEmbedPlayer/\?segmentId=)(?P<id>[0-9a-f]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "api/segments/52dd3e4b02a7602131000677.json".to_owned(),
                br#"{"data":{"title":"Native HuffPost segment","description":"Native description","running_time":"00:25:49","schedule":{"starts_at":"2014-01-24T12:00:00Z"},"images":{"small":"https://cdn.example/segment-320x180.jpg"},"sources":{"live":{"video/mp4":"https://cdn.example/live.mp4","video/hls":"https://cdn.example/live.m3u8","legacy":"https://cdn.example/live.f4m"},"live_again":{}}}}"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "http://live.huffingtonpost.com/r/segment/legalese/52dd3e4b02a7602131000677",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("52dd3e4b02a7602131000677"));
    assert_eq!(result.get_str("title"), Some("Native HuffPost segment"));
    assert_eq!(result.get_f64("duration"), Some(1549.0));
    assert_eq!(result.get_str("upload_date"), Some("20140124"));
    assert_eq!(result.get_i64("timestamp"), Some(1_390_564_800));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/segment-320x180.jpg")
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
}
