#[test]
fn hrfernsehen_native_extractor_maps_loader_streams_and_subtitles() {
    let extractor = HrFernsehenExtractor::new(ExtractorDescriptor::new(
        "HRFernsehenIE",
        "hrfernsehen",
        r#"https?://www\.(?:hr-fernsehen|hessenschau)\.de/.*,video-(?P<id>[0-9]{6})\.html"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<html><head>
            <meta property="og:title" content="Native Hessenschau">
            <meta name="description" content="Native description">
            <meta property="og:image" content="https://cdn.example/fallback.jpg">
        </head><body>
            <time datetime="2020-08-26"></time>
            <div thumbnailUrl":"https://cdn.example/native-thumbnail.jpg"></div>
            <div data-hr-mediaplayer-loader='{
                "mediaCollection":{
                    "streams":[{"media":[
                        {"url":"https://cdn.example/auto.mp4"},
                        {"url":"https://cdn.example/512x288-25p-500kbit.mp4","maxHResolutionPx":288},
                        {"url":"https://cdn.example/1280x720-25p-1500kbit.mp4","maxHResolutionPx":720}
                    ]}],
                    "subTitles":[{"sources":[{"url":"https://cdn.example/native.vtt"}]}]
                },
                "playerConfig":{"pluginData":{"trackingAti@all":{"richMedia":{"duration":1654}}}}
            }'></div>
        </body></html>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.hessenschau.de/tv-sendung/native,video-130546.html",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("130546"));
    assert_eq!(result.get_str("title"), Some("Native Hessenschau"));
    assert_eq!(
        result.get_str("description"),
        Some("Native description")
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/512x288-25p-500kbit.mp4")
    );
    assert_eq!(result.get_str("thumbnail"), Some("https://cdn.example/native-thumbnail.jpg"));
    assert_eq!(result.get("timestamp"), Some(&serde_json::json!(1598400000)));
    assert_eq!(result.get("upload_date"), Some(&serde_json::json!("20200826")));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(1654)));
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
            .and_then(|format| format.get("width")),
        Some(&serde_json::json!(512))
    );
    assert_eq!(
        result
            .get("subtitles")
            .and_then(|subtitles| subtitles.get("de"))
            .and_then(serde_json::Value::as_array)
            .and_then(|entries| entries.first())
            .and_then(|entry| entry.get("url")),
        Some(&serde_json::json!("https://cdn.example/native.vtt"))
    );
}
