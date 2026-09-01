#[test]
fn europarl_webstream_native_extractor_maps_next_data_and_meeting_api() {
    let extractor = EuroParlWebstreamExtractor::new(ExtractorDescriptor::new(
        "EuroParlWebstreamIE",
        "EuroParlWebstream",
        r"(?x)
            https?://multimedia\.europarl\.europa\.eu/
            (?:\w+/)?webstreaming/(?:[\w-]+_)?(?P<id>[\w-]+)
        ",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "FullMeeting".to_owned(),
                br#"{
                    "id":"62388b15-d85b-4add-99aa-ba12ccf64f0d",
                    "startDateTime":"2022-09-14T12:24:29Z",
                    "meetingVideo":{"hlsUrl":"https://cdn.example/plenary.m3u8"},
                    "meetingVideos":[{"hlsUrl":"https://cdn.example/plenary-backup.m3u8"}]
                }"#
                .to_vec(),
            ),
            (
                "multimedia.europarl.europa.eu".to_owned(),
                br#"<script id="__NEXT_DATA__" type="application/json">{
                    "props":{"pageProps":{
                        "mediaItem":{"title":"Plenary session","mediaSubType":"Recorded"}
                    }}
                }</script>"#
                .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://multimedia.europarl.europa.eu/pl/webstreaming/plenary-session_20220914-0900-PLENARY",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(
        result.get_str("id"),
        Some("62388b15-d85b-4add-99aa-ba12ccf64f0d")
    );
    assert_eq!(
        result.get_str("display_id"),
        Some("20220914-0900-PLENARY")
    );
    assert_eq!(result.get_str("title"), Some("Plenary session"));
    assert_eq!(
        result.get_i64("release_timestamp"),
        Some(1663158269)
    );
    assert_eq!(result.get_str("release_date"), Some("20220914"));
    assert_eq!(result.get("is_live"), Some(&serde_json::json!(false)));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/plenary.m3u8"));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
}

#[test]
fn europarl_webstream_native_extractor_errors_without_hls() {
    let extractor = EuroParlWebstreamExtractor::new(ExtractorDescriptor::new(
        "EuroParlWebstreamIE",
        "EuroParlWebstream",
        r"https?://multimedia\.europarl\.europa\.eu/webstreaming/(?P<id>[\w-]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "FullMeeting".to_owned(),
                br#"{"id":"meeting-id","startDateTime":"2024-01-01T00:00:00Z"}"#.to_vec(),
            ),
            (
                "multimedia.europarl.europa.eu".to_owned(),
                br#"<script id="__NEXT_DATA__">{"props":{"pageProps":{}}}</script>"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context(
            "https://multimedia.europarl.europa.eu/webstreaming/20240101-0000",
            &context,
        )
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Extraction);
    assert!(error.message.contains("no HLS stream"));
}
