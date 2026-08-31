#[test]
fn bild_native_extractor_maps_json_hls_and_mp4_sources() {
    let extractor = BildExtractor::new(ExtractorDescriptor::new(
        "BildIE",
        "Bild",
        r"https?://(?:www\.)?bild\.de/(?:[^/]+/)+(?P<display_id>[^/]+)-(?P<id>\d+)(?:,auto=true)?\.bild\.html",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "www.bild.de/video/clip/news/deftiger-abgang-85158620,view=json.bild.html".to_owned(),
            r#"{
                "title":"Der Sprungturm-Skandal &amp; mehr",
                "description":"A native &amp; description",
                "poster":"https://img.example/bild/85158620.jpg",
                "durationSec":69,
                "clipList":[{
                    "srces":[
                        {"type":"application/x-mpegURL","src":"https://cdn.example/bild/master.m3u8"},
                        {"type":"video/mp4","src":"https://cdn.example/bild/source.mp4"},
                        {"type":"video/webm","src":"https://cdn.example/bild/source.webm"}
                    ]
                }]
            }"#
            .as_bytes()
            .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.bild.de/video/clip/news/deftiger-abgang-85158620.bild.html",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("85158620"));
    assert_eq!(result.get_str("title"), Some("Der Sprungturm-Skandal & mehr"));
    assert_eq!(
        result.get_str("description"),
        Some("A native & description")
    );
    assert_eq!(result.get_i64("duration"), Some(69));
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
