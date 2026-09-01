#[test]
fn espn_cricinfo_native_extractor_maps_playbacks_and_metadata() {
    let extractor = EspnCricinfoExtractor::new(ExtractorDescriptor::new(
        "ESPNCricInfoIE",
        "ESPNCricInfo",
        r"https?://(?:www\.)?espncricinfo\.com/(?:cricket-)?videos?/[^#$&?/]+-(?P<id>\d+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{"video":{
            "title":"Native Cricinfo video","summary":"Native summary",
            "publishedAt":"2023-01-28T12:00:00Z","duration":96,
            "playbacks":[
                {"type":"HLS","url":"https://cdn.example/cricinfo.m3u8"},
                {"type":"AUDIO","url":"https://cdn.example/cricinfo.m4a"},
                {"type":"UNKNOWN","url":"https://cdn.example/ignored"}
            ]
        }}"#
        .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.espncricinfo.com/video/native-cricinfo-video-1356225",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("1356225"));
    assert_eq!(result.get_str("title"), Some("Native Cricinfo video"));
    assert_eq!(result.get_str("description"), Some("Native summary"));
    assert_eq!(result.get_str("upload_date"), Some("20230128"));
    assert!(result.get_i64("timestamp").is_some());
    assert_eq!(result.get_f64("duration"), Some(96.0));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.get(1))
            .and_then(|format| format.get("vcodec")),
        Some(&serde_json::json!("none"))
    );
}

#[test]
fn espn_cricinfo_native_extractor_reports_missing_playbacks() {
    let extractor = EspnCricinfoExtractor::new(ExtractorDescriptor::new(
        "ESPNCricInfoIE",
        "ESPNCricInfo",
        r"https?://(?:www\.)?espncricinfo\.com/(?:cricket-)?videos?/[^#$&?/]+-(?P<id>\d+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{"video":{"title":"No playback","playbacks":[]}}"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context(
            "https://www.espncricinfo.com/video/no-playback-1",
            &context,
        )
        .unwrap_err();

    assert_eq!(error.kind, ExtractorErrorKind::Extraction);
    assert!(error.message.contains("no playable playback URLs"));
}
