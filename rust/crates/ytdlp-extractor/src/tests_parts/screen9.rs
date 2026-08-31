#[test]
fn screen9_native_extractor_maps_hls_mp4_and_plugin_metadata() {
    let extractor = Screen9Extractor::new(ExtractorDescriptor::new(
        "Screen9IE",
        "Screen9",
        r"https?://(?:\w+\.screen9\.(?:tv|com)|play\.su\.se)/(?:embed|media)/(?P<id>[^?#/]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "api.screen9.com/embed/8kTNEjvoXGM33dmWwF0uDA".to_owned(),
            r#"<script>
                var config = {
                    "src": [
                        {"type":"application/x-mpegURL","src":"https://cdn.example/screen9/master.m3u8"},
                        {"type":"video/mp4","src":"https://cdn.example/screen9/source.mp4"},
                        {"type":"video/webm","src":"https://cdn.example/screen9/source.webm"}
                    ],
                    "plugins": {
                        "title":{"title":"Östersjön i förändrat klimat","description":"Native Screen9 description"},
                        "share":{"mediaTitle":"Fallback title"}
                    },
                    "poster":"https://cdn.example/screen9/poster.jpg"
                };
            </script>"#
                .as_bytes()
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://api.screen9.com/embed/8kTNEjvoXGM33dmWwF0uDA",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("8kTNEjvoXGM33dmWwF0uDA"));
    assert_eq!(
        result.get_str("title"),
        Some("Östersjön i förändrat klimat")
    );
    assert_eq!(
        result.get_str("description"),
        Some("Native Screen9 description")
    );
    assert_eq!(result.get_str("ext"), Some("mp4"));
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(
        formats[0].get("protocol"),
        Some(&serde_json::json!("m3u8_native"))
    );
    assert_eq!(
        formats[1].get("format_id"),
        Some(&serde_json::json!("http-mp4"))
    );
}
