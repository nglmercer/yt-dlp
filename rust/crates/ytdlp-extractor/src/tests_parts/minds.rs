#[test]
fn minds_native_video_extractor_maps_entity_and_media_api_data() {
    let extractor = MindsExtractor::new(ExtractorDescriptor::new(
        "MindsIE",
        "minds",
        r#"https?://(?:www\.)?minds\.com/(?:media|newsfeed|archive/view)/(?P<id>[0-9]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "/api/v1/entities/entity/100000000000086822".to_owned(),
                br#"{"entity":{"type":"video","subtype":"video","title":"Minds intro sequence","description":"<p>Native <strong>description</strong></p>","license":"attribution-cc","time_created":1369404826,"ownerObj":{"username":"ottman","name":"Bill Ottman"},"play:count":11,"thumbs:up:count":7,"thumbs:down:count":2,"comments:count":3,"tags":"animation","thumbnail_src":"https://cdn.example/poster.png"}}"#.to_vec(),
            ),
            (
                "/api/v2/media/video/100000000000086822".to_owned(),
                br#"{"sources":[{"src":"https://cdn.example/minds.mp4","label":"720p","size":"720"}],"entity":{"type":"video","subtype":"video","title":"Minds intro sequence","description":"<p>Native <strong>description</strong></p>","license":"attribution-cc","time_created":1369404826,"ownerObj":{"username":"ottman","name":"Bill Ottman"},"play:count":11,"thumbs:up:count":7,"thumbs:down:count":2,"comments:count":3,"tags":"animation"}}"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.minds.com/media/100000000000086822",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("100000000000086822"));
    assert_eq!(result.get_str("title"), Some("Minds intro sequence"));
    assert_eq!(
        result.get_str("description"),
        Some("Native description")
    );
    assert_eq!(result.get_str("license"), Some("attribution-cc"));
    assert_eq!(result.get_i64("timestamp"), Some(1369404826));
    assert_eq!(result.get_str("uploader"), Some("Bill Ottman"));
    assert_eq!(result.get_str("uploader_id"), Some("ottman"));
    assert_eq!(
        result.get_str("uploader_url"),
        Some("https://www.minds.com/ottman")
    );
    assert_eq!(result.get_i64("view_count"), Some(11));
    assert_eq!(result.get_i64("like_count"), Some(7));
    assert_eq!(result.get_i64("dislike_count"), Some(2));
    assert_eq!(result.get_i64("comment_count"), Some(3));
    assert_eq!(result.get("tags"), Some(&serde_json::json!(["animation"])));
    assert_eq!(result.get_str("ext"), Some("mp4"));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/minds.mp4")
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("height")),
        Some(&serde_json::json!(720))
    );
}

#[test]
fn minds_native_activity_redirects_non_video_entities() {
    let extractor = MindsExtractor::new(ExtractorDescriptor::new(
        "MindsIE",
        "minds",
        r#"https?://(?:www\.)?minds\.com/(?:media|newsfeed|archive/view)/(?P<id>[0-9]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{"entity":{"type":"activity","custom_type":"image","perma_url":"https://www.minds.com/newsfeed/other"}}"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    assert_eq!(
        extractor
            .extract_with_context(
                "https://www.minds.com/newsfeed/798025111988506624",
                &context,
            )
            .unwrap(),
        ExtractorResult::Redirect {
            url: "https://www.minds.com/newsfeed/other".to_owned(),
            ie_key: None,
        }
    );
}

#[test]
fn minds_native_channel_playlist_maps_feed_entries() {
    let extractor = MindsChannelExtractor::new(ExtractorDescriptor::new(
        "MindsChannelIE",
        "minds:channel",
        r#"https?://(?:www\.)?minds\.com/(?!(?:newsfeed|media|api|archive|groups)/)(?P<id>[^/?&#]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "/api/v1/channel/ottman".to_owned(),
                br#"{"channel":{"guid":"channel-guid","name":"Bill Ottman","briefdescription":"Native channel description"}}"#.to_vec(),
            ),
            (
                "/api/v2/feeds/container/channel-guid/videos".to_owned(),
                br#"{"entities":[{"guid":"100"},{"guid":"200"}],"load-next":null}"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://www.minds.com/ottman", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("ottman"));
    assert_eq!(result.get_str("title"), Some("Bill Ottman"));
    assert_eq!(
        result.get_str("description"),
        Some("Native channel description")
    );
    let entries = result
        .get("entries")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get("ie_key"), Some(&serde_json::json!("Minds")));
    assert_eq!(
        entries[1].get("url"),
        Some(&serde_json::json!("https://www.minds.com/newsfeed/200"))
    );
}
