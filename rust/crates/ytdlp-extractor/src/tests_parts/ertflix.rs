#[test]
fn ertflix_codename_native_extractor_maps_api_media_files() {
    let extractor = ErtflixCodenameExtractor::new(ExtractorDescriptor::new(
        "ERTFlixCodenameIE",
        "ertflix:codename",
        r"ertflix:(?P<id>[\w-]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{"Result":{"Success":true},"MediaFiles":[
            {"RoleCodename":"trailer","Formats":[{"Id":"trailer","Url":"https://cdn.example/trailer.mp4"}]},
            {"RoleCodename":"main","Formats":[
                {"Id":"hls-main","Url":"https://cdn.example/main.m3u8"},
                {"Id":"dash-main","Url":"https://cdn.example/main.mpd"},
                {"Id":"http-main","Url":"https://cdn.example/main.mp4"}
            ]}
        ]}"#
        .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("ertflix:monogramma-praxitelis-tzanoylinos", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(
        result.get_str("id"),
        Some("monogramma-praxitelis-tzanoylinos")
    );
    assert_eq!(
        result.get_str("title"),
        Some("monogramma-praxitelis-tzanoylinos")
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(3)
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.get(1))
            .and_then(|format| format.get("protocol")),
        Some(&serde_json::json!("http_dash_segments"))
    );
}

#[test]
fn ertflix_codename_native_extractor_reports_api_failure() {
    let extractor = ErtflixCodenameExtractor::new(ExtractorDescriptor::new(
        "ERTFlixCodenameIE",
        "ertflix:codename",
        r"ertflix:(?P<id>[\w-]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{"Result":{"Success":false}}"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context("ertflix:missing", &context)
        .unwrap_err();

    assert_eq!(error.kind, ExtractorErrorKind::Extraction);
    assert!(error.message.contains("rejected"));
}
