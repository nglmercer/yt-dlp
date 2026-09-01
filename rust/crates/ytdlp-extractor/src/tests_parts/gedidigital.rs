#[test]
fn gedidigital_native_extractor_maps_player_parameters() {
    let extractor = GediDigitalExtractor::new(ExtractorDescriptor::new(
        "GediDigitalIE",
        "GediDigital",
        r#"(?P<base_url>https?://video\.lastampa\.it/[^/]+/[^/]+/[^/]+/(?P<id>\d+))(?:$|[?&].*)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "video.lastampa.it/politica/native/gedi/121683".to_owned(),
            br#"<html><meta property="twitter:title" content="Native Gedi video"><meta property="og:description" content="Native Gedi description"><meta property="og:image" content="https://cdn.example/fallback.jpg"><script>PlayerFactory.setParam('format', 'video-rrtv-720-1500', 'https://cdn.example/video-720.mp4');PlayerFactory.setParam('format', 'audio-mp3', 'https://cdn.example/audio-mp3-audio-128.mp3');PlayerFactory.setParam('format', 'hls', 'https://cdn.example/master.m3u8');PlayerFactory.setParam('format', 'video-rrtv-720-1500', 'https://cdn.example/video-720.mp4');PlayerFactory.setParam('param', 'image_full', 'https://cdn.example/hero.jpg');PlayerFactory.setParam('param', 'videoDuration', '125');</script></html>"#.to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://video.lastampa.it/politica/native/gedi/121683?responsive=true",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("121683"));
    assert_eq!(result.get_str("title"), Some("Native Gedi video"));
    assert_eq!(result.get_str("thumbnail"), Some("https://cdn.example/hero.jpg"));
    assert_eq!(result.get_i64("duration"), Some(125));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(3)
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.get(1))
            .and_then(|format| format.get("abr"))
            .and_then(serde_json::Value::as_i64),
        Some(128)
    );
}
