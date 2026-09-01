#[test]
fn francais_facile_native_extractor_maps_embedded_audio_and_jsonld() {
    let extractor = FrancaisFacileExtractor::new(ExtractorDescriptor::new(
        "FrancaisFacileIE",
        "FrancaisFacile",
        r"https?://francaisfacile\.rfi\.fr/[a-z]{2}/(?:actualit%C3%A9|podcasts/[^/#?]+)/(?P<id>[^/#?]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: r#"<html><head>
            <title>Réconcilier les jeunes avec la lecture</title>
            <meta name="description" content="Page description">
            <script type="application/ld+json">{
                "@type":"AudioObject",
                "description":"JSON-LD description",
                "datePublished":"2025-03-05T10:00:00Z"
            }</script>
        </head><body>
            <script data-media-id="native-media" type="application/json">{
                "mediaId":"WBMZ58952-FLE-FR-20250305",
                "title":"Réconcilier les jeunes avec la lecture grâce aux réseaux sociaux",
                "sources":[{"url":"https://aod-fle.example/native.mp3","duration":103.15}]
            }</script>
        </body></html>"#
            .as_bytes()
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://francaisfacile.rfi.fr/fr/actualit%C3%A9/20250305-r%C3%A9concilier-les-jeunes",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("WBMZ58952-FLE-FR-20250305"));
    assert_eq!(
        result.get_str("display_id"),
        Some("20250305-réconcilier-les-jeunes")
    );
    assert_eq!(
        result.get_str("title"),
        Some("Réconcilier les jeunes avec la lecture grâce aux réseaux sociaux")
    );
    assert_eq!(
        result.get_str("description"),
        Some("JSON-LD description")
    );
    assert_eq!(result.get_f64("duration"), Some(103.15));
    assert_eq!(result.get_i64("timestamp"), Some(1741168800));
    assert_eq!(result.get_str("upload_date"), Some("20250305"));
    assert_eq!(result.get_str("url"), Some("https://aod-fle.example/native.mp3"));
    assert_eq!(result.get_str("ext"), Some("mp3"));
    assert_eq!(result.get_str("vcodec"), Some("none"));
}

#[test]
fn francais_facile_native_extractor_requires_audio_source() {
    let extractor = FrancaisFacileExtractor::new(ExtractorDescriptor::new(
        "FrancaisFacileIE",
        "FrancaisFacile",
        r"https?://francaisfacile\.rfi\.fr/[a-z]{2}/(?:actualit%C3%A9|podcasts/[^/#?]+)/(?P<id>[^/#?]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<script data-media-id="native-media" type="application/json">{"mediaId":"native","sources":[]}</script>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context(
            "https://francaisfacile.rfi.fr/fr/actualit%C3%A9/native",
            &context,
        )
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Extraction);
    assert!(error.message.contains("has no source URL"));
}
