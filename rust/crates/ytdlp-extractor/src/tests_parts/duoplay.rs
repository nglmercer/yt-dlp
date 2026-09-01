#[test]
fn duoplay_native_extractor_registers_session_and_maps_episode_data() {
    let extractor = DuoplayExtractor::new(ExtractorDescriptor::new(
        "DuoplayIE",
        "Duoplay",
        r"https?://duoplay\.ee/(?P<id>\d+)(?:[/?#]|$)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "duoplay.ee/4312/siberi-vomm?ep=24".to_owned(),
                r#"<video-player manifest-url="https://cdn.example/duoplay/master.m3u8"
                    :episode="{&quot;title&quot;:&quot;Native episode&quot;,&quot;synopsis&quot;:&quot;Native synopsis&quot;,
                    &quot;images&quot;:{&quot;original&quot;:&quot;https://cdn.example/poster.jpg&quot;},
                    &quot;duration&quot;:123.5,&quot;airtime&quot;:&quot;2017-05-23T14:50:00&quot;,
                    &quot;telecast_id&quot;:&quot;4312&quot;,&quot;season_id&quot;:2,&quot;subtitle&quot;:&quot;Episode 12&quot;,
                    &quot;episode_nr&quot;:12,&quot;episode_id&quot;:&quot;24&quot;,&quot;year&quot;:2017,
                    &quot;cast&quot;:&quot;First Actor, Second Actor&quot;}"></video-player>"#
                    .as_bytes()
                    .to_vec(),
            ),
            (
                "sts.postimees.ee/session/register".to_owned(),
                br#"{"session":"native-session"}"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://duoplay.ee/4312/siberi-vomm?ep=24", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("4312_24"));
    assert_eq!(result.get_str("display_id"), Some("4312"));
    assert_eq!(result.get_str("title"), Some("Native episode"));
    assert_eq!(result.get_str("description"), Some("Native synopsis"));
    assert_eq!(result.get_str("thumbnail"), Some("https://cdn.example/poster.jpg"));
    assert_eq!(result.get_f64("duration"), Some(123.5));
    assert_eq!(result.get_i64("timestamp"), Some(1_495_551_000));
    assert_eq!(result.get_str("series"), Some("Native episode"));
    assert_eq!(result.get_i64("season_number"), Some(2));
    assert_eq!(result.get_i64("episode_number"), Some(12));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/duoplay/master.m3u8?s=native-session")
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("protocol")),
        Some(&serde_json::json!("m3u8_native"))
    );
}
