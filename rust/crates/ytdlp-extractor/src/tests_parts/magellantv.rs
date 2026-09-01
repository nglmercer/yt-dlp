fn magellantv_extractor() -> MagellanTvExtractor {
    MagellanTvExtractor::new(ExtractorDescriptor::new(
        "MagellanTVIE",
        "MagellanTV",
        r#"https?://(?:www\.)?magellantv\.com/(?:watch|video)/(?P<id>[\w-]+)"#,
        true,
    ))
    .unwrap()
}

fn magellantv_next_page(data: serde_json::Value) -> Vec<u8> {
    format!(
        r#"<script id="__NEXT_DATA__" type="application/json">{}</script>"#,
        serde_json::to_string(&data).unwrap()
    )
    .into_bytes()
}

#[test]
fn magellantv_native_extractor_maps_video_react_context() {
    let page = magellantv_next_page(serde_json::json!({
        "props": {"pageProps": {"reactContext": {
            "video": {"detail": {
                "title": "Native Magellan episode",
                "metadata": {"description": "Native documentary description"},
                "ratingCategory": "TV-14",
                "duration": 3060,
                "tags": ["Ancient History", "Archaeology"],
                "manifests": [
                    {"hls": {"jwp_video_url": "https://cdn.example/magellan/native.m3u8"}}
                ]
            }}
        }}}
    }));
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![("magellantv.com/watch/native-episode".to_owned(), page)],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = magellantv_extractor()
        .extract_with_context(
            "https://www.magellantv.com/watch/native-episode?type=v",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("native-episode"));
    assert_eq!(result.get_str("title"), Some("Native Magellan episode"));
    assert_eq!(
        result.get_str("description"),
        Some("Native documentary description")
    );
    assert_eq!(result.get_i64("age_limit"), Some(14));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(3060.0)));
    assert_eq!(
        result.get("tags"),
        Some(&serde_json::json!(["Ancient History", "Archaeology"]))
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/magellan/native.m3u8")
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
}

#[test]
fn magellantv_native_extractor_accepts_series_current_episode_state() {
    let page = magellantv_next_page(serde_json::json!({
        "props": {"pageProps": {"reactContext": {
            "series": {"currentEpisode": {
                "title": "Native series episode",
                "duration": "00:44:00",
                "manifests": [
                    {"hls": {"jwp_video_url": "https://cdn.example/magellan/series.m3u8"}}
                ]
            }}
        }}}
    }));
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler { body: page });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = magellantv_extractor()
        .extract_with_context(
            "https://magellantv.com/video/native-series-episode",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("title"), Some("Native series episode"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(2640.0)));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/magellan/series.m3u8")
    );
}
