#[test]
fn footyroom_native_extractor_maps_streamable_playlist_entries() {
    let extractor = FootyRoomExtractor::new(ExtractorDescriptor::new(
        "FootyRoomIE",
        "FootyRoom",
        r"https?://footyroom\.com/matches/(?P<id>\d+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "footyroom.com/matches/79922154".to_owned(),
            br#"<html><head><meta property="og:title" content="VIDEO Native Match"></head>
                <script>DataStore.media = [
                    {"payload":"<iframe src=\"https://streamable.com/native-home\"></iframe>"},
                    {"payload":"<iframe src=\"//streamable.com/native-away\"></iframe>"},
                    {"payload":"{\"ignored\":true}"}
                ];</script></html>"#
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let extraction = extractor
        .extract_with_context(
            "http://footyroom.com/matches/79922154/hull-city-vs-chelsea/review",
            &context,
        )
        .unwrap();
    let ExtractorResult::Playlist { info, entries } = extraction else {
        panic!("expected FootyRoom playlist result");
    };

    assert_eq!(info.get_str("id"), Some("79922154"));
    assert_eq!(info.get_str("title"), Some("VIDEO Native Match"));
    assert_eq!(entries.len(), 2);
    assert_eq!(
        entries[0].get_str("url"),
        Some("https://streamable.com/native-home")
    );
    assert_eq!(
        entries[1].get_str("url"),
        Some("https://streamable.com/native-away")
    );
    assert_eq!(entries[0].get_str("ie_key"), Some("Streamable"));
}
