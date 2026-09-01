#[test]
fn espn_native_extractor_recurses_source_links_and_maps_metadata() {
    let extractor = EspnExtractor::new(ExtractorDescriptor::new(
        "ESPNIE",
        "ESPN",
        r#"(?x)
            https?://
                (?:
                    (?:
                        (?:
                            (?:(?:\w+\.)+)?espn\.go|
                            (?:www\.)?espn
                        )\.com/
                        (?:
                            (?:
                                video/(?:clip|iframe/twitter)|
                            )
                            (?:
                                .*?\?.*?\bid=|
                                /_/id/
                            )|
                            [^/]+/video/
                        )
                    )|
                    (?:www\.)espnfc\.(?:com|us)/(?:video/)?[^/]+/\d+/video/
                )
                (?P<id>\d+)
        "#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{"videos":[{
            "headline":"Native ESPN clip","caption":"Native caption",
            "thumbnail":"https://cdn.example/espn.jpg","duration":1302,
            "originalPublishDate":"2014-01-28T12:00:00Z",
            "links":{
                "source":{
                    "hls":"https://cdn.example/master.m3u8",
                    "mezzanine":{"mp4":"https://cdn.example/1080p30_1500k.mp4"},
                    "nested":{"720p60_900k":"https://cdn.example/720p60_900k.mp4"}
                },
                "mobile":{"small":"https://cdn.example/mobile.mp4"}
            }
        }]}"#
        .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.espn.com/video/clip?id=10365079",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("10365079"));
    assert_eq!(result.get_str("title"), Some("Native ESPN clip"));
    assert_eq!(result.get_str("description"), Some("Native caption"));
    assert_eq!(result.get_i64("duration"), Some(1302));
    assert_eq!(result.get_str("upload_date"), None);
    assert!(result.get_i64("timestamp").is_some());
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(4)
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.iter().find(|format| {
                format.get("format_id") == Some(&serde_json::json!("mezzanine"))
            }))
            .and_then(|format| format.get("quality")),
        Some(&serde_json::json!(1))
    );
}

#[test]
fn espn_native_extractor_marks_f4m_sources_as_todo() {
    let extractor = EspnExtractor::new(ExtractorDescriptor::new(
        "ESPNIE",
        "ESPN",
        r"https?://(?:www\.)?espn\.com/video/clip\?id=(?P<id>\d+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{"videos":[{"headline":"HDS clip","links":{"source":{"hds":"https://cdn.example/stream.f4m"}}}]}"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context("https://www.espn.com/video/clip?id=1", &context)
        .unwrap_err();

    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}
