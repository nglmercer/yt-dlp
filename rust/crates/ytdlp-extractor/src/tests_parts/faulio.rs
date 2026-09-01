#[test]
fn faulio_native_extractor_maps_api_metadata_and_manifests() {
    let extractor = FaulioExtractor::new(ExtractorDescriptor::new(
        "FaulioIE",
        "Faulio",
        r"https?://(?:aloula\.sba\.sa|bahry\.com|maraya\.sba\.net\.ae|sat7plus\.org)/(?:(?:ar|en|fa)/)?(?:episode|media)/(?P<id>[a-zA-Z0-9-]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "aloula.sba.sa/en/episode/29102".to_owned(),
                br#"<script>window.__NUXT__.config={public:{TRANSLATIONS_API_URL:"https://api.faulio.example"}};</script>"#.to_vec(),
            ),
            (
                "api.faulio.example/video/29102/player".to_owned(),
                br#"{"settings":{"protocols":{"hls":"https://cdn.example/faulio/master.m3u8","dash":"https://cdn.example/faulio/manifest.mpd"}}}"#.to_vec(),
            ),
            (
                "api.faulio.example/video/29102".to_owned(),
                r#"{"blocks":[{"slug":"native-episode-29102","title":"الحلقة 4","description":"","program_title":"هذا مكانك","season_number":3,"episode":4,"image":"https://cdn.example/faulio.jpg","duration":{"total":"4855"},"age_rating":3}]}"#
                    .as_bytes()
                    .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://aloula.sba.sa/en/episode/29102", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("api.faulio.example_29102"));
    assert_eq!(result.get_str("display_id"), Some("native-episode-29102"));
    assert_eq!(result.get_str("title"), Some("الحلقة 4"));
    assert_eq!(result.get_str("episode"), Some("الحلقة 4"));
    assert_eq!(result.get_str("series"), Some("هذا مكانك"));
    assert_eq!(result.get_i64("season_number"), Some(3));
    assert_eq!(result.get_i64("episode_number"), Some(4));
    assert_eq!(result.get_i64("duration"), Some(4855));
    assert_eq!(result.get_i64("age_limit"), Some(3));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(
        result
            .get("http_headers")
            .and_then(|headers| headers.get("Origin")),
        Some(&serde_json::json!("https://aloula.sba.sa"))
    );
}

#[test]
fn faulio_live_native_extractor_maps_channel_hls() {
    let extractor = FaulioLiveExtractor::new(ExtractorDescriptor::new(
        "FaulioLiveIE",
        "FaulioLive",
        r"https?://(?:aloula\.sba\.sa|bahry\.com|maraya\.sba\.net\.ae|sat7plus\.org)/(?:(?:ar|en|fa)/)?live/(?P<id>[a-zA-Z0-9-]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "sat7plus.org/live/pars".to_owned(),
                br#"<script>window.__NUXT__.config={public:{TRANSLATIONS_API_URL:"https://api.sat7.example"}};</script>"#.to_vec(),
            ),
            (
                "api.sat7.example/channels".to_owned(),
                br#"[{"url":"pars","title":"Native SAT-7","description":"Live description","streams":{"hls":"https://cdn.example/sat7/live.m3u8"}}]"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://sat7plus.org/live/pars", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("api.sat7.example_pars"));
    assert_eq!(result.get_str("title"), Some("Native SAT-7"));
    assert_eq!(result.get_bool("is_live"), Some(true));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/sat7/live.m3u8")
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
fn faulio_native_extractor_reports_missing_manifests() {
    let extractor = FaulioExtractor::new(ExtractorDescriptor::new(
        "FaulioIE",
        "Faulio",
        r"https?://(?:aloula\.sba\.sa|bahry\.com|maraya\.sba\.net\.ae|sat7plus\.org)/(?:(?:ar|en|fa)/)?(?:episode|media)/(?P<id>[a-zA-Z0-9-]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "bahry.com/media/1191".to_owned(),
                br#"<script>window.__NUXT__.config={public:{TRANSLATIONS_API_URL:"https://api.faulio.example"}};</script>"#.to_vec(),
            ),
            (
                "api.faulio.example/video/1191/player".to_owned(),
                br#"{"settings":{"protocols":{}}}"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context("https://bahry.com/media/1191", &context)
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Extraction);
    assert!(error.message.contains("no playable HLS or DASH manifest"));
}
