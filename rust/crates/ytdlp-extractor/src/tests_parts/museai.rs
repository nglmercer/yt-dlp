struct MuseAiHandler;

impl RequestHandler for MuseAiHandler {
    fn name(&self) -> &str {
        "muse-ai-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        let body: &[u8] = if request.url().contains("embed/YdTWvUW") {
            br#"<script>
                player.setData({
                    url: "https://cdn.example/muse/YdTWvUW/data",
                    filename: "YdTWvUW.mp4",
                    width: 1920,
                    height: 1080,
                    size: 123456,
                    title: "Native MuseAI data video",
                    description: "Native MuseAI description",
                    duration: 1291.3,
                    tcreated: 1685285044,
                    owner_name: "Native News",
                    owner_username: "native-news",
                    views: 77,
                    mature: false,
                    visibility: "public",
                });
            </script>"#
        } else if request.url().contains("embed/gQ4gGAA") {
            br#"<script>
                player.setData({
                    "url": "https://cdn.example/muse/gQ4gGAA.mp4",
                    "filename": "gQ4gGAA.mp4",
                    "title": "Native MuseAI source video",
                    "description": "",
                    "duration": 21.4,
                    "tcreated": 1615072842,
                    "owner_name": "Native Aerial",
                    "owner_username": "aerial",
                    "views": 12,
                    "mature": true,
                    "visibility": "unlisted",
                });
            </script>"#
        } else {
            return Err(RequestError::new(
                ErrorKind::Transport,
                format!("no MuseAI route for {}", request.url()),
            ));
        };
        Ok(Response::new(
            request.url(),
            200,
            "OK",
            body.to_vec(),
        ))
    }
}

fn museai_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(MuseAiHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

fn museai_extractor() -> MuseAiExtractor {
    MuseAiExtractor::new(ExtractorDescriptor::new(
        "MuseAIIE",
        "MuseAI",
        r#"https?://(?:www\.)?muse\.ai/(?:v|embed)/(?P<id>\w+)"#,
        true,
    ))
    .unwrap()
}

#[test]
fn museai_native_extractor_maps_source_and_adaptive_formats() {
    let result = museai_extractor()
        .extract_with_context("https://muse.ai/embed/YdTWvUW", &museai_context())
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("YdTWvUW"));
    assert_eq!(result.get_str("title"), Some("Native MuseAI data video"));
    assert_eq!(result.get_str("uploader"), Some("Native News"));
    assert_eq!(result.get_str("uploader_id"), Some("native-news"));
    assert_eq!(result.get_f64("duration"), Some(1291.3));
    assert_eq!(result.get_i64("timestamp"), Some(1_685_285_044));
    assert_eq!(result.get_i64("view_count"), Some(77));
    assert_eq!(result.get_str("availability"), Some("public"));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/muse/YdTWvUW/data")
    );
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 3);
    assert_eq!(formats[0].get("width"), Some(&serde_json::json!(1920)));
    assert_eq!(formats[1].get("protocol"), Some(&serde_json::json!("m3u8_native")));
    assert_eq!(formats[2].get("protocol"), Some(&serde_json::json!("http_dash_segments")));
}

#[test]
fn museai_native_extractor_maps_direct_source_metadata() {
    let result = museai_extractor()
        .extract_with_context("https://muse.ai/v/gQ4gGAA-0756", &museai_context())
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("gQ4gGAA"));
    assert_eq!(result.get_str("title"), Some("Native MuseAI source video"));
    assert_eq!(result.get_str("description"), Some(""));
    assert_eq!(result.get_i64("age_limit"), Some(18));
    assert_eq!(result.get_str("availability"), Some("unlisted"));
    assert_eq!(result.get_str("ext"), Some("mp4"));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/muse/gQ4gGAA.mp4")
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
}
