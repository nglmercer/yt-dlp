#[test]
fn fptplay_native_extractor_maps_signed_hls_and_page_metadata() {
    let extractor = FptplayExtractor::new(ExtractorDescriptor::new(
        "FptplayIE",
        "fptplay",
        r#"https?://fptplay\.vn/xem-video/[^/]+\-(?P<id>\w+)(?:/tap-(?P<episode>\d+)?/?(?:[?#]|$)|)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "fptplay.vn/xem-video/native-show-621a123016f369ebbde55945".to_owned(),
                r#"<html>
                    <h4 class="mb-1 text-2xl text-white">Native &amp; show</h4>
                    <p title="Tập 1A" class="epi-title active">episode</p>
                    <p class="overflow-hidden">Native <b>description</b></p>
                </html>"#
                    .as_bytes()
                    .to_vec(),
            ),
            (
                "api.fptplay.net/api/v6.2_w/stream/vod/621a123016f369ebbde55945/0/auto_vip?st="
                    .to_owned(),
                br#"{"data":{"url":"https://cdn.example/fptplay/master.m3u8"}}"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://fptplay.vn/xem-video/native-show-621a123016f369ebbde55945",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("621a123016f369ebbde55945"));
    assert_eq!(result.get_str("title"), Some("Native & show - Tập 1A"));
    assert_eq!(result.get_str("description"), Some("Native description"));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/fptplay/master.m3u8")
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

#[test]
fn fptplay_native_extractor_uses_zero_based_episode_api_index() {
    let extractor = FptplayExtractor::new(ExtractorDescriptor::new(
        "FptplayIE",
        "fptplay",
        r#"https?://fptplay\.vn/xem-video/[^/]+\-(?P<id>\w+)(?:/tap-(?P<episode>\d+)?/?(?:[?#]|$)|)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "fptplay.vn/xem-video/native-show-61f3aa8a6b3b1d2e73c60eb5/tap-3".to_owned(),
                br#"<meta property="og:title" content="Native show">"#.to_vec(),
            ),
            (
                "api.fptplay.net/api/v6.2_w/stream/vod/61f3aa8a6b3b1d2e73c60eb5/2/auto_vip?st="
                    .to_owned(),
                br#"{"data":{"url":"https://cdn.example/fptplay/episode-3.m3u8"}}"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://fptplay.vn/xem-video/native-show-61f3aa8a6b3b1d2e73c60eb5/tap-3",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("title"), Some("Native show - 3"));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/fptplay/episode-3.m3u8")
    );
}
