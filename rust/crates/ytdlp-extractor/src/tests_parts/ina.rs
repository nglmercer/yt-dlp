#[test]
fn ina_native_extractor_maps_asset_api_metadata_and_media() {
    let extractor = InaExtractor::new(ExtractorDescriptor::new(
        "InaIE",
        "Ina",
        r#"https?://(?:(?:www|m)\.)?ina\.fr/(?:[^?#]+/)(?P<id>[\w-]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "www.ina.fr/video/I12055569".to_owned(),
                br#"<html><body>
                    <div asset-details-url="https://api.ina.example/assets/I12055569"></div>
                </body></html>"#
                    .to_vec(),
            ),
            (
                "api.ina.example/assets/I12055569.json".to_owned(),
                br#"{
                    "resourceUrl":"https://cdn.ina.example/video/I12055569.mp4",
                    "type":"video",
                    "title":"Native INA title",
                    "description":"Native INA description",
                    "dateOfBroadcast":"2007-07-12",
                    "duration":123.5,
                    "resourceThumbnail":"https://cdn.ina.example/I12055569.jpg"
                }"#
                .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.ina.fr/video/I12055569/francois-hollande-video.html",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("I12055569"));
    assert_eq!(result.get_str("url"), Some("https://cdn.ina.example/video/I12055569.mp4"));
    assert_eq!(result.get_str("ext"), Some("mp4"));
    assert_eq!(result.get_str("title"), Some("Native INA title"));
    assert_eq!(result.get_str("description"), Some("Native INA description"));
    assert_eq!(result.get_str("upload_date"), Some("20070712"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(123.5)));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.ina.example/I12055569.jpg")
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("protocol")),
        Some(&serde_json::json!("http"))
    );
}

#[test]
fn ina_native_extractor_marks_unknown_media_types_as_todo() {
    let extractor = InaExtractor::new(ExtractorDescriptor::new(
        "InaIE",
        "Ina",
        r#"https?://(?:(?:www|m)\.)?ina\.fr/(?:[^?#]+/)(?P<id>[\w-]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "www.ina.fr/video/unknown".to_owned(),
                br#"<div asset-details-url="https://api.ina.example/assets/unknown"></div>"#
                    .to_vec(),
            ),
            (
                "api.ina.example/assets/unknown.json".to_owned(),
                br#"{"resourceUrl":"https://cdn.ina.example/unknown.bin","type":"document"}"#
                    .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context("https://www.ina.fr/video/unknown", &context)
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}
