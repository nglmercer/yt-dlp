#[test]
fn berufetv_native_extractor_maps_metadata_sources_and_subtitles() {
    let extractor = BerufeTvExtractor::new(ExtractorDescriptor::new(
        "BerufeTVIE",
        "BerufeTV",
        r"https?://(?:www\.)?web\.arbeitsagentur\.de/berufetv/[^?#]+/film;filmId=(?P<id>[\w-]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "rest.arbeitsagentur.de/infosysbub/berufetv/pc/v1/film-metadata".to_owned(),
                br#"{"metadaten":[{
                    "miId":"native-film",
                    "titel":"Native film title",
                    "beschreibung":"Native film description",
                    "thumbnail":"https://cdn.example/film/poster.jpg",
                    "kategorie":"Study",
                    "themengebiete":["Economics"]
                }]}"#
                .to_vec(),
            ),
            (
                "d.video-cdn.net/play/player/8YRzUk6pTzmBdrsLe9Y88W/video/native-film".to_owned(),
                br#"{
                    "videoSources":{"html":{
                        "auto":[{"source":"https://cdn.example/film/master.m3u8","mimeType":"application/vnd.apple.mpegurl"}],
                        "720p":[{"source":"https://cdn.example/film/720.mp4","mimeType":"video/mp4"}]
                    }},
                    "videoMetaData":{"title":"Fallback title"},
                    "duration":602440,
                    "videoTracks":[{"type":"SUBTITLES","language":"de","source":"https://cdn.example/film/de.vtt","label":"Deutsch"}]
                }"#
                .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://web.arbeitsagentur.de/berufetv/study/film;filmId=native-film",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("native-film"));
    assert_eq!(result.get_str("title"), Some("Native film title"));
    assert_eq!(result.get_str("description"), Some("Native film description"));
    assert_eq!(result.get_i64("duration"), None);
    assert_eq!(result.get_f64("duration"), Some(602.44));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/film/master.m3u8"));
    assert_eq!(result.get("categories"), Some(&serde_json::json!(["Study"])));
    assert_eq!(result.get("tags"), Some(&serde_json::json!(["Economics"])));
    assert_eq!(
        result
            .get("subtitles")
            .and_then(|value| value.get("de"))
            .and_then(serde_json::Value::as_array)
            .and_then(|values| values.first())
            .and_then(|value| value.get("ext")),
        Some(&serde_json::json!("vtt"))
    );
}

#[test]
fn berufetv_native_extractor_marks_unknown_sources_as_todo() {
    let extractor = BerufeTvExtractor::new(ExtractorDescriptor::new(
        "BerufeTVIE",
        "BerufeTV",
        r"https?://(?:www\.)?web\.arbeitsagentur\.de/berufetv/[^?#]+/film;filmId=(?P<id>[\w-]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "rest.arbeitsagentur.de/infosysbub/berufetv/pc/v1/film-metadata".to_owned(),
                br#"{"metadaten":[]}"#.to_vec(),
            ),
            (
                "d.video-cdn.net/play/player/8YRzUk6pTzmBdrsLe9Y88W/video/unknown-film".to_owned(),
                br#"{"videoSources":{"html":{"unknown":[{"source":"https://cdn.example/unknown.bin","mimeType":"video/x-unknown"}]}}}"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context(
            "https://web.arbeitsagentur.de/berufetv/study/film;filmId=unknown-film",
            &context,
        )
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}
