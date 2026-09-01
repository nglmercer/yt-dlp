#[test]
fn gopro_native_extractor_maps_reflect_metadata_and_variations() {
    let extractor = GoProExtractor::new(ExtractorDescriptor::new(
        "GoProIE",
        "GoPro",
        r#"https?://(www\.)?gopro\.com/v/(?P<id>[A-Za-z0-9]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "gopro.com/v/ZNVvED8QDzR5V".to_owned(),
                br#"<html><head>
                    <meta property="og:image" content="https://cdn.example/gopro.jpg">
                </head><body><script>
                    window.__reflectData = {
                        "collection":{"title":"My GoPro Adventure - 9/19/21",
                            "created_at":"2021-09-19T12:15:47Z"},
                        "account":{"nickname":"fireydive30018"},
                        "collectionMedia":[{
                            "id":"media-1","source_duration":396062,
                            "music_track_artist":"Artist","music_track_name":"Track"
                        }]
                    };
                </script></body></html>"#
                    .to_vec(),
            ),
            (
                "api.gopro.com/media/media-1/download".to_owned(),
                br#"{"_embedded":{"variations":[
                    {"url":"https://cdn.example/gopro-1080.mp4","quality":"1080p",
                     "label":"Full HD","type":"mp4","width":1920,"height":1080},
                    {"url":"https://cdn.example/gopro-720.mp4","quality":"720p",
                     "label":"HD","type":"mp4","width":1280,"height":720}
                ]}}"#
                .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://gopro.com/v/ZNVvED8QDzR5V", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("ZNVvED8QDzR5V"));
    assert_eq!(
        result.get_str("title"),
        Some("My GoPro Adventure - 9/19/21")
    );
    assert_eq!(result.get_str("uploader_id"), Some("fireydive30018"));
    assert_eq!(result.get_i64("duration"), Some(396_062));
    assert_eq!(result.get_str("artist"), Some("Artist"));
    assert_eq!(result.get_str("track"), Some("Track"));
    assert_eq!(result.get_i64("timestamp"), Some(1_632_053_747));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/gopro-1080.mp4")
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("height"))
            .and_then(serde_json::Value::as_i64),
        Some(1080)
    );
}
