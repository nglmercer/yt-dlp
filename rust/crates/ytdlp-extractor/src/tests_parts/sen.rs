#[test]
fn sen_native_extractor_maps_api_metadata_and_hls() {
    let extractor = SenExtractor::new(ExtractorDescriptor::new(
        "SenIE",
        "Sen",
        r"https?://(?:www\.)?sen\.com/video/(?P<id>[0-9a-f-]+)",
        true,
    ))
    .unwrap();
    let video_id = "eef46eb1-4d79-4e28-be9d-bd937767f8c4";
    let api_url = format!("api.sen.com/content/public/video/{video_id}");
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                api_url,
                br#"{
                    "data": {
                        "nodes": [
                            {"id":"player","video":{"url":"https://vod.example/sen/master.m3u8"}},
                            {"id":"details","content":{
                                "title":{"text":"Hurricane Ian"},
                                "descriptions":[{"text":"Florida, 28 Sep 2022"}],
                                "badges":[{"text":"North America"},{"text":"Storm"},{"text":"Weather"}]
                            }}
                        ]
                    }
                }"#
                .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            &format!("https://www.sen.com/video/{video_id}"),
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some(video_id));
    assert_eq!(result.get_str("title"), Some("Hurricane Ian"));
    assert_eq!(
        result.get_str("description"),
        Some("Florida, 28 Sep 2022")
    );
    assert_eq!(
        result.get("tags"),
        Some(&serde_json::json!(["North America", "Storm", "Weather"]))
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://vod.example/sen/master.m3u8")
    );
    assert_eq!(result.get_str("ext"), Some("mp4"));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("protocol"))
            .and_then(serde_json::Value::as_str),
        Some("m3u8_native")
    );
}

#[test]
fn sen_native_extractor_uses_manifest_fallback() {
    let extractor = SenExtractor::new(ExtractorDescriptor::new(
        "SenIE",
        "Sen",
        r"https?://(?:www\.)?sen\.com/video/(?P<id>[0-9a-f-]+)",
        true,
    ))
    .unwrap();
    let video_id = "abcdef01-2345-6789-abcd-ef0123456789";
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                format!("api.sen.com/content/public/video/{video_id}"),
                br#"{"data":{"nodes":[{"id":"player","video":{"url":null}},{"id":"details","content":{}}]}}"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            &format!("https://sen.com/video/{video_id}"),
            &context,
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(
        result.get_str("url"),
        Some("https://vod.sen.com/videos/abcdef01-2345-6789-abcd-ef0123456789/manifest.m3u8")
    );
}
