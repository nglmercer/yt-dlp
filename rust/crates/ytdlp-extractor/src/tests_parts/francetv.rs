#[test]
fn francetv_native_extractor_maps_api_metadata_and_hls_formats() {
    let extractor = FranceTvExtractor::new(ExtractorDescriptor::new(
        "FranceTVIE",
        "francetv",
        r#"francetv:(?P<id>[^@#]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "k7.ftven.fr/videos/native-france".to_owned(),
            br#"{
                "video":{
                    "url":"https://cdn.example/france/master.m3u8",
                    "format":"hls",
                    "duration":2580,
                    "is_live":false
                },
                "meta":{
                    "title":"Native FranceTV series",
                    "additional_title":"Native episode",
                    "pre_title":"S1 E2",
                    "image_url":"https://cdn.example/france/poster.jpg",
                    "broadcasted_at":"2017-08-13T12:45:00Z"
                }
            }"#
            .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("francetv:native-france", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("native-france"));
    assert_eq!(
        result.get_str("title"),
        Some("Native FranceTV series - Native episode")
    );
    assert_eq!(result.get_f64("duration"), Some(2580.0));
    assert_eq!(result.get_i64("timestamp"), Some(1502628300));
    assert_eq!(result.get_i64("season_number"), Some(1));
    assert_eq!(result.get_i64("episode_number"), Some(2));
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
fn francetv_native_extractor_marks_drm_responses_as_todo() {
    let extractor = FranceTvExtractor::new(ExtractorDescriptor::new(
        "FranceTVIE",
        "francetv",
        r#"francetv:(?P<id>[^@#]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "k7.ftven.fr/videos/drm-france".to_owned(),
            br#"{"code":2015,"message":"DRM only"}"#.to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context("francetv:drm-france", &context)
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}

#[test]
fn francetv_site_native_extractor_reads_next_flight_video_id() {
    let extractor = FranceTvSiteExtractor::new(ExtractorDescriptor::new(
        "FranceTVSiteIE",
        "francetv:site",
        r#"https?://(?:(?:www\.)?france\.tv|mobile\.france\.tv)/(?:[^/]+/)*(?P<id>[^/]+)\.html"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<script>self.__next_f.push([1,"0:[\"$\",\"$L1\",\"\",{\"options\":{\"id\":\"flight-id\"}}]\n"])</script>"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://www.france.tv/show/native-page.html", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("_type"), Some("url_transparent"));
    assert_eq!(result.get_str("url"), Some("francetv:flight-id"));
    assert_eq!(result.get_str("ie_key"), Some("FranceTV"));
}

#[test]
fn franceinfo_native_extractor_reads_player_video_id() {
    let extractor = FranceTvInfoExtractor::new(ExtractorDescriptor::new(
        "FranceTVInfoIE",
        "franceinfo",
        r#"https?://(?:www|mobile|france3-regions)\.france(?:tv)?info.fr/(?:[^/?#]+/)*(?P<id>[^/?#&.]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<script>player.load({src: "https://videos.francetv.fr/video/info-id@France3"});</script>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.francetvinfo.fr/news/native-article",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("_type"), Some("url_transparent"));
    assert_eq!(result.get_str("url"), Some("francetv:info-id"));
}
