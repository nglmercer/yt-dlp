#[test]
fn markiza_native_video_extractor_maps_legacy_jwplayer_source() {
    let extractor = MarkizaExtractor::new(ExtractorDescriptor::new(
        "MarkizaIE",
        "Markiza",
        r#"https?://(?:www\.)?videoarchiv\.markiza\.sk/(?:video/(?:[^/]+/)*|embed/)(?P<id>\d+)(?:[_/]|$)"#,
        false,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "/json/video_jwplayer7.json".to_owned(),
            br#"{"details":{"name":"Oteckovia 109","duration":"00:46:00"},"playlist":[{"mediaid":"139078","title":"Oteckovia 109","image":"https://cdn.example/markiza.jpg","sources":[{"file":"https://cdn.example/markiza.mp4","label":"720p","width":1280,"height":720}]}]}"#.to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://videoarchiv.markiza.sk/video/oteckovia/139078_",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("139078"));
    assert_eq!(result.get_str("title"), Some("Oteckovia 109"));
    assert_eq!(result.get_i64("duration"), Some(2760));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/markiza.jpg")
    );
    assert_eq!(result.get_str("url"), Some("https://cdn.example/markiza.mp4"));
    assert_eq!(result.get_str("ext"), Some("mp4"));
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
fn markiza_native_video_extractor_materializes_multi_item_playlist() {
    let extractor = MarkizaExtractor::new(ExtractorDescriptor::new(
        "MarkizaIE",
        "Markiza",
        r#"https?://videoarchiv\.markiza\.sk/video/(?P<id>\d+)(?:[_/]|$)"#,
        false,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "/json/video_jwplayer7.json".to_owned(),
            r#"{"details":{"name":"Televízne noviny"},"playlist":[{"mediaid":"1","title":"One","sources":[{"file":"https://cdn.example/one.mp4"}]},{"mediaid":"2","title":"Two","sources":[{"file":"https://cdn.example/two.mp4"}]}]}"#.as_bytes().to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let ExtractorResult::Playlist { info, entries } = extractor
        .extract_with_context("https://videoarchiv.markiza.sk/video/85430", &context)
        .unwrap()
    else {
        panic!("Markiza multi-item payload should return a playlist");
    };

    assert_eq!(info.get_str("id"), Some("85430"));
    assert_eq!(info.get_str("title"), Some("Televízne noviny"));
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get_str("id"), Some("1"));
    assert_eq!(entries[1].get_str("url"), Some("https://cdn.example/two.mp4"));
}

#[test]
fn markiza_page_native_extractor_deduplicates_embedded_video_ids() {
    let extractor = MarkizaPageExtractor::new(ExtractorDescriptor::new(
        "MarkizaPageIE",
        "MarkizaPage",
        r#"https?://(?:www\.)?(?:(?:[^/]+\.)?markiza|tvnoviny)\.sk/(?:[^/]+/)*(?P<id>\d+)_"#,
        false,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<script>initPlayer_12(); data-entity="34"; id="player_12"; data-entity='56'</script>"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let ExtractorResult::Playlist { info, entries } = extractor
        .extract_with_context("https://www.markiza.sk/clanok/12345_noviny", &context)
        .unwrap()
    else {
        panic!("Markiza page should return a playlist");
    };

    assert_eq!(info.get_str("id"), Some("12345"));
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].get_str("_type"), Some("url"));
    assert_eq!(entries[0].get_str("id"), Some("12"));
    assert_eq!(
        entries[1].get_str("url"),
        Some("http://videoarchiv.markiza.sk/video/34")
    );
}
