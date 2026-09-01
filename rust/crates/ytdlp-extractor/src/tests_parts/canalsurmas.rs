#[test]
fn canalsurmas_native_extractor_maps_api_metadata_and_streams() {
    let extractor = CanalsurmasExtractor::new(ExtractorDescriptor::new(
        "CanalsurmasIE",
        "Canalsurmas",
        r"https?://(?:www\.)?canalsurmas\.es/videos/(?P<id>\d+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "api-rtva.interactvty.com/jwt/token/".to_owned(),
                br#"{"access":"native-token"}"#.to_vec(),
            ),
            (
                "api-rtva.interactvty.com/api/2.0/contents/content/44006/".to_owned(),
                r#"{
                    "name":"Lora del Río (Sevilla)",
                    "description":"Native Canal Sur description",
                    "image":"https://cdn.example/canalsurmas/poster.jpg",
                    "duration":321.5,
                    "created_at":"2022-03-24T10:39:42Z",
                    "tags":["Andalucía","native"]
                }"#
                .as_bytes()
                .to_vec(),
            ),
            (
                "api-rtva.interactvty.com/api/2.0/contents/content_resources/44006/".to_owned(),
                br#"{"results":[
                    {"media_url":"https://cdn.example/canalsurmas/master.m3u8"},
                    {"media_url":"https://cdn.example/canalsurmas/video.mp4"}
                ]}"#
                .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.canalsurmas.es/videos/44006-el-gran-queo-1-lora-del-rio-sevilla-20072014",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("44006"));
    assert_eq!(result.get_str("title"), Some("Lora del Río (Sevilla)"));
    assert_eq!(
        result.get_str("description"),
        Some("Native Canal Sur description")
    );
    assert_eq!(result.get_f64("duration"), Some(321.5));
    assert_eq!(result.get_i64("timestamp"), Some(1_648_118_382));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/canalsurmas/poster.jpg")
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/canalsurmas/master.m3u8")
    );
    assert_eq!(
        result.get("tags"),
        Some(&serde_json::json!(["Andalucía", "native"]))
    );
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(formats[0].get("format_id"), Some(&serde_json::json!("hls")));
    assert_eq!(formats[1].get("ext"), Some(&serde_json::json!("mp4")));
}

#[test]
fn canalsurmas_native_extractor_marks_unsupported_streams_as_todo() {
    let extractor = CanalsurmasExtractor::new(ExtractorDescriptor::new(
        "CanalsurmasIE",
        "Canalsurmas",
        r"https?://(?:www\.)?canalsurmas\.es/videos/(?P<id>\d+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "api-rtva.interactvty.com/jwt/token/".to_owned(),
                br#"{"access":"native-token"}"#.to_vec(),
            ),
            (
                "api-rtva.interactvty.com/api/2.0/contents/content/99/".to_owned(),
                br#"{"name":"Unsupported"}"#.to_vec(),
            ),
            (
                "api-rtva.interactvty.com/api/2.0/contents/content_resources/99/".to_owned(),
                br#"{"results":[{"media_url":"https://cdn.example/live/stream.f4m"}]}"#
                    .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context("https://canalsurmas.es/videos/99-title", &context)
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}
