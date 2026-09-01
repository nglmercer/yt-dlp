#[test]
fn gamestar_native_extractor_maps_video_object_and_media_url() {
    let extractor = GameStarExtractor::new(ExtractorDescriptor::new(
        "GameStarIE",
        "GameStar",
        r#"https?://(?:www\.)?game(?P<site>pro|star)\.de/videos/.*,(?P<id>[0-9]+)\.html"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<html><head>
            <script type="application/ld+json">
                {"@context":"https://schema.org","@type":"WebSite","name":"GameStar"}
            </script>
            <script type="application/ld+json">
                {"@context":"https://schema.org","@type":"VideoObject",
                 "name":"Native GameStar title - GameStar",
                 "description":"Native GameStar description",
                 "thumbnailUrl":["https://cdn.example/gamestar.jpg"],
                 "uploadDate":"2014-07-28T10:13:00Z",
                 "duration":"PT17S","interactionCount":"123"}
            </script>
            <meta name="description" content="HTML description">
        </head><body>
            <span>Kommentare</span><span class="count">(42)</span>
        </body></html>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.gamestar.de/videos/trailer,3/native-title,76110.html",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("76110"));
    assert_eq!(result.get_str("title"), Some("Native GameStar title"));
    assert_eq!(
        result.get_str("description"),
        Some("Native GameStar description")
    );
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/gamestar.jpg")
    );
    assert_eq!(result.get_i64("timestamp"), Some(1_406_542_380));
    assert_eq!(result.get_f64("duration"), Some(17.0));
    assert_eq!(result.get_i64("view_count"), Some(123));
    assert_eq!(result.get_i64("comment_count"), Some(42));
    assert_eq!(
        result.get_str("url"),
        Some("http://gamestar.de/_misc/videos/portal/getVideoUrl.cfm?premium=0&videoId=76110")
    );
}
