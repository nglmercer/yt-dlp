#[test]
fn ivideon_native_extractor_builds_live_quality_formats_and_metadata() {
    let extractor = IvideonExtractor::new(ExtractorDescriptor::new(
        "IvideonIE",
        "ivideon",
        r#"https?://(?:www\.)?ivideon\.com/tv/(?:[^/]+/)*camera/(?P<id>\d+-[\da-f]+)/(?P<camera_id>\d+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "www.ivideon.com/tv/camera/100-916ca13b5c4ad9f564266424a026386d/0/"
                .to_owned(),
            br#"<script>
                var config = {
                    "ivTvAppOptions": {
                        "currentCameraInfo": {
                            "camera_name": "Native Ivideon camera",
                            "misc": {"description": "Native Ivideon description"}
                        }
                    }
                };
            </script>"#
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.ivideon.com/tv/camera/100-916ca13b5c4ad9f564266424a026386d/0/",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(
        result.get_str("id"),
        Some("100-916ca13b5c4ad9f564266424a026386d")
    );
    assert_eq!(result.get_str("title"), Some("Native Ivideon camera"));
    assert_eq!(
        result.get_str("description"),
        Some("Native Ivideon description")
    );
    assert_eq!(result.get_bool("is_live"), Some(true));
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 3);
    assert_eq!(formats[0].get("quality"), Some(&serde_json::json!(0)));
    assert!(formats[2]
        .get("url")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|url| url.contains("q=hi")));
}
