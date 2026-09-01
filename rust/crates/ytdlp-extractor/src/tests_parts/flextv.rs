#[test]
fn flextv_native_extractor_maps_live_api_sources() {
    let extractor = FlexTvExtractor::new(ExtractorDescriptor::new(
        "FlexTVIE",
        "ttinglive",
        r"https?://(?:www\.)?(?:ttinglive\.com|flextv\.co\.kr)/channels/(?P<id>\d+)/live",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{
            "sources":[
                {"format":"ivs","url":"https://cdn.example/flextv/ivs.m3u8"},
                {"urlDetail":{
                    "hls":{"resolution":{
                        "720":{"url":"https://cdn.example/flextv/720.m3u8","resolution":"720","suffixName":"720p"}
                    }},
                    "flv":{"resolution":{
                        "720":{"url":"https://cdn.example/flextv/720.flv","resolution":"720","suffixName":"720p"}
                    }}
                }}
            ],
            "stream":{"title":"Native FlexTV stream","createdAt":"2024-01-02T03:04:05Z"},
            "thumbUrl":"https://cdn.example/flextv/thumb.jpg",
            "owner":{"name":"Native channel","id":244396}
        }"#
        .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.flextv.co.kr/channels/231638/live",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("231638"));
    assert_eq!(result.get_str("title"), Some("Native FlexTV stream"));
    assert_eq!(result.get_i64("timestamp"), Some(1704164645));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/flextv/thumb.jpg")
    );
    assert_eq!(result.get_str("channel"), Some("Native channel"));
    assert_eq!(result.get_str("channel_id"), Some("244396"));
    assert_eq!(result.get_bool("is_live"), Some(true));
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 3);
    assert_eq!(formats[0].get("protocol"), Some(&serde_json::json!("m3u8_native")));
    assert_eq!(formats[1].get("height"), Some(&serde_json::json!(720)));
    assert_eq!(formats[2].get("ext"), Some(&serde_json::json!("flv")));
}

#[test]
fn flextv_native_extractor_reports_missing_sources() {
    let extractor = FlexTvExtractor::new(ExtractorDescriptor::new(
        "FlexTVIE",
        "ttinglive",
        r"https?://(?:www\.)?(?:ttinglive\.com|flextv\.co\.kr)/channels/(?P<id>\d+)/live",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{"sources":[]}"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context("https://ttinglive.com/channels/746/live", &context)
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Extraction);
    assert!(error.message.contains("no playable stream URLs"));
}
