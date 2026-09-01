#[test]
fn mzaalo_native_extractor_maps_stream_metadata_subtitles_and_thumbnails() {
    let extractor = MzaaloExtractor::new(ExtractorDescriptor::new(
        "MzaaloIE",
        "Mzaalo",
        r#"(?i)https?://(?:www\.)?mzaalo\.com/(?:play|watch)/(?P<type>movie|original|clip)/(?P<id>[a-f0-9-]+)/[\w-]+"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "/api/v2/player/details".to_owned(),
            r#"{"data":{"streamURL":"https://cdn.example/mzaalo/master.m3u8","title":"Native Jamun","description":"Native film description","duration":"01:32:07","language":"HIN","maturity_rating":"13+","genre":["Drama"],"subtitles":{"en":"https://cdn.example/mzaalo/en.vtt"},"images":[{"url":"https://cdn.example/mzaalo/one.jpg"},{"url":"https://cdn.example/mzaalo/two.jpg"}]}}"#.as_bytes().to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.mzaalo.com/play/movie/c0958d9f-f90e-4503-a755-44358758921d/Jamun",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(
        result.get_str("id"),
        Some("c0958d9f-f90e-4503-a755-44358758921d")
    );
    assert_eq!(result.get_str("title"), Some("Native Jamun"));
    assert_eq!(result.get_str("language"), Some("hin"));
    assert_eq!(result.get_f64("duration"), Some(5527.0));
    assert_eq!(result.get_i64("age_limit"), Some(13));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/mzaalo/one.jpg")
    );
    assert_eq!(
        result.get("categories"),
        Some(&serde_json::json!(["Drama"]))
    );
    assert_eq!(
        result
            .get("subtitles")
            .and_then(|subtitles| subtitles.get("en"))
            .and_then(|entries| entries.get(0))
            .and_then(|entry| entry.get("ext")),
        Some(&serde_json::json!("vtt"))
    );
}
