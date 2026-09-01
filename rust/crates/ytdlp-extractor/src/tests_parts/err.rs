#[test]
fn err_jupiter_native_extractor_maps_vod_media_and_episode_metadata() {
    let extractor = ErrJupiterExtractor::new(ExtractorDescriptor::new(
        "ERRJupiterIE",
        "drtv",
        r"https?://(?:jupiter(?:pluss)?|lasteekraan)\.err\.ee/(?P<id>\d+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{"data":{"mainContent":{
            "heading":"Native Jupiter","subHeading":"Native episode","type":"episode",
            "lead":"<p>Native description</p>","created":1700000000,"updated":1700000100,
            "scheduleStart":1700000200,"year":2023,"rootContentId":"native-series",
            "season":2,"episode":7,"id":"native-episode",
            "medias":{"src":{"hls":"https://cdn.example/native.m3u8","dash":"https://cdn.example/native.mpd","file":"https://cdn.example/native.mp4"}}
        }}}"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://jupiter.err.ee/1609145945/native", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("1609145945"));
    assert_eq!(result.get_str("title"), Some("Native Jupiter"));
    assert_eq!(result.get_str("alt_title"), Some("Native episode"));
    assert_eq!(result.get_str("description"), Some("Native description"));
    assert_eq!(result.get_i64("timestamp"), Some(1_700_000_000));
    assert_eq!(result.get_i64("release_timestamp"), Some(1_700_000_200));
    assert_eq!(result.get_str("series"), Some("Native Jupiter"));
    assert_eq!(result.get_str("series_id"), Some("native-series"));
    assert_eq!(result.get_str("season"), Some("Season 2"));
    assert_eq!(result.get_i64("episode_number"), Some(7));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(3)
    );
}

#[test]
fn err_jupiter_native_extractor_marks_drm_as_todo() {
    let extractor = ErrJupiterExtractor::new(ExtractorDescriptor::new(
        "ERRJupiterIE",
        "drtv",
        r"https?://(?:jupiter(?:pluss)?|lasteekraan)\.err\.ee/(?P<id>\d+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{"data":{"mainContent":{"medias":{"restrictions":{"drm":true},"src":{"hls":"https://cdn.example/encrypted.m3u8"}}}}}"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context("https://jupiter.err.ee/1609145945/native", &context)
        .unwrap_err();

    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}
