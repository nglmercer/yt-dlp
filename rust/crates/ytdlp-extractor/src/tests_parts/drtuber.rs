#[test]
fn drtuber_native_extractor_maps_player_files_and_page_metadata() {
    let extractor = DrTuberExtractor::new(ExtractorDescriptor::new(
        "DrTuberIE",
        "DrTuber",
        r"https?://(?:(?:www|m)\.)?drtuber\.com/(?:video|embed)/(?P<id>\d+)(?:/(?P<display_id>[\w-]+))?",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "drtuber.com/video/1740434".to_owned(),
                br#"<html><h1 class="title">Native DrTuber title</h1>
                    <video poster="https://cdn.example/drtuber.jpg"></video>
                    <span id="rate_likes">1,234</span>
                    <span id="rate_dislikes">56</span>
                    <span id="comments_count">78</span>
                    <div class="categories_list"><a title="Native"></a><a title="Outdoor"></a></div>
                </html>"#
                    .to_vec(),
            ),
            (
                "drtuber.com/player_config_json/".to_owned(),
                br#"{"files":{"hq":"https://cdn.example/drtuber-hq.mp4","sd":"https://cdn.example/drtuber-sd.mp4"},"duration_format":"01:02:03"}"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.drtuber.com/video/1740434/native-drtuber-title",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("1740434"));
    assert_eq!(result.get_str("display_id"), Some("native-drtuber-title"));
    assert_eq!(result.get_str("title"), Some("Native DrTuber title"));
    assert_eq!(result.get_str("thumbnail"), Some("https://cdn.example/drtuber.jpg"));
    assert_eq!(result.get_f64("duration"), Some(3_723.0));
    assert_eq!(result.get_i64("like_count"), Some(1_234));
    assert_eq!(result.get_i64("dislike_count"), Some(56));
    assert_eq!(result.get_i64("comment_count"), Some(78));
    assert_eq!(
        result.get("categories"),
        Some(&serde_json::json!(["Native", "Outdoor"]))
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
}
