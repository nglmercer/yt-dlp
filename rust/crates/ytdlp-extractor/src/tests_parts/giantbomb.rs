#[test]
fn giantbomb_native_extractor_maps_embedded_streams() {
    let extractor = GiantBombExtractor::new(ExtractorDescriptor::new(
        "GiantBombIE",
        "GiantBomb",
        r#"https?://(?:www\.)?giantbomb\.com/(?:videos|shows)/(?P<display_id>[^/]+)/(?P<id>\d+-\d+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "giantbomb.com/videos/quick-look-destiny".to_owned(),
            br#"<html><meta property="og:title" content="Quick Look: Destiny: The Dark Below"><meta property="og:description" content="Native Giant Bomb description"><meta property="og:image" content="https://cdn.example/giantbomb.jpg"><div data-video="{&quot;lengthSeconds&quot;:2399,&quot;videoStreams&quot;:{&quot;progressive_low&quot;:&quot;https://cdn.example/low.mp4&quot;,&quot;progressive_hd&quot;:&quot;https://cdn.example/hd.mp4&quot;,&quot;hls&quot;:&quot;https://cdn.example/master.m3u8&quot;,&quot;f4m_hd&quot;:&quot;https://cdn.example/legacy.f4m&quot;}}"></div></html>"#.to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.giantbomb.com/videos/quick-look-destiny/2300-9782/",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("2300-9782"));
    assert_eq!(
        result.get_str("display_id"),
        Some("quick-look-destiny")
    );
    assert_eq!(
        result.get_str("title"),
        Some("Quick Look: Destiny: The Dark Below")
    );
    assert_eq!(result.get_i64("duration"), Some(2399));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/giantbomb.jpg")
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(3)
    );
}

#[test]
fn giantbomb_native_extractor_redirects_youtube_fallback() {
    let extractor = GiantBombExtractor::new(ExtractorDescriptor::new(
        "GiantBombIE",
        "GiantBomb",
        r#"https?://(?:www\.)?giantbomb\.com/(?:videos|shows)/(?P<display_id>[^/]+)/(?P<id>\d+-\d+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "giantbomb.com/shows/fallback".to_owned(),
            br#"<meta property="og:title" content="Fallback"><div data-video="{&quot;videoStreams&quot;:{&quot;f4m_stream&quot;:&quot;https://cdn.example/legacy.f4m&quot;},&quot;youtubeID&quot;:&quot;native-youtube&quot;}"></div>"#.to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let ExtractorResult::Redirect { url, ie_key } = extractor
        .extract_with_context(
            "https://giantbomb.com/shows/fallback/2300-9782/",
            &context,
        )
        .unwrap()
    else {
        panic!("expected Youtube fallback redirect");
    };
    assert_eq!(ie_key.as_deref(), Some("Youtube"));
    assert_eq!(url, "https://www.youtube.com/watch?v=native-youtube");
}
