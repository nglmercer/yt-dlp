#[test]
fn canalc2_native_extractor_maps_http_file_and_duration() {
    let extractor = Canalc2Extractor::new(ExtractorDescriptor::new(
        "Canalc2IE",
        "Canalc2",
        r"https?://(?:(?:www\.)?canalc2\.tv/video/|archives-canalc2\.u-strasbg\.fr/video\.asp\?.*\bidVideo=)(?P<id>\d+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "www.canalc2.tv/video/12163".to_owned(),
            r#"<html><div class="col_description"><h3>Native Terrasses du Numérique</h3></div>
                <div id="video_duree">02:02</div>
                <script>file = "https://cdn.example/canalc2/12163.mp4";</script>
            </html>"#
                .as_bytes()
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("http://www.canalc2.tv/video/12163", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("12163"));
    assert_eq!(result.get_str("title"), Some("Native Terrasses du Numérique"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(122.0)));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/canalc2/12163.mp4")
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("ext")),
        Some(&serde_json::json!("mp4"))
    );
}
