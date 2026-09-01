#[test]
fn fifa_native_extractor_resolves_preplay_hls_and_metadata() {
    let extractor = FifaExtractor::new(ExtractorDescriptor::new(
        "FifaIE",
        "Fifa",
        r"https?://www\.fifa\.com/fifaplus/\w{2}/watch/([^#?]+/)?(?P<id>\w+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "fifa.com/fifaplus/en/watch/demo123".to_owned(),
                br#"<link rel="preconnect" href="https://api.example/fifa">"#.to_vec(),
            ),
            (
                "api.example/fifa/sections/videoDetails/demo123".to_owned(),
                br#"{
                    "title":"Brazil v Germany",
                    "description":"Native FIFA description",
                    "duration":"902",
                    "dateOfRelease":"2014-07-08",
                    "videoCategory":"FIFA Tournaments",
                    "videoSubcategory":"Highlights",
                    "backgroundImage":{"src":"https://cdn.example/thumb.jpg"}
                }"#
                .to_vec(),
            ),
            (
                "api.example/fifa/videoPlayerData/demo123".to_owned(),
                br#"{"preplayParameters":{"contentId":"content-123","queryStr":"a=1&b=2","signature":"sig-123"}}"#
                    .to_vec(),
            ),
            (
                "content.uplynk.com/preplay/content-123/multiple.json?a=1&b=2&sig=sig-123"
                    .to_owned(),
                br#"{"playURL":"https://cdn.example/fifa/master.m3u8"}"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://www.fifa.com/fifaplus/en/watch/demo123", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("demo123"));
    assert_eq!(result.get_str("title"), Some("Brazil v Germany"));
    assert_eq!(
        result.get_str("description"),
        Some("Native FIFA description")
    );
    assert_eq!(result.get_i64("duration"), Some(902));
    assert_eq!(result.get_i64("release_timestamp"), Some(1404777600));
    assert_eq!(
        result.get("categories"),
        Some(&serde_json::json!(["FIFA Tournaments", "Highlights"]))
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/fifa/master.m3u8")
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("protocol")),
        Some(&serde_json::json!("m3u8_native"))
    );
}

#[test]
fn fifa_native_extractor_requires_playback_url() {
    let extractor = FifaExtractor::new(ExtractorDescriptor::new(
        "FifaIE",
        "Fifa",
        r"https?://www\.fifa\.com/fifaplus/\w{2}/watch/([^#?]+/)?(?P<id>\w+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "fifa.com/fifaplus/en/watch/missing".to_owned(),
                br#"<link rel="preconnect" href="https://api.example/fifa">"#.to_vec(),
            ),
            (
                "api.example/fifa/videoPlayerData/missing".to_owned(),
                br#"{"preplayParameters":{"contentId":"content-missing","queryStr":"x=1","signature":"sig"}}"#
                    .to_vec(),
            ),
            (
                "content.uplynk.com/preplay/content-missing".to_owned(),
                br#"{"playURL":""}"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context("https://www.fifa.com/fifaplus/en/watch/missing", &context)
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Extraction);
    assert!(error.message.contains("no playable HLS URL"));
}
