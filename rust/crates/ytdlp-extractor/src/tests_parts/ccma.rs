#[test]
fn ccma_native_extractor_maps_video_api_metadata_formats_and_subtitles() {
    let extractor = CcmaExtractor::new(ExtractorDescriptor::new(
        "CCMAIE",
        "CCMA",
        r"https?://(?:www\.)?3cat\.cat/(?:3cat|tv3/sx3)/[^/?#]+/(?P<type>video|audio)/(?P<id>\d+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "api-media.3cat.cat/pvideo/media.jsp".to_owned(),
            r#"{
                "media":{"url":[
                    {"file":"https://cdn.example/3cat/360.mp4","label":"360p"},
                    {"file":"https://cdn.example/3cat/master.mpd","label":"dash"}
                ]},
                "informacio":{
                    "titol":"Native 3Cat title",
                    "descripcio":"<p>Native 3Cat description</p>",
                    "durada":{"milisegons":79000},
                    "data_emissio":{"utc":"2016-11-08T00:00:00Z"},
                    "tematica":{"text":"Divulgació"},
                    "codi_etic":{"id":"C_13"},
                    "titol_complet":"Native full title",
                    "capitol":5,
                    "programa":"Native series"
                },
                "subtitols":{"iso":"ca","url":"https://cdn.example/3cat/ca.vtt"},
                "imatges":{"url":"https://cdn.example/3cat/poster.jpg","amplada":1280,"alcada":720}
            }"#
            .as_bytes()
            .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.3cat.cat/3cat/native-title/video/5630208/",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("5630208"));
    assert_eq!(result.get_str("title"), Some("Native 3Cat title"));
    assert_eq!(
        result.get_str("description"),
        Some("Native 3Cat description")
    );
    assert_eq!(result.get_f64("duration"), Some(79.0));
    assert_eq!(result.get_str("upload_date"), Some("20161108"));
    assert_eq!(result.get_i64("age_limit"), Some(13));
    assert_eq!(result.get_str("series"), Some("Native series"));
    assert_eq!(result.get_i64("episode_number"), Some(5));
    assert_eq!(
        result.get("categories"),
        Some(&serde_json::json!(["Divulgació"]))
    );
    assert_eq!(
        result
            .get("subtitles")
            .and_then(|value| value.get("ca"))
            .and_then(serde_json::Value::as_array)
            .and_then(|entries| entries.first())
            .and_then(|entry| entry.get("url")),
        Some(&serde_json::json!("https://cdn.example/3cat/ca.vtt"))
    );
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(formats[0].get("height"), Some(&serde_json::json!(360)));
    assert_eq!(formats[1].get("format_id"), Some(&serde_json::json!("dash")));
    assert_eq!(
        formats[1].get("protocol"),
        Some(&serde_json::json!("http_dash_segments"))
    );
}

#[test]
fn ccma_native_extractor_marks_legacy_streams_as_todo() {
    let extractor = CcmaExtractor::new(ExtractorDescriptor::new(
        "CCMAIE",
        "CCMA",
        r"https?://(?:www\.)?3cat\.cat/(?:3cat|tv3/sx3)/[^/?#]+/(?P<type>video|audio)/(?P<id>\d+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "api-media.3cat.cat/pvideo/media.jsp".to_owned(),
            br#"{"media":{"url":[{"file":"rtmp://legacy.example/app/stream","label":"legacy"}]},"informacio":{"titol":"Legacy"}}"#
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context("https://3cat.cat/3cat/legacy/video/99", &context)
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}
