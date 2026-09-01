struct KompasHandler;

impl RequestHandler for KompasHandler {
    fn name(&self) -> &str {
        "kompas-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        let url = request.url();
        if url.contains("video.kompas.com/watch/164474/native-kompas") {
            let body = br#"<meta property="og:title" content="Fallback Kompas title">
                <meta name="description" content="Fallback description">
                <div id="player"></div>"#;
            return Ok(Response::new(url, 200, "OK", body.to_vec()));
        }
        if url.starts_with("https://apidam.jixie.io/api/public/stream") {
            let body = br#"{
                "data": {
                    "title": "Native Kompas title",
                    "owner_id": "9262bf2590d558736cac4fff7978fcb1",
                    "drm": true,
                    "streams": [
                        {"type":"HLS","url":"https://cdn.example/kompas.m3u8","width":1280,"height":720},
                        {"type":"MP4","url":"https://cdn.example/kompas.mp4","width":640,"height":360}
                    ],
                    "metadata": {
                        "description":"<p>Native <b>Kompas</b> description</p>",
                        "duration":"85.066667",
                        "keywords":"news,world",
                        "categories":"news",
                        "thumbnails":[{"url":"https://cdn.example/kompas.jpg","width":1280}]
                    }
                }
            }"#;
            return Ok(Response::new(url, 200, "OK", body.to_vec()));
        }
        Err(RequestError::new(
            ErrorKind::Transport,
            format!("no Kompas route for {url}"),
        ))
    }
}

fn kompas_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(KompasHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

#[test]
fn kompas_native_extractor_maps_jixie_streams_and_metadata() {
    let extractor = KompasVideoExtractor::new(ExtractorDescriptor::new(
        "KompasVideoIE",
        "KompasVideo",
        r#"https?://video\.kompas\.com/\w+/(?P<id>\d+)/(?P<slug>[\w-]+)"#,
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://video.kompas.com/watch/164474/native-kompas",
            &kompas_context(),
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("164474"));
    assert_eq!(result.get_str("display_id"), Some("native-kompas"));
    assert_eq!(result.get_str("title"), Some("Native Kompas title"));
    assert_eq!(
        result.get_str("description"),
        Some("Native Kompas description")
    );
    assert_eq!(result.get_f64("duration"), Some(85.066667));
    assert_eq!(
        result.get("tags"),
        Some(&serde_json::json!(["news", "world"]))
    );
    assert_eq!(
        result.get("categories"),
        Some(&serde_json::json!(["news"]))
    );
    assert_eq!(
        result.get_str("uploader_id"),
        Some("9262bf2590d558736cac4fff7978fcb1")
    );
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(formats[0].get("has_drm"), Some(&serde_json::json!(true)));
    assert_eq!(
        formats[0].get("protocol"),
        Some(&serde_json::json!("m3u8_native"))
    );
    assert_eq!(formats[1].get("height"), Some(&serde_json::json!(360)));
}
