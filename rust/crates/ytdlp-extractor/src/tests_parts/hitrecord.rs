#[test]
fn hitrecord_native_extractor_maps_api_mp4_metadata() {
    let extractor = HitRecordExtractor::new(ExtractorDescriptor::new(
        "HitRecordIE",
        "HitRecord",
        r#"https?://(?:www\.)?hitrecord\.org/records/(?P<id>\d+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "hitrecord.org/api/web/records/2954362".to_owned(),
                br#"{"title":"Native HitRecord","source_url":{"mp4_url":"https://cdn.example/hitrecord.mp4"},"body":"<p>Native <b>description</b></p>","duration":139327,"created_at_i":1471557582,"total_views_count":42,"hearts_count":7,"comments_count":3,"user":{"username":"Zuzi.C12","id":362811},"tags":[{"text":"native"},{"text":"film"}]}"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://hitrecord.org/records/2954362", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("2954362"));
    assert_eq!(result.get_str("title"), Some("Native HitRecord"));
    assert_eq!(result.get_str("ext"), Some("mp4"));
    assert_eq!(result.get_f64("duration"), Some(139.327));
    assert_eq!(result.get_i64("timestamp"), Some(1_471_557_582));
    assert_eq!(result.get_str("uploader"), Some("Zuzi.C12"));
    assert_eq!(result.get_i64("view_count"), Some(42));
    assert_eq!(result.get_str("description"), Some("Native description"));
}
