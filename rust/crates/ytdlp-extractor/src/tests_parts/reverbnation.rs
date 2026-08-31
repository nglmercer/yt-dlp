#[test]
fn reverbnation_native_extractor_maps_song_api_and_thumbnail_preferences() {
    let extractor = ReverbNationExtractor::new(ExtractorDescriptor::new(
        "ReverbNationIE",
        "ReverbNation",
        r"https?://(?:www\.)?reverbnation\.com/.*?/song/(?P<id>\d+).*?$",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "api.reverbnation.com/song/16965047".to_owned(),
                br#"{
                    "name":"MONA LISA",
                    "url":"https://audio.example/reverbnation/16965047.mp3",
                    "artist":{"name":"ALKILADOS","id":216429},
                    "thumbnail":"https://img.example/thumb.jpg",
                    "image":"https://img.example/image.jpg"
                }"#
                .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "http://www.reverbnation.com/alkilados/song/16965047-mona-lisa",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("16965047"));
    assert_eq!(result.get_str("title"), Some("MONA LISA"));
    assert_eq!(result.get_str("uploader"), Some("ALKILADOS"));
    assert_eq!(result.get_str("uploader_id"), Some("216429"));
    assert_eq!(result.get_str("ext"), Some("mp3"));
    assert_eq!(result.get_str("vcodec"), Some("none"));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("url"))
            .and_then(serde_json::Value::as_str),
        Some("https://audio.example/reverbnation/16965047.mp3")
    );
    assert_eq!(
        result.get("thumbnails"),
        Some(&serde_json::json!([
            {"url":"https://img.example/thumb.jpg","preference":0},
            {"url":"https://img.example/image.jpg","preference":1}
        ]))
    );
}
