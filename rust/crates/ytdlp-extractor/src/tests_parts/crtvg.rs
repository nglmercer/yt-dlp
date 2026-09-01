#[test]
fn crtvg_native_extractor_maps_fixed_manifests_and_legacy_archive_id() {
    let extractor = CrtvgExtractor::new(ExtractorDescriptor::new(
        "CrtvgIE",
        "Crtvg",
        r"https?://(?:www\.)?crtvg\.es/tvg/a-carta/(?P<id>[^/#?]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "crtvg.es/tvg/a-carta/os-caimans-do-tea-5839623".to_owned(),
            r#"<html><head>
                <meta property="og:title" content="Os caimáns do Tea | CRTVG">
                <meta name="description" content="Native CRTVG description">
                <meta property="og:image" content="https://cdn.example/crtvg.jpg">
                <script>var url = "https://cdn.example/crtvg/native-stream";</script>
            </head></html>"#
            .as_bytes()
            .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.crtvg.es/tvg/a-carta/os-caimans-do-tea-5839623",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("os-caimans-do-tea-5839623"));
    assert_eq!(result.get_str("title"), Some("Os caimáns do Tea"));
    assert_eq!(
        result.get_str("description"),
        Some("Native CRTVG description")
    );
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/crtvg.jpg")
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/crtvg/native-stream/playlist.m3u8")
    );
    assert_eq!(
        result.get("_old_archive_ids"),
        Some(&serde_json::json!(["crtvg 5839623"]))
    );
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(
        formats[1].get("protocol"),
        Some(&serde_json::json!("http_dash_segments"))
    );
}
