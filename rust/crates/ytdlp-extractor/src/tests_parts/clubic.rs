#[test]
fn clubic_native_extractor_maps_m6web_configuration() {
    let extractor = ClubicExtractor::new(ExtractorDescriptor::new(
        "ClubicIE",
        "Clubic",
        r"https?://(?:www\.)?clubic\.com/video/(?:[^/]+/)*video.*-(?P<id>[0-9]+)\.html",
        false,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "player.m6web.fr/v1/player/clubic/448474.html".to_owned(),
            br#"<html><script>M6.Player.config = {
                "videoInfo": {
                    "title": "Clubic Week 2.0",
                    "description": "<p>Native <strong>Clubic</strong> description.</p>"
                },
                "sources": [
                    {"streamQuality": "sd", "src": "https://cdn.example/clubic/sd.mp4"},
                    {"streamQuality": "hq", "src": "https://cdn.example/clubic/hq.mp4"}
                ],
                "poster": "https://cdn.example/clubic/poster.jpg"
            };</script></html>"#
            .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "http://www.clubic.com/video/clubic-week/video-clubic-week-2-0-le-fbi-448474.html",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("448474"));
    assert_eq!(result.get_str("title"), Some("Clubic Week 2.0"));
    assert_eq!(
        result.get_str("description"),
        Some("Native Clubic description.")
    );
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/clubic/poster.jpg")
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/clubic/sd.mp4")
    );
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(formats[0].get("quality"), Some(&serde_json::json!(0)));
    assert_eq!(formats[1].get("quality"), Some(&serde_json::json!(1)));
}
