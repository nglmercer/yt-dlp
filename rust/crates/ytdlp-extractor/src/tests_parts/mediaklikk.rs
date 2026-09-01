#[test]
fn mediaklikk_native_extractor_maps_player_token_and_metadata() {
    let extractor = MediaKlikkExtractor::new(ExtractorDescriptor::new(
        "MediaKlikkIE",
        "MediaKlikk",
        r#"(?x)https?://(?:www\.)?(?:mediaklikk|m4sport|hirado)\.hu/.*?(?:videok?|cikk)/(?:(?P<year>[0-9]{4})/(?P<month>[0-9]{1,2})/(?P<day>[0-9]{1,2})/)?(?P<id>[^/#?_]+)"#,
        true,
    ))
    .unwrap();
    let page = br#"<html>
        <meta property="og:title" content="Fallback title">
        <meta property="og:image" content="https://cdn.example/mediaklikk/fallback.jpg">
        <script>
            loadPlayer({
                contentId: 8573769,
                title: 'Native MediaKlikk title',
                token: 'https%3A%2F%2Ftoken.example%2Fvalue',
                bgImage: 'https://cdn.example/mediaklikk/native.jpg',
            });
        </script>
    </html>"#
    .to_vec();
    let player = br#"<script>pl.setup({
        playlist: [{type: 'hls', file: 'https://cdn.example/mediaklikk/native.m3u8'}],
    });</script>"#
    .to_vec();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "player.mediaklikk.hu/playernew/player.php".to_owned(),
                player,
            ),
            ("mediaklikk.hu/video/2025/08/04/native-story".to_owned(), page),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://mediaklikk.hu/video/2025/08/04/native-story/",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("8573769"));
    assert_eq!(result.get_str("display_id"), Some("native-story"));
    assert_eq!(result.get_str("title"), Some("Native MediaKlikk title"));
    assert_eq!(result.get_str("upload_date"), Some("20250804"));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/mediaklikk/native.jpg")
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/mediaklikk/native.m3u8")
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("protocol")),
        Some(&serde_json::json!("m3u8_native"))
    );
}
