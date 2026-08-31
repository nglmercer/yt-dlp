#[test]
fn unistra_native_extractor_maps_progressive_files_and_metadata() {
    let extractor = UnistraExtractor::new(ExtractorDescriptor::new(
        "UnistraIE",
        "Unistra",
        r#"https?://utv\.unistra\.fr/(?:index|video)\.php\?id_video=(?P<id>\d+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "utv.unistra.fr/video.php?id_video=154".to_owned(),
            br#"<html><head>
                    <title>UTV - Native UTV title</title>
                    <meta name="Description" content="Native &amp; precise description">
                </head><body>
                    <script>
                        player({file: "/videos/154.mp4"});
                        player({file : "/videos/154.mp4"});
                        player({file: "/videos/154-HD.mp4"});
                        image: "https://img.example/unistra/154.jpg"
                    </script>
                </body></html>"#
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "http://utv.unistra.fr/video.php?id_video=154",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("154"));
    assert_eq!(result.get_str("title"), Some("Native UTV title"));
    assert_eq!(
        result.get_str("description"),
        Some("Native & precise description")
    );
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://img.example/unistra/154.jpg")
    );
    assert_eq!(
        result.get_str("url"),
        Some("http://vod-flash.u-strasbg.fr:8080/videos/154.mp4")
    );
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(formats[0].get("format_id"), Some(&serde_json::json!("SD")));
    assert_eq!(formats[0].get("quality"), Some(&serde_json::json!(0)));
    assert_eq!(formats[1].get("format_id"), Some(&serde_json::json!("HD")));
    assert_eq!(formats[1].get("quality"), Some(&serde_json::json!(1)));
}
