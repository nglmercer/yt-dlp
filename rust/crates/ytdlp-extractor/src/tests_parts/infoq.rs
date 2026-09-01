#[test]
fn infoq_native_extractor_maps_http_audio_video_and_explicit_rtmp_format() {
    let extractor = InfoqExtractor::new(ExtractorDescriptor::new(
        "InfoQIE",
        "InfoQ",
        r#"https?://(?:www\.)?infoq\.com/(?:[^/]+/)+(?P<id>[^/]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "www.infoq.com/presentations/native-infoq".to_owned(),
            br#"<html><head>
                <title>Native InfoQ presentation</title>
                <meta name="description" content="Native InfoQ description">
            </head><body>
                <script>
                    jsclassref = 'dGFsa19wYXRo';
                    P.s = 'https://cdn.example/infoq.mp4';
                    InfoQConstants.scp = 'policy';
                    InfoQConstants.scs = 'signature';
                    InfoQConstants.sck = 'key';
                </script>
                <form id="mp3Form">
                    <input type="hidden" name="filename" value="native.mp3">
                </form>
            </body></html>"#
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.infoq.com/presentations/native-infoq",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("native-infoq"));
    assert_eq!(result.get_str("title"), Some("Native InfoQ presentation"));
    assert_eq!(result.get_str("description"), Some("Native InfoQ description"));
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 3);
    assert_eq!(formats[0].get("play_path"), Some(&serde_json::json!("mp4:talk_path")));
    assert!(formats[1]
        .get("url")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|url| url.contains("Policy=policy")));
    assert_eq!(formats[2].get("vcodec"), Some(&serde_json::json!("none")));
}
