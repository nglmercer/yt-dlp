#[test]
fn france_culture_native_extractor_maps_jsonld_audio_episode() {
    let extractor = FranceCultureExtractor::new(ExtractorDescriptor::new(
        "FranceCultureIE",
        "FranceCulture",
        r#"(?x)
            https?://(?:www\.)?radiofrance\.fr
            /(?:franceculture|franceinfo|franceinter|francemusique|fip|mouv)
            /podcasts/(?:[^?#]+/)?(?P<display_id>[^?#]+)-(?P<id>\d{6,})(?:$|[?#])
        "#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: r#"<html><head>
            <meta property="og:title" content="Fallback title">
            <meta name="description" content="Podcast description">
            <meta property="og:image" content="https://cdn.example/radio.jpg">
            <script type="application/ld+json">{
                "@context":"https://schema.org",
                "@graph":[
                    {"@type":"WebPage","name":"Page"},
                    {"@type":"NewsArticle","datePublished":"2022-05-14T14:00:00.000Z"},
                    {"@type":"RadioEpisode","mainEntity":{
                        "@type":"AudioObject",
                        "contentUrl":"https://media.example/episode.mp3",
                        "duration":"P0Y0M0DT0H45M50S",
                        "encodingFormat":"audio/mpeg"
                    }}
                ]
            }</script>
        </head><body>
            <h1 itemprop="name">Native podcast title</h1>
            <span class="author">Native presenter</span>
        </body></html>"#
            .as_bytes()
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.radiofrance.fr/franceculture/podcasts/science/native-podcast-title-8440487",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("8440487"));
    assert_eq!(
        result.get_str("display_id"),
        Some("native-podcast-title")
    );
    assert_eq!(result.get_str("title"), Some("Native podcast title"));
    assert_eq!(result.get_str("description"), Some("Podcast description"));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/radio.jpg")
    );
    assert_eq!(result.get_str("uploader"), Some("Native presenter"));
    assert_eq!(result.get_f64("duration"), Some(2750.0));
    assert_eq!(result.get_i64("timestamp"), Some(1652536800));
    assert_eq!(result.get_str("upload_date"), Some("20220514"));
    assert_eq!(
        result.get_str("url"),
        Some("https://media.example/episode.mp3")
    );
    assert_eq!(result.get_str("ext"), Some("mp3"));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("vcodec")),
        Some(&serde_json::json!("none"))
    );
}

#[test]
fn france_culture_native_extractor_requires_audio_object() {
    let extractor = FranceCultureExtractor::new(ExtractorDescriptor::new(
        "FranceCultureIE",
        "FranceCulture",
        r#"https?://(?:www\.)?radiofrance\.fr/franceculture/podcasts/[^/]+/(?P<display_id>[^?#]+)-(?P<id>\d{6,})"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<script type="application/ld+json">{"@type":"RadioEpisode","name":"No audio"}</script>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context(
            "https://www.radiofrance.fr/franceculture/podcasts/science/no-audio-8440487",
            &context,
        )
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Extraction);
    assert!(error.message.contains("has no audio data"));
}
