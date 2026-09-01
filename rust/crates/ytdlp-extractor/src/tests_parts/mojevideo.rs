#[test]
fn mojevideo_native_extractor_builds_signed_quality_formats_and_json_ld_metadata() {
    let extractor = MojevideoExtractor::new(ExtractorDescriptor::new(
        "MojevideoIE",
        "mojevideo.sk",
        r#"https?://(?:www\.)?mojevideo\.sk/video/(?P<id>\w+)/(?P<display_id>[\w()]+?)\.html"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<html>
            <meta property="og:title" content="Fallback title">
            <meta property="og:description" content="Fallback description">
            <meta property="og:image" content="https://cdn.example/fallback.jpg">
            <title>Fallback title - Mojevideo</title>
            <script type="application/ld+json">{"@type":"VideoObject","name":"Native Mojevideo","description":"JSON-LD description","thumbnailUrl":"https://cdn.example/mojevideo.jpg","duration":"PT21S","uploadDate":"2023-09-19T12:01:46Z","interactionStatistic":[{"interactionType":"https://schema.org/LikeAction","userInteractionCount":7},{"interactionType":"https://schema.org/WatchAction","userInteractionCount":42},{"interactionType":"https://schema.org/CommentAction","userInteractionCount":3}]}</script>
            <script>var vId = 250236; var vEx = '1700000000'; var vHash = ['hash-normal', 'hash-low', 'hash-hd'];</script>
        </html>"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.mojevideo.sk/video/3d17c/chlapci_dobetonovali_sme_mame_hotovo.html",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("3d17c"));
    assert_eq!(result.get_str("display_id"), Some("chlapci_dobetonovali_sme_mame_hotovo"));
    assert_eq!(result.get_str("title"), Some("Native Mojevideo"));
    assert_eq!(result.get_str("description"), Some("JSON-LD description"));
    assert_eq!(result.get_str("thumbnail"), Some("https://cdn.example/mojevideo.jpg"));
    assert_eq!(result.get_f64("duration"), Some(21.0));
    assert_eq!(result.get_str("upload_date"), Some("20230919"));
    assert_eq!(result.get_i64("timestamp"), Some(1695124906));
    assert_eq!(result.get_i64("view_count"), Some(42));
    assert_eq!(result.get_i64("like_count"), Some(7));
    assert_eq!(result.get_i64("comment_count"), Some(3));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(3)
    );
    assert!(result
        .get_str("url")
        .is_some_and(|url| url.contains("md5=hash-normal") && url.contains("expires=1700000000")));
}
