#[test]
fn internazionale_native_extractor_maps_data_attributes_and_manifests() {
    let extractor = InternazionaleExtractor::new(ExtractorDescriptor::new(
        "InternazionaleIE",
        "Internazionale",
        r#"https?://(?:www\.)?internazionale\.it/video/(?:[^/]+/)*(?P<id>[^/?#&]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "internazionale.it/video/2018/08/29/native-video".to_owned(),
            br#"<html><head>
                <meta property="og:title" content="Native Internazionale title">
                <meta property="og:description" content="Native Internazionale description">
                <meta property="og:image" content="https://cdn.example/native.jpg">
                <meta property="article:published_time" content="2018-08-29T12:00:00Z">
            </head><body>
                <div data-video-title="Native Internazionale title"
                     data-job-id="761344"
                     data-video-path="2018/08/29"
                     data-video-available_abroad="0"></div>
            </body></html>"#
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.internazionale.it/video/2018/08/29/native-video",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("761344"));
    assert_eq!(result.get_str("display_id"), Some("native-video"));
    assert_eq!(result.get_str("title"), Some("Native Internazionale title"));
    assert_eq!(
        result.get_str("description"),
        Some("Native Internazionale description")
    );
    assert_eq!(result.get_str("upload_date"), Some("20180829"));
    assert_eq!(
        result.get_str("url"),
        Some("https://video-ita.internazionale.it/2018/08/29/761344.m3u8")
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
