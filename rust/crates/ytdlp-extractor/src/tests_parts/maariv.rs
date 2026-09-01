struct MaarivHandler;

impl RequestHandler for MaarivHandler {
    fn name(&self) -> &str {
        "maariv-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        let url = request.url();
        if url.contains("dal.walla.co.il/media/3611585") {
            return Ok(Response::new(
                url,
                200,
                "OK",
                br#"{
                    "data": {
                        "title": "Native Maariv report",
                        "upload_date": "2023-10-09T11:35:01Z",
                        "video": {
                            "duration": 75,
                            "url": "https://cdn.example/maariv/report.m3u8",
                            "stream_urls": [
                                {"stream_url": "https://cdn.example/maariv/report_1280x720.mp4"},
                                {"stream_url": "https://cdn.example/maariv/report_640x360.mp4"}
                            ]
                        }
                    }
                }"#
                .to_vec(),
            ));
        }
        Err(RequestError::new(
            ErrorKind::Transport,
            format!("no Maariv route for {url}"),
        ))
    }
}

fn maariv_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(MaarivHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

#[test]
fn maariv_native_extractor_maps_api_streams_and_metadata() {
    let extractor = MaarivExtractor::new(ExtractorDescriptor::new(
        "MaarivIE",
        "maariv.co.il",
        r#"https?://player\.maariv\.co\.il/public/player\.html\?(?:[^#]+&)?media=(?P<id>\d+)"#,
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://player.maariv.co.il/public/player.html?player=maariv-desktop&media=3611585",
            &maariv_context(),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_str("id"), Some("3611585"));
    assert_eq!(result.get_str("title"), Some("Native Maariv report"));
    assert_eq!(result.get_i64("duration"), Some(75));
    assert_eq!(result.get_i64("timestamp"), Some(1_696_851_301));
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 3);
    assert_eq!(formats[0].get("protocol"), Some(&serde_json::json!("m3u8_native")));
    assert_eq!(formats[1].get("width"), Some(&serde_json::json!(1280)));
    assert_eq!(formats[1].get("height"), Some(&serde_json::json!(720)));
}
