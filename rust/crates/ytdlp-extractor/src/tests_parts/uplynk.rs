#[test]
fn uplynk_native_extractor_maps_asset_info_and_session_query() {
    let extractor = UplynkExtractor::new(ExtractorDescriptor::new(
        "UplynkIE",
        "uplynk",
        r#"(?x)https?://[\w-]+\.uplynk\.com/(?P<path>
            ext/[0-9a-f]{32}/(?P<external_id>[^/?&]+)|
            (?P<id>[0-9a-f]{32})
        )\.(?:m3u8|json)(?:.*?\bpbs=(?P<session_id>[^&]+))?"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "content.uplynk.com/player/assetinfo/0123456789abcdef0123456789abcdef.json"
                .to_owned(),
            br#"{
                "asset":"asset-123",
                "desc":"Native Uplynk asset",
                "default_poster_url":"https://cdn.example/uplynk.jpg",
                "duration":"12.5",
                "owner":"owner-1"
            }"#
            .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://content.uplynk.com/0123456789abcdef0123456789abcdef.m3u8?pbs=session-1",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("asset-123"));
    assert_eq!(result.get_str("title"), Some("Native Uplynk asset"));
    assert_eq!(result.get_f64("duration"), Some(12.5));
    assert_eq!(
        result.get_str("url"),
        Some("http://content.uplynk.com/0123456789abcdef0123456789abcdef.m3u8?pbs=session-1")
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("extra_param_to_segment_url")),
        Some(&serde_json::json!("pbs=session-1"))
    );
}

#[test]
fn uplynk_preplay_native_extractor_resolves_sid_to_content() {
    let extractor = UplynkPreplayExtractor::new(ExtractorDescriptor::new(
        "UplynkPreplayIE",
        "uplynk:preplay",
        r#"https?://[\w-]+\.uplynk\.com/preplay2?/(?P<path>ext/[0-9a-f]{32}/(?P<external_id>[^/?&]+)|(?P<id>[0-9a-f]{32}))\.json"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "abc.uplynk.com/preplay2/ext/0123456789abcdef0123456789abcdef/episode.json"
                    .to_owned(),
                br#"{"sid":"sid-42"}"#.to_vec(),
            ),
            (
                "content.uplynk.com/player/assetinfo/ext/0123456789abcdef0123456789abcdef/episode.json"
                    .to_owned(),
                br#"{"asset":"asset-episode","desc":"Episode"}"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://abc.uplynk.com/preplay2/ext/0123456789abcdef0123456789abcdef/episode.json",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("asset-episode"));
    assert_eq!(
        result.get_str("url"),
        Some(
            "http://content.uplynk.com/ext/0123456789abcdef0123456789abcdef/episode.m3u8?pbs=sid-42"
        )
    );
}
