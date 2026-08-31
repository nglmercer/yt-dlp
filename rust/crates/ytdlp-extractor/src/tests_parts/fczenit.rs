#[test]
fn fczenit_native_extractor_maps_player_api_qualities() {
    let extractor = FczenitExtractor::new(ExtractorDescriptor::new(
        "FczenitIE",
        "Fczenit",
        r"https?://(?:www\.)?fc-zenit\.ru/video/(?P<id>[0-9]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "player.fc-zenit.ru/msi/video?video=zenit-native".to_owned(),
                br#"{"data":{
                    "name":"Native FC Zenit video",
                    "preview":"https://cdn.example/zenit/poster.jpg",
                    "duration":91.5,
                    "date":1462283735,
                    "qualities":[
                        {"label":"360","url":"https://cdn.example/zenit/360.mp4"},
                        {"label":"720","url":"https://cdn.example/zenit/720.mp4"}
                    ],
                    "tags":[{"label":"football"},{"label":"native"}]
                }}"#
                    .to_vec(),
            ),
            (
                "fc-zenit.ru/video/41044".to_owned(),
                br#"<script>config = {video_id: 'zenit-native'};</script>"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("http://fc-zenit.ru/video/41044/", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("41044"));
    assert_eq!(result.get_str("title"), Some("Native FC Zenit video"));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/zenit/poster.jpg")
    );
    assert_eq!(result.get("duration"), Some(&serde_json::json!(91.5)));
    assert_eq!(
        result.get("timestamp"),
        Some(&serde_json::json!(1462283735i64))
    );
    assert_eq!(
        result.get("tags"),
        Some(&serde_json::json!(["football", "native"]))
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/zenit/360.mp4")
    );
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(formats[1].get("height"), Some(&serde_json::json!(720)));
}
