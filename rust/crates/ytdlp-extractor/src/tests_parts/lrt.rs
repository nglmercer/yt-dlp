struct LrtHandler;

impl RequestHandler for LrtHandler {
    fn name(&self) -> &str {
        "lrt-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        let body = if request.url().contains("/mediateka/tiesiogiai/") {
            br#"<html><head><meta property="og:title" content="Native LRT Opus"></head><script>{"get_streams_url":"https://cdn.example/lrt/streams.json"}</script></html>"#.to_vec()
        } else if request.url().contains("cdn.example/lrt/streams.json") {
            br#"{"response":{"data":{"content":"https://cdn.example/lrt/live/master.m3u8","contentBackup":"https://cdn.example/lrt/live/backup.m3u8"}}}"#.to_vec()
        } else if request.url().contains("/mediateka/irasas/2000127261") {
            br#"<html><head><link rel="canonical" href="/mediateka/irasas/2000127261/native-content"></head></html>"#.to_vec()
        } else if request
            .url()
            .contains("/servisai/stream_url/vod/media_info/")
        {
            serde_json::to_vec(&serde_json::json!({
                    "id": "2000127261",
                    "title": "Native LRT VOD",
                    "content": "<p>Native VOD description</p>",
                    "date": "30.10.2020 18:30",
                    "tags": [{"name": "Native tag"}],
                    "playlist_item": {
                        "file": "https://cdn.example/lrt/vod/master.m3u8",
                        "image": "/img/native-vod.jpg",
                        "duration": 321,
                        "tracks": [{"file": "/subs/en.vtt", "language": "en"}]
                    }
                }))
                .unwrap()
        } else if request.url().contains("/rest-api/media") {
            serde_json::to_vec(&serde_json::json!({
                    "id": "2000359728",
                    "title": "Native LRT radio",
                    "content": "<p>Native radio description</p>",
                    "date": "12.09.2024 09:12",
                    "tags": [{"name": "Radio tag"}],
                    "playlist_item": {
                        "file": "https://cdn.example/lrt/radio/audio.m3u8",
                        "image": "/img/native-radio.jpg",
                        "duration": 99,
                        "category": ["Native category"]
                    }
                }))
                .unwrap()
        } else {
            return Err(RequestError::new(
                ErrorKind::Transport,
                format!("no LRT route for {}", request.url()),
            ));
        };
        Ok(Response::new(request.url(), 200, "OK", body))
    }
}

fn lrt_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(LrtHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

#[test]
fn lrt_stream_native_extractor_maps_live_hls() {
    let extractor = LrtStreamExtractor::new(ExtractorDescriptor::new(
        "LRTStreamIE",
        "LRTStream",
        r"https?://(?:www\.)?lrt\.lt/mediateka/tiesiogiai/(?P<id>[\w-]+)",
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://www.lrt.lt/mediateka/tiesiogiai/lrt-opus",
            &lrt_context(),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_str("id"), Some("lrt-opus"));
    assert_eq!(result.get_str("title"), Some("Native LRT Opus"));
    assert_eq!(result.get_bool("is_live"), Some(true));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
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
fn lrt_vod_native_extractor_maps_api_metadata_and_subtitles() {
    let extractor = LrtVodExtractor::new(ExtractorDescriptor::with_valid_urls(
        "LRTVODIE",
        "LRTVOD",
        vec![
            r"https?://(?:(?:www|archyvai)\.)?lrt\.lt/mediateka/irasas/(?P<id>[0-9]+)"
                .to_owned(),
            r"https?://(?:(?:www|archyvai)\.)?lrt\.lt/mediateka/video/[^?#]+\?(?:[^#]*&)?episode=(?P<id>[0-9]+)"
                .to_owned(),
        ],
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://www.lrt.lt/mediateka/irasas/2000127261/native-content",
            &lrt_context(),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_str("id"), Some("2000127261"));
    assert_eq!(result.get_str("title"), Some("Native LRT VOD"));
    assert_eq!(
        result.get_str("description"),
        Some("Native VOD description")
    );
    assert_eq!(result.get_i64("timestamp"), Some(1_604_082_600));
    assert_eq!(result.get_f64("duration"), Some(321.0));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("protocol")),
        Some(&serde_json::json!("m3u8_native"))
    );
    assert_eq!(
        result
            .get("subtitles")
            .and_then(|subtitles| subtitles.get("en"))
            .and_then(serde_json::Value::as_array)
            .and_then(|tracks| tracks.first())
            .and_then(|track| track.get("url")),
        Some(&serde_json::json!("https://www.lrt.lt/subs/en.vtt"))
    );
    assert!(extractor.suitable(
        "https://archyvai.lrt.lt/mediateka/video/native?episode=2000127261"
    ));
}

#[test]
fn lrt_radio_native_extractor_maps_audio_metadata() {
    let extractor = LrtRadioExtractor::new(ExtractorDescriptor::new(
        "LRTRadioIE",
        "LRTRadio",
        r"https?://(?:www\.)?lrt\.lt/radioteka/irasas/(?P<id>\d+)/(?P<path>[^?#/]+)",
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://www.lrt.lt/radioteka/irasas/2000359728/native-radio",
            &lrt_context(),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_str("id"), Some("2000359728"));
    assert_eq!(result.get_str("title"), Some("Native LRT radio"));
    assert_eq!(
        result.get_str("description"),
        Some("Native radio description")
    );
    assert_eq!(result.get_f64("duration"), Some(99.0));
    assert_eq!(
        result
            .get("categories")
            .and_then(|categories| categories.as_array())
            .and_then(|categories| categories.first())
            .and_then(serde_json::Value::as_str),
        Some("Native category")
    );
    assert_eq!(
        result
            .get("thumbnail")
            .and_then(serde_json::Value::as_str),
        Some("https://www.lrt.lt/img/native-radio.jpg")
    );
}
