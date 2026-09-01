#[test]
fn canal1_native_extractor_returns_transparent_embedded_player() {
    let extractor = Canal1Extractor::new(ExtractorDescriptor::new(
        "Canal1IE",
        "Canal1",
        r"https?://(?:www\.|noticias\.)?canal1\.com\.co/(?:[^?#&])+/(?P<id>[\w-]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "canal1.com.co/noticias/native-story".to_owned(),
            br#"<script type="application/ld+json">{
                "embedUrl":"https:\/\/player.example\/native-video"
            }</script>"#
            .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://canal1.com.co/noticias/native-story",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("_type"), Some("url_transparent"));
    assert_eq!(result.get_str("display_id"), Some("native-story"));
    assert_eq!(result.get_str("url"), Some("https://player.example/native-video"));
}

#[test]
fn canal1_native_extractor_reports_missing_embed_url() {
    let extractor = Canal1Extractor::new(ExtractorDescriptor::new(
        "Canal1IE",
        "Canal1",
        r"https?://(?:www\.|noticias\.)?canal1\.com\.co/(?:[^?#&])+/(?P<id>[\w-]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "canal1.com.co/noticias/missing".to_owned(),
            br#"<html><title>missing</title></html>"#.to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context("https://canal1.com.co/noticias/missing", &context)
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Extraction);
    assert!(error.message.contains("embedded player URL"));
}
