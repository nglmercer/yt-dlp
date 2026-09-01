#[test]
fn agalega_native_extractor_maps_content_and_hls_resources() {
    let extractor = AGalegaExtractor::new(ExtractorDescriptor::new(
        "AGalegaIE",
        "agalega:videos",
        r"https?://(?:www\.)?agalega\.gal/videos/(?:detail/)?(?P<id>[0-9]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "www.agalega.gal/api/fetch-api/jwt/token".to_owned(),
                br#"{"access":"native-token"}"#.to_vec(),
            ),
            (
                "api-agalega.interactvty.com/api/2.0/contents/content/288664/".to_owned(),
                br#"{
                    "name":"Native A Galega title",
                    "description":"Native A Galega description",
                    "image":"https://cdn.example/agalega/poster.png"
                }"#
                .to_vec(),
            ),
            (
                "api-agalega.interactvty.com/api/2.0/contents/content_resources/288664/"
                    .to_owned(),
                br#"{"results":[
                    {"media_url":"https://cdn.example/agalega/master.m3u8"}
                ]}"#
                .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://www.agalega.gal/videos/288664-native", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("288664"));
    assert_eq!(result.get_str("title"), Some("Native A Galega title"));
    assert_eq!(
        result.get_str("description"),
        Some("Native A Galega description")
    );
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/agalega/poster.png")
    );
    assert_eq!(result.get_str("url"), Some("https://cdn.example/agalega/master.m3u8"));
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
fn agalega_native_extractor_marks_non_hls_resources_as_todo() {
    let extractor = AGalegaExtractor::new(ExtractorDescriptor::new(
        "AGalegaIE",
        "agalega:videos",
        r"https?://(?:www\.)?agalega\.gal/videos/(?:detail/)?(?P<id>[0-9]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "www.agalega.gal/api/fetch-api/jwt/token".to_owned(),
                br#"{"access":"native-token"}"#.to_vec(),
            ),
            (
                "api-agalega.interactvty.com/api/2.0/contents/content/99/".to_owned(),
                br#"{}"#.to_vec(),
            ),
            (
                "api-agalega.interactvty.com/api/2.0/contents/content_resources/99/"
                    .to_owned(),
                br#"{"results":[{"media_url":"https://cdn.example/agalega/video.mp4"}]}"#
                    .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context("https://agalega.gal/videos/99-title", &context)
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}
