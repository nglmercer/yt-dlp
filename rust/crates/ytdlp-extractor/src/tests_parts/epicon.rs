#[test]
fn epicon_native_extractor_maps_player_post_media_metadata_and_subtitles() {
    let extractor = EpiconExtractor::new(ExtractorDescriptor::new(
        "EpiconIE",
        "Epicon",
        r"https?://(?:www\.)?epicon\.in/(?:documentaries|movies|tv-shows/[^/?#]+/[^/?#]+)/(?P<id>[^/?#]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "ajaxplayer".to_owned(),
                br#"{"success":true,"url":{"video_url":"https://cdn.example/epicon.m3u8"},
                    "subtitles":[{"lang":"English","file":"//cdn.example/en.vtt"},{"lang":"default","file":"https://cdn.example/default.vtt"}]}"#
                    .to_vec(),
            ),
            (
                "epicon.in/movies/native".to_owned(),
                br#"<html><meta property="og:description" content="Native description">
                    <meta property="og:image" content="https://cdn.example/native.jpg">
                    <span class="mylist-icon iconclick" id="12345"></span>
                    <script>setplaytitle="Native Epicon";</script></html>"#
                    .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://www.epicon.in/movies/native", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("native"));
    assert_eq!(result.get_str("title"), Some("Native Epicon"));
    assert_eq!(result.get_str("description"), Some("Native description"));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/native.jpg")
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/epicon.m3u8")
    );
    assert_eq!(
        result
            .get("subtitles")
            .and_then(|value| value.get("English"))
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        result
            .get("subtitles")
            .and_then(|value| value.get("default"))
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
}

#[test]
fn epicon_series_native_extractor_builds_native_episode_entries() {
    let extractor = EpiconSeriesExtractor::new(ExtractorDescriptor::new(
        "EpiconSeriesIE",
        "EpiconSeries",
        r"(?!.*season)https?://(?:www\.)?epicon\.in/tv-shows/(?P<id>[^/?#]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<div ct-tray-url="tv-shows/native-show/season-1/episode-one"></div>
            <div ct-tray-url="tv-shows/native-show/season-1/episode-two"></div>
            <div ct-tray-url="tv-shows/native-show/season-1/episode-one"></div>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let ExtractorResult::Playlist { info, entries } = extractor
        .extract_with_context("https://www.epicon.in/tv-shows/native-show", &context)
        .unwrap()
    else {
        panic!("Epicon series should return a playlist");
    };

    assert_eq!(info.get_str("id"), Some("native-show"));
    assert_eq!(entries.len(), 2);
    assert_eq!(
        entries[0].get_str("url"),
        Some("https://www.epicon.in/tv-shows/native-show/season-1/episode-one")
    );
    assert_eq!(entries[0].get_str("ie_key"), Some("Epicon"));
}
