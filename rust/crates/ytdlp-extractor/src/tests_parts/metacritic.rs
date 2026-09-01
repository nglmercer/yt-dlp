#[test]
fn metacritic_native_extractor_maps_page_description_and_xml_clip() {
    let extractor = MetacriticExtractor::new(ExtractorDescriptor::new(
        "MetacriticIE",
        "Metacritic",
        r#"https?://(?:www\.)?metacritic\.com/.+?/trailers/(?P<id>\d+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "/trailers/5740315".to_owned(),
                br#"<p><b>Description:</b>Native <em>trailer</em> & details.</p>"#.to_vec(),
            ),
            (
                "/video_data?video=5740315".to_owned(),
                br#"<video_data><playList><clip><id>5740315</id><title>Native Trailer</title><duration>114</duration><httpURI><videoFile><rate>800</rate><filePath>https://cdn.example/trailer.mp4?token=a&quality=hd</filePath></videoFile></httpURI></clip></playList></video_data>"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.metacritic.com/game/demo/trailers/5740315",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("5740315"));
    assert_eq!(result.get_str("title"), Some("Native Trailer"));
    assert_eq!(result.get_str("description"), Some("Native trailer & details."));
    assert_eq!(result.get_i64("duration"), Some(114));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/trailer.mp4?token=a&quality=hd")
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("tbr")),
        Some(&serde_json::json!(800))
    );
}
