struct ToggleHandler;

impl RequestHandler for ToggleHandler {
    fn name(&self) -> &str {
        "toggle-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        if request.url().contains("cdn.mewatch.sg/api/items/") {
            return Ok(Response::new(
                request.url(),
                200,
                "OK",
                br#"{"customId":"343115"}"#.to_vec(),
            ));
        }
        Ok(Response::new(
            request.url(),
            200,
            "OK",
            br#"{
                "MediaName":"Native Toggle media",
                "Description":"  Native Toggle description  ",
                "Duration":"42",
                "CreationDate":"2020-10-21T12:15:26Z",
                "Rating":"4.5",
                "ViewCounter":"12",
                "like_counter":3,
                "Pictures":[{"URL":"https://cdn.example/toggle.jpg","PicSize":"1280x720"}],
                "Files":[
                    {"URL":"https://cdn.example/native-high.m3u8","Format":"High Quality"},
                    {"URL":"https://cdn.example/native.mp4","Format":"Low Quality"},
                    {"URL":"https://cdn.example/native.mpd","Format":"Dash Quality"},
                    {"URL":"https://cdn.example/fpshls/native.m3u8","Format":"FairPlay"}
                ]
            }"#
            .to_vec(),
        ))
    }
}

#[test]
fn toggle_native_extractor_maps_api_metadata_and_native_manifest_formats() {
    let extractor = ToggleExtractor::new(ExtractorDescriptor::new(
        "ToggleIE",
        "toggle",
        r#"(?:https?://(?:(?:www\.)?mewatch|video\.toggle)\.sg/(?:en|zh)/(?:[^/]+/){2,}|toggle:)(?P<id>[0-9]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(ToggleHandler);
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("toggle:343115", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("343115"));
    assert_eq!(result.get_str("title"), Some("Native Toggle media"));
    assert_eq!(result.get_str("description"), Some("Native Toggle description"));
    assert_eq!(result.get_i64("duration"), Some(42));
    assert_eq!(result.get_i64("timestamp"), Some(1603282526));
    assert_eq!(result.get_str("upload_date"), Some("20201021"));
    assert_eq!(result.get_i64("view_count"), Some(12));
    assert_eq!(result.get_i64("like_count"), Some(3));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/native-high.m3u8"));
    assert_eq!(result.get_str("ext"), Some("mp4"));
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 3);
    assert_eq!(
        formats[0]
            .get("protocol")
            .and_then(serde_json::Value::as_str),
        Some("m3u8_native")
    );
    assert_eq!(
        formats[2]
            .get("protocol")
            .and_then(serde_json::Value::as_str),
        Some("http_dash_segments")
    );
    assert_eq!(
        result
            .get("thumbnails")
            .and_then(serde_json::Value::as_array)
            .and_then(|thumbnails| thumbnails.first())
            .and_then(|thumbnail| thumbnail.get("width"))
            .and_then(serde_json::Value::as_i64),
        Some(1280)
    );
}

#[test]
fn mewatch_native_extractor_redirects_to_toggle_without_python() {
    let extractor = MeWatchExtractor::new(ExtractorDescriptor::new(
        "MeWatchIE",
        "mewatch",
        r#"https?://(?:(?:www|live)\.)?mewatch\.sg/watch/[^/?#&]+-(?P<id>[0-9]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(ToggleHandler);
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.mewatch.sg/watch/Recipe-Of-Life-E1-179371",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("_type"), Some("url"));
    assert_eq!(result.get_str("url"), Some("toggle:343115"));
    assert_eq!(result.get_str("ie_key"), Some("Toggle"));
}

#[test]
fn toggle_native_extractor_marks_smooth_streaming_as_todo_when_unavoidable() {
    struct IsmHandler;

    impl RequestHandler for IsmHandler {
        fn name(&self) -> &str {
            "toggle-ism-test"
        }

        fn supports(&self, _request: &Request) -> Result<(), RequestError> {
            Ok(())
        }

        fn send(&self, request: &Request) -> Result<Response, RequestError> {
            Ok(Response::new(
                request.url(),
                200,
                "OK",
                br#"{"MediaName":"ISM media","Files":[{"URL":"https://cdn.example/native.ism/Manifest","Format":"Smooth"}]}"#.to_vec(),
            ))
        }
    }

    let extractor = ToggleExtractor::new(ExtractorDescriptor::new(
        "ToggleIE",
        "toggle",
        r#"(?:toggle:)(?P<id>[0-9]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(IsmHandler);
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context("toggle:999", &context)
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}
