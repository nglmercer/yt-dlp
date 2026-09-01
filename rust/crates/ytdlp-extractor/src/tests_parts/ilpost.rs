#[test]
fn ilpost_native_extractor_posts_for_episode_metadata_and_maps_audio() {
    let extractor = IlPostExtractor::new(ExtractorDescriptor::new(
        "IlPostIE",
        "IlPost",
        r#"https?://(?:www\.)?ilpost\.it/episodes/(?P<id>[^/?#]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "www.ilpost.it/episodes/native-episode".to_owned(),
                br#"<script>
                    var ilpostpodcast = {
                        "post_id": "2972047",
                        "podcast_id": "235598",
                        "ajax_url": "https://api.ilpost.example/podcast",
                        "cookie": "native-cookie"
                    };
                </script>"#
                    .to_vec(),
            ),
            (
                "api.ilpost.example/podcast".to_owned(),
                br#"{"data":{"postcastList":[{
                    "id":2972047,
                    "title":"Native IlPost episode",
                    "description":"",
                    "podcast_raw_url":"https://cdn.ilpost.example/native.mp3",
                    "image":"https://cdn.ilpost.example/native.jpg",
                    "timestamp":1703835014,
                    "milliseconds":2495000,
                    "free":true
                }]}}"#
                .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.ilpost.it/episodes/native-episode/",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("2972047"));
    assert_eq!(result.get_str("display_id"), Some("native-episode"));
    assert_eq!(result.get_str("series_id"), Some("235598"));
    assert_eq!(result.get_str("title"), Some("Native IlPost episode"));
    assert_eq!(result.get_str("url"), Some("https://cdn.ilpost.example/native.mp3"));
    assert_eq!(result.get_str("vcodec"), Some("none"));
    assert_eq!(result.get("timestamp"), Some(&serde_json::json!(1703835014)));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(2495.0)));
    assert_eq!(result.get_str("availability"), Some("public"));
}
