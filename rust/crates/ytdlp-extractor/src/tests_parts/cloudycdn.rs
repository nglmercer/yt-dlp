#[test]
fn cloudycdn_native_extractor_maps_player_hls_and_metadata() {
    let extractor = CloudyCdnExtractor::new(ExtractorDescriptor::new(
        "CloudyCDNIE",
        "CloudyCDN",
        r"(?:https?:)?//embed\.(?P<domain>cloudycdn\.services|backscreen\.com)/(?P<site_id>[^/?#]+)/media/(?P<id>[\w-]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "player.cloudycdn.services/player/ltv/media/46k_d23-6000-105/".to_owned(),
            br#"{
                "name":"Native Cloudy title",
                "duration":1442,
                "upload_date":"2023-11-21T00:00:00Z",
                "source":{
                    "poster":"https://cdn.example/cloudy/poster.jpg",
                    "sources":[
                        {"src":"https://cdn.example/cloudy/master.m3u8"},
                        {"src":"https://cdn.example/cloudy/chunklist_b1_vo_.m3u8"}
                    ]
                }
            }"#
            .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://embed.cloudycdn.services/ltv/media/46k_d23-6000-105?",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("46k_d23-6000-105"));
    assert_eq!(result.get_str("title"), Some("Native Cloudy title"));
    assert_eq!(result.get_i64("duration"), Some(1442));
    assert_eq!(result.get_i64("timestamp"), Some(1_700_524_800));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/cloudy/poster.jpg")
    );
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(formats[0].get("format_id"), Some(&serde_json::json!("hls")));
    assert_eq!(formats[1].get("acodec"), Some(&serde_json::json!("none")));
}

#[test]
fn cloudycdn_native_extractor_marks_non_hls_source_as_todo() {
    let extractor = CloudyCdnExtractor::new(ExtractorDescriptor::new(
        "CloudyCDNIE",
        "CloudyCDN",
        r"(?:https?:)?//embed\.(?P<domain>cloudycdn\.services|backscreen\.com)/(?P<site_id>[^/?#]+)/media/(?P<id>[\w-]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "player.backscreen.com/player/ltv/media/unsupported/".to_owned(),
            br#"{"source":{"sources":[{"src":"https://cdn.example/cloudy/video.mp4"}]}}"#
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context(
            "https://embed.backscreen.com/ltv/media/unsupported",
            &context,
        )
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}
