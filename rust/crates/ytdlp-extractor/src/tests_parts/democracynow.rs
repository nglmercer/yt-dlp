#[test]
fn democracynow_native_extractor_maps_json_media_and_captions() {
    let extractor = DemocracynowExtractor::new(ExtractorDescriptor::new(
        "DemocracynowIE",
        "democracynow",
        r"https?://(?:www\.)?democracynow\.org/(?P<id>[^\?]*)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "democracynow.org/2015/7/3/native".to_owned(),
            br#"<meta property="og:description" content="Native Democracy Now description">
                <script type="text/json">{
                    "title":"Daily Show for July 03, 2015",
                    "file":"/media/dn2015-0703-001.mp4?token=1",
                    "audio":"/media/dn2015-0703-001.mp3",
                    "caption_file":"/captions/main.vtt",
                    "captions":[{"language":"ES","url":"/captions/es.vtt"}],
                    "image":"/images/poster.jpg"
                }</script>"#
            .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.democracynow.org/2015/7/3/native",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("2015-0703-001"));
    assert_eq!(
        result.get_str("title"),
        Some("Daily Show for July 03, 2015")
    );
    assert_eq!(
        result.get_str("description"),
        Some("Native Democracy Now description")
    );
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://www.democracynow.org/images/poster.jpg")
    );
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(formats[1].get("vcodec"), Some(&serde_json::json!("none")));
    assert_eq!(
        result
            .get("subtitles")
            .and_then(|value| value.get("en"))
            .and_then(serde_json::Value::as_array)
            .and_then(|entries| entries.first())
            .and_then(|entry| entry.get("ext")),
        Some(&serde_json::json!("vtt"))
    );
    assert_eq!(
        result
            .get("subtitles")
            .and_then(|value| value.get("es"))
            .and_then(serde_json::Value::as_array)
            .and_then(|entries| entries.first())
            .and_then(|entry| entry.get("url")),
        Some(&serde_json::json!(
            "https://www.democracynow.org/captions/es.vtt"
        ))
    );
}

#[test]
fn democracynow_native_extractor_requires_playable_media() {
    let extractor = DemocracynowExtractor::new(ExtractorDescriptor::new(
        "DemocracynowIE",
        "democracynow",
        r"https?://(?:www\.)?democracynow\.org/(?P<id>[^\?]*)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "democracynow.org/missing".to_owned(),
            br#"<script type="text/json">{"title":"Missing"}</script>"#.to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context("https://democracynow.org/missing", &context)
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Extraction);
    assert!(error.message.contains("no playable media"));
}
