#[test]
fn ixigua_native_extractor_decodes_ssr_media_and_maps_metadata() {
    let extractor = IxiguaExtractor::new(ExtractorDescriptor::new(
        "IxiguaIE",
        "Ixigua",
        r#"https?://(?:\w+\.)?ixigua\.com/(?:video/)?(?P<id>\d+).+"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "ixigua.com/6996881461559165471".to_owned(),
            br#"<script id="SSR_HYDRATED_DATA">window._SSR_HYDRATED_DATA={
                "anyVideo":{"gidInformation":{"packerData":{"video":{
                    "videoResource":{
                        "video_list":[
                            {"main_url":"aHR0cHM6Ly9jZG4uZXhhbXBsZS9oaWdoLm1wNA==","vwidth":1920,"vheight":1080,"fps":30,"size":1000,"codec_type":"h264","quality_type":"1080p"}
                        ],
                        "dynamic_video":{
                            "dynamic_video_list":[
                                {"main_url":"aHR0cHM6Ly9jZG4uZXhhbXBsZS9sb3cubXA0","vwidth":640,"vheight":360,"quality_type":"360p"}
                            ],
                            "dynamic_audio_list":[
                                {"main_url":"aHR0cHM6Ly9jZG4uZXhhbXBsZS9hdWRpby5tNGE=","quality_type":"audio"}
                            ]
                        }
                    },
                    "title":"Native Ixigua title",
                    "video_abstract":"Native Ixigua description",
                    "video_like_count":11,
                    "video_unlike_count":2,
                    "video_watch_count":99,
                    "duration":1030,
                    "video_publish_time":1629088414,
                    "tag":"video_car",
                    "cover_url":"https://cdn.example/native.webp",
                    "user_info":{"user_id":6480145787,"name":"Native uploader"}
                }}}}
            };</script>"#
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.ixigua.com/6996881461559165471",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("6996881461559165471"));
    assert_eq!(result.get_str("title"), Some("Native Ixigua title"));
    assert_eq!(
        result.get_str("description"),
        Some("Native Ixigua description")
    );
    assert_eq!(result.get_str("thumbnail"), Some("https://cdn.example/native.webp"));
    assert_eq!(result.get("view_count"), Some(&serde_json::json!(99)));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(1030)));
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 3);
    assert_eq!(formats[0].get("width"), Some(&serde_json::json!(1920)));
    assert_eq!(formats[2].get("vcodec"), Some(&serde_json::json!("none")));
    assert_eq!(
        formats[2].get("ext"),
        Some(&serde_json::json!("m4a"))
    );
}
