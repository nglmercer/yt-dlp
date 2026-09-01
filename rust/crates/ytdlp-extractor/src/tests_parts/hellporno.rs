#[test]
fn hellporno_native_extractor_maps_html5_video_metadata() {
    let extractor = HellPornoExtractor::new(ExtractorDescriptor::new(
        "HellPornoIE",
        "HellPorno",
        r#"https?://(?:www\.)?hellporno\.(?:com/videos|net/v)/(?P<id>[^/]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<html><head>
            <title>Native video - Hell Porno</title>
            <meta name="keywords" content="category one, category two">
            <meta property="og:image" content="https://cdn.example/native.jpg">
            <meta property="video:duration" content="240">
            <meta property="video:release_date" content="2014-04-29T00:00:00Z">
        </head><body>
            <div class="desc_video_view_v2">Native description</div>
            <video><source src="/videos/native.mp4" type="video/mp4"></video>
            <span>Views 1234</span>
            <script>chs_object = "149116";</script>
        </body></html>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://hellporno.com/videos/native-video/",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("149116"));
    assert_eq!(result.get_str("title"), Some("Native video"));
    assert_eq!(
        result.get_str("description"),
        Some("Native description")
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://hellporno.com/videos/native.mp4")
    );
    assert_eq!(result.get_str("thumbnail"), Some("https://cdn.example/native.jpg"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(240.0)));
    assert_eq!(result.get("timestamp"), Some(&serde_json::json!(1398729600)));
    assert_eq!(result.get("upload_date"), Some(&serde_json::json!("20140429")));
    assert_eq!(result.get("view_count"), Some(&serde_json::json!(1234)));
    assert_eq!(result.get("age_limit"), Some(&serde_json::json!(18)));
    assert_eq!(
        result
            .get("categories")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
}

#[test]
fn hellporno_native_extractor_requires_html5_media() {
    let extractor = HellPornoExtractor::new(ExtractorDescriptor::new(
        "HellPornoIE",
        "HellPorno",
        r#"https?://(?:www\.)?hellporno\.(?:com/videos|net/v)/(?P<id>[^/]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: b"<html><title>Native video - Hell Porno</title></html>".to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context(
            "https://hellporno.com/videos/native-video/",
            &context,
        )
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Extraction);
    assert!(error.message.contains("no HTML5 media formats"));
}
