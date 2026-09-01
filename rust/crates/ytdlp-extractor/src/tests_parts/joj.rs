#[test]
fn joj_native_extractor_maps_embed_bitrates_and_metadata() {
    let extractor = JojExtractor::new(ExtractorDescriptor::new(
        "JojIE",
        "Joj",
        r#"(?x)(?:joj:|https?://media\.joj\.sk/embed/)(?P<id>[^/?#^]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "media.joj.sk/embed/native-joj".to_owned(),
            br#"<html><head>
                <meta property="og:image" content="https://cdn.example/joj.jpg">
            </head><body>
                <script>
                    videoTitle: "Native JOJ title",
                    videoDuration: 3937,
                    src = {mp4: [
                        "https://cdn.example/joj-720p.mp4",
                        "https://cdn.example/joj-360p.mp4"
                    ]};
                </script>
            </body></html>"#
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("joj:native-joj", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("native-joj"));
    assert_eq!(result.get_str("title"), Some("Native JOJ title"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(3937)));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/joj.jpg")
    );
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(formats[0].get("height"), Some(&serde_json::json!(720)));
}

#[test]
fn joj_native_extractor_uses_xml_file_fallback() {
    let extractor = JojExtractor::new(ExtractorDescriptor::new(
        "JojIE",
        "Joj",
        r#"(?x)(?:joj:|https?://media\.joj\.sk/embed/)(?P<id>[^/?#^]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "media.joj.sk/embed/native-xml".to_owned(),
                br#"<title>Native XML JOJ</title>"#.to_vec(),
            ),
            (
                "media.joj.sk/services/Video.php?clip=native-xml".to_owned(),
                br#"<playlist><files>
                    <file path="dat/native/video.mp4" id="720p"/>
                </files></playlist>"#
                    .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("joj:native-xml", &context)
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_str("id"), Some("native-xml"));
    assert_eq!(result.get_str("title"), Some("Native XML JOJ"));
    assert_eq!(
        result.get_str("url"),
        Some("http://n16.joj.sk/storage/native/video.mp4")
    );
}
