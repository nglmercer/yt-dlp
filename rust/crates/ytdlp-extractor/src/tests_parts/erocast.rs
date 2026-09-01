#[test]
fn erocast_native_extractor_maps_player_data_and_track_metadata() {
    let extractor = ErocastExtractor::new(ExtractorDescriptor::new(
        "ErocastIE",
        "Erocast",
        r"https?://(?:www\.)?erocast\.me/track/(?P<id>[0-9]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<html><script>
            var song_data_9787 = {
                file_url: 'https://cdn.example/track.m3u8',
                title: '[F4M] Native track', description: 'Native description',
                created_at: '2023-10-01T12:44:12Z', updated_at: '2024-01-02T03:04:05Z',
                duration: 2307, plays: 42, comment_count: 7,
                artwork_url: 'https://cdn.example/art.jpg',
                permalink_url: 'https://erocast.me/track/9787/native',
                user: {name: 'native-user', id: 8113, permalink_url: 'https://erocast.me/native-user'}
            };
        </script></html>"#
        .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://erocast.me/track/9787/f", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("9787"));
    assert_eq!(result.get_str("title"), Some("[F4M] Native track"));
    assert_eq!(result.get_str("description"), Some("Native description"));
    assert_eq!(result.get_i64("release_timestamp"), Some(1_696_164_252));
    assert_eq!(result.get_i64("modified_timestamp"), Some(1_704_164_645));
    assert_eq!(result.get_str("uploader"), Some("native-user"));
    assert_eq!(result.get_str("uploader_id"), Some("8113"));
    assert_eq!(result.get_i64("duration"), Some(2307));
    assert_eq!(result.get_i64("view_count"), Some(42));
    assert_eq!(result.get_i64("comment_count"), Some(7));
    assert_eq!(result.get_str("ext"), Some("m4a"));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
}

#[test]
fn erocast_native_extractor_falls_back_to_stream_url() {
    let extractor = ErocastExtractor::new(ExtractorDescriptor::new(
        "ErocastIE",
        "Erocast",
        r"https?://(?:www\.)?erocast\.me/track/(?P<id>[0-9]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<script>var song_data_1={stream_url:'https://cdn.example/track.m4a',title:'Fallback'};</script>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://erocast.me/track/1", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("url"), Some("https://cdn.example/track.m4a"));
    assert_eq!(result.get_str("ext"), Some("m4a"));
}
