struct KalturaHandler;

impl RequestHandler for KalturaHandler {
    fn name(&self) -> &str {
        "kaltura-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        if !request.url().contains("/api_v3/service/multirequest") {
            return Err(RequestError::new(
                ErrorKind::Transport,
                format!("no Kaltura route for {}", request.url()),
            ));
        }
        let body = serde_json::json!([
            {"result": "ok"},
            {"objects": [{"ks": "native-session"}]},
            {"objects": [{
                "name": "Native Kaltura title",
                "description": "<p>Native Kaltura description</p>",
                "thumbnailUrl": "https://cdn.example/kaltura-thumb.jpg",
                "duration": 321.5,
                "createdAt": 1704164645,
                "userId": "native-user",
                "plays": "42",
                "dataUrl": "https://cdn.example/flvclipper/native"
            }]},
            {"objects": [
                {"id": "video-1", "status": 2, "fileExt": "mp4", "bitrate": 1500, "frameRate": 30, "size": 12, "containerFormat": "mp4", "videoCodecId": "h264", "height": 1080, "width": 1920},
                {"id": "audio-1", "status": 2, "fileExt": "mp4", "bitrate": 128, "frameRate": 0, "size": 2, "containerFormat": "mp4"},
                {"id": "pending", "status": 1, "fileExt": "mp4", "bitrate": 900},
                {"id": "drm", "status": 2, "fileExt": "wvm", "bitrate": 900}
            ]},
            {"objects": [
                {"id": "caption-1", "status": 2, "languageCode": "en", "format": 3}
            ]}
        ]);
        Ok(Response::new(
            request.url(),
            200,
            "OK",
            serde_json::to_vec(&body).unwrap(),
        ))
    }
}

fn kaltura_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(KalturaHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

#[test]
fn kaltura_native_extractor_maps_multirequest_flavors_and_captions() {
    let extractor = KalturaExtractor::new(ExtractorDescriptor::new(
        "KalturaIE",
        "Kaltura",
        r#"(?x)(?:kaltura:(?P<partner_id>\w+):(?P<id>\w+)(?::(?P<player_type>\w+))?|https?://(?:(?:www|cdnapi(?:sec)?)\.)?kaltura\.com/index\.php/kwidget/.*)"#,
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context("kaltura:269692:1_native", &kaltura_context())
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("1_native"));
    assert_eq!(result.get_str("title"), Some("Native Kaltura title"));
    assert_eq!(
        result.get_str("description"),
        Some("Native Kaltura description")
    );
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/kaltura-thumb.jpg")
    );
    assert_eq!(result.get("duration"), Some(&serde_json::json!(321.5)));
    assert_eq!(result.get("timestamp"), Some(&serde_json::json!(1704164645)));
    assert_eq!(result.get("view_count"), Some(&serde_json::json!(42)));
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(
        formats[0].get("url"),
        Some(&serde_json::json!(
            "https://cdn.example/serveFlavor/flavorId/video-1"
        ))
    );
    assert_eq!(formats[1].get("vcodec"), Some(&serde_json::json!("none")));
    assert_eq!(
        result
            .get("subtitles")
            .and_then(|value| value.get("en"))
            .and_then(serde_json::Value::as_array)
            .and_then(|tracks| tracks.first())
            .and_then(|track| track.get("ext")),
        Some(&serde_json::json!("vtt"))
    );
}

#[test]
fn kaltura_native_extractor_parses_html5_widget_urls() {
    let extractor = KalturaExtractor::new(ExtractorDescriptor::new(
        "KalturaIE",
        "Kaltura",
        r#"https?://(?:www\.)?kaltura\.com/index\.php/kwidget/.*"#,
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://www.kaltura.com/index.php/kwidget/wid/_269692/uiconf_id/3873291/entry_id/1_native",
            &kaltura_context(),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_str("id"), Some("1_native"));
    assert!(result.get("formats").is_some());
}
