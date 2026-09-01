#[test]
fn huya_video_native_extractor_maps_api_hls_definitions() {
    let extractor = HuyaVideoExtractor::new(ExtractorDescriptor::new(
        "HuyaVideoIE",
        "huya:video",
        r#"https?://(?:www\.)?huya\.com/video/play/(?P<id>\d+)\.html"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "liveapi.huya.com/moment/getMomentContent".to_owned(),
            br#"{"data":{"moment":{
                "content":"<p>Native Huya description</p>",
                "commentCount":7,
                "favorCount":8,
                "cTime":1722675950,
                "videoInfo":{
                    "videoTitle":"Native Huya VOD",
                    "category":["Gaming"],
                    "tags":["native","rust"],
                    "videoDuration":"00:14",
                    "nickName":"Native Huya",
                    "uid":1564376151,
                    "videoPlayNum":42,
                    "videoBigCover":"https://cdn.example/huya.jpg?width=640",
                    "definitions":{
                        "720":{"m3u8":"https://cdn.example/huya/720.m3u8","defName":"High","size":1000,"height":720,"width":1280,"definition":3},
                        "480":{"m3u8":"https://cdn.example/huya/480.m3u8","defName":"Low","size":500,"height":480,"width":854,"definition":1}
                    }
                }
            }}}"#
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.huya.com/video/play/1002412640.html",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("1002412640"));
    assert_eq!(result.get_str("title"), Some("Native Huya VOD"));
    assert_eq!(
        result.get_str("description"),
        Some("Native Huya description")
    );
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/huya.jpg")
    );
    assert_eq!(result.get("duration"), Some(&serde_json::json!(14.0)));
    assert_eq!(result.get("view_count"), Some(&serde_json::json!(42)));
    assert_eq!(result.get("comment_count"), Some(&serde_json::json!(7)));
    assert_eq!(result.get("like_count"), Some(&serde_json::json!(8)));
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
            .and_then(|format| format.get("protocol")),
        Some(&serde_json::json!("m3u8_native"))
    );
}
