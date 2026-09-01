#[test]
fn hypergryph_native_extractor_maps_song_and_album_metadata() {
    let extractor = MonsterSirenHypergryphMusicExtractor::new(ExtractorDescriptor::new(
        "MonsterSirenHypergryphMusicIE",
        "monstersiren",
        r#"https?://monster-siren\.hypergryph\.com/music/(?P<id>\d+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "monster-siren.hypergryph.com/api/song/514562".to_owned(),
                br#"{"code":0,"data":{"name":"Native Flame Shadow","artists":["Native Artist"],"sourceUrl":"https://cdn.example/flame.wav","lyricUrl":"https://cdn.example/flame.lrc","albumCid":"album-native"}}"#.to_vec(),
            ),
            (
                "monster-siren.hypergryph.com/api/album/album-native/detail".to_owned(),
                br#"{"code":0,"data":{"name":"Native Album","intro":"<p>Native album description</p>","coverUrl":"https://cdn.example/cover.jpg"}}"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://monster-siren.hypergryph.com/music/514562",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("514562"));
    assert_eq!(result.get_str("title"), Some("Native Flame Shadow"));
    assert_eq!(result.get_str("album"), Some("Native Album"));
    assert_eq!(result.get_str("description"), Some("Native album description"));
    assert_eq!(result.get_str("thumbnail"), Some("https://cdn.example/cover.jpg"));
    assert_eq!(result.get_str("ext"), Some("wav"));
    assert_eq!(
        result
            .get("subtitles")
            .and_then(|value| value.get("en"))
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
}
