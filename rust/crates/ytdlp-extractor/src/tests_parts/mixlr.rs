struct MixlrEventHandler {
    body: Vec<u8>,
}

impl RequestHandler for MixlrEventHandler {
    fn name(&self) -> &str {
        "mixlr-event-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        if request.method() == "HEAD" {
            let mut response = Response::new(request.url(), 200, "OK", Vec::new());
            response.headers_mut().add("Content-Type", "audio/mpeg");
            return Ok(response);
        }
        Ok(Response::new(request.url(), 200, "OK", self.body.clone()))
    }
}

#[test]
fn mixlr_event_native_extractor_maps_api_metadata_and_progressive_audio() {
    let extractor = MixlrExtractor::new(ExtractorDescriptor::new(
        "MixlrIE",
        "Mixlr",
        r#"https?://(?:www\.)?(?P<username>[\w-]+)\.mixlr\.com/events/(?P<id>\d+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(MixlrEventHandler {
        body: br#"{
            "data": {"attributes": {
                "title": "Native Mixlr event",
                "description": "Event description",
                "starts_at": "2025-07-08T03:02:44.478Z",
                "started_at": "2025-07-08T03:02:53.861Z",
                "concurrent_view_count": 123,
                "heart_count": 7,
                "live": true,
                "artwork_url": "https://cdn.example/mixlr.png",
                "broadcaster_id": 7828871
            }},
            "included": [{"attributes": {
                "progressive_stream_url": "https://cdn.example/mixlr.mp3",
                "title": "Included title",
                "live": false
            }}]
        }"#
        .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://suncity-104-9fm.mixlr.com/events/4387115", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("4387115"));
    assert_eq!(result.get_str("uploader"), Some("suncity-104-9fm"));
    assert_eq!(result.get_str("title"), Some("Native Mixlr event"));
    assert_eq!(result.get_str("description"), Some("Event description"));
    assert_eq!(result.get_i64("release_timestamp"), Some(1751943764));
    assert_eq!(result.get_i64("timestamp"), Some(1751943773));
    assert_eq!(result.get_i64("view_count"), Some(123));
    assert_eq!(result.get_i64("like_count"), Some(7));
    assert_eq!(result.get_bool("is_live"), Some(true));
    assert_eq!(result.get_str("live_status"), Some("is_live"));
    assert_eq!(result.get_str("uploader_id"), Some("7828871"));
    assert_eq!(result.get_str("ext"), Some("mp3"));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/mixlr.mp3"));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("vcodec")),
        Some(&serde_json::json!("none"))
    );
}

#[test]
fn mixlr_recording_native_extractor_maps_direct_audio_metadata() {
    let extractor = MixlrRecoringExtractor::new(ExtractorDescriptor::new(
        "MixlrRecoringIE",
        "MixlrRecoring",
        r#"https?://(?:www\.)?(?P<username>[\w-]+)\.mixlr\.com/recordings/(?P<id>\d+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{"data":{"attributes":{
            "file_format":"mp3",
            "url":"https://cdn.example/recording.mp3",
            "title":"Native recording",
            "description":"Recording description",
            "created_at":"2024-02-21T12:22:22Z",
            "duration":10968,
            "artwork_url":"https://cdn.example/recording.jpg",
            "user_id":8659190
        }}}"#
        .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://biblewayng.mixlr.com/recordings/2375193", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("2375193"));
    assert_eq!(result.get_str("ext"), Some("mp3"));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/recording.mp3"));
    assert_eq!(result.get_str("title"), Some("Native recording"));
    assert_eq!(result.get_str("description"), Some("Recording description"));
    assert_eq!(result.get_i64("timestamp"), Some(1708518142));
    assert_eq!(result.get_i64("duration"), Some(10968));
    assert_eq!(result.get_str("uploader_id"), Some("8659190"));
}
