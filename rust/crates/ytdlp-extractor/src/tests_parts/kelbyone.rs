struct KelbyOneHandler;

impl RequestHandler for KelbyOneHandler {
    fn name(&self) -> &str {
        "kelbyone-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        let url = request.url();
        if url.contains("members.kelbyone.com/course/native-course") {
            let body = br#"<div data-config='playlist":"https:\/\/content.jwplatform.com\/v2\/playlists\/native.json"'></div>"#;
            return Ok(Response::new(url, 200, "OK", body.to_vec()));
        }
        if url == "https://content.jwplatform.com/v2/playlists/native.json" {
            let body = br#"{
                "title": "Native KelbyOne course",
                "description": "Native course description",
                "playlist": [{
                    "mediaid": "native-media",
                    "title": "Native lesson",
                    "description": "Lesson description",
                    "image": "https://cdn.example/native-poster.jpg",
                    "pubdate": 1601568639,
                    "duration": 90,
                    "images": [{"src":"https://cdn.example/native-thumb.jpg","width":720}],
                    "sources": [
                        {"file":"https://cdn.example/native-720.mp4","label":"720p","width":1280,"height":720},
                        {"file":"https://cdn.example/native.m3u8","type":"application/vnd.apple.mpegurl"},
                        {"file":"https://cdn.example/native-audio.mp4","type":"audio/mp4","label":"audio"}
                    ],
                    "tracks": [{"kind":"captions","file":"https://cdn.example/native-en.vtt"}]
                }]
            }"#;
            return Ok(Response::new(url, 200, "OK", body.to_vec()));
        }
        Err(RequestError::new(
            ErrorKind::Transport,
            format!("no KelbyOne route for {url}"),
        ))
    }
}

fn kelbyone_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(KelbyOneHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

#[test]
fn kelbyone_native_extractor_maps_playlist_sources_and_captions() {
    let extractor = KelbyOneExtractor::new(ExtractorDescriptor::new(
        "KelbyOneIE",
        "KelbyOne",
        r#"https?://members\.kelbyone\.com/course/(?P<id>[^$&?#/]+)"#,
        false,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://members.kelbyone.com/course/native-course/",
            &kelbyone_context(),
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("native-course"));
    assert_eq!(result.get_str("title"), Some("Native KelbyOne course"));
    let entries = result
        .get("entries")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].get("id"), Some(&serde_json::json!("native-media")));
    assert_eq!(entries[0].get("duration"), Some(&serde_json::json!(90.0)));
    assert_eq!(
        entries[0]
            .get("subtitles")
            .and_then(|subtitles| subtitles.get("en"))
            .and_then(serde_json::Value::as_array)
            .and_then(|captions| captions.first())
            .and_then(|caption| caption.get("url")),
        Some(&serde_json::json!("https://cdn.example/native-en.vtt"))
    );
    assert_eq!(
        entries[0]
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(3)
    );
}
