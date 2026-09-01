struct ManyVidsHandler;

impl RequestHandler for ManyVidsHandler {
    fn name(&self) -> &str {
        "manyvids-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        let url = request.url();
        if url.ends_with("/bff/store/video/935718/private") {
            return Ok(Response::new(
                url,
                200,
                "OK",
                br#"{"data":{
                    "teaser":{"filepath":"https://cdn.example/manyvids/preview_480_.mp4"},
                    "transcodedFilepath":"https://cdn.example/manyvids/transcoded_720_.mp4",
                    "filepath":"https://cdn.example/manyvids/original.mp4"
                }}"#
                .to_vec(),
            ));
        }
        if url.ends_with("/bff/store/video/935718") {
            return Ok(Response::new(
                url,
                200,
                "OK",
                br#"{"data":{
                    "title":"MY <strong>FACE</strong> REVEAL",
                    "description":"<p>Native ManyVids description</p>",
                    "model":{"displayName":"Sarah Calanthe"},
                    "screenshot":{"thumbnail":"https://cdn.example/manyvids/poster.jpg"},
                    "views":"1,234",
                    "likes":"56",
                    "launchDate":"2018-11-10T12:00:00Z",
                    "videoDuration":"3:44",
                    "tagList":[{"label":"Redhead"},{"label":"Interviews"}]
                }}"#
                .to_vec(),
            ));
        }
        if url.ends_with("/bff/store/video/530341/private") {
            return Ok(Response::new(
                url,
                200,
                "OK",
                br#"{"data":{
                    "teaser":{"filepath":"https://cdn.example/manyvids/preview_480_.mp4"}
                }}"#
                .to_vec(),
            ));
        }
        if url.ends_with("/bff/store/video/530341") {
            return Ok(Response::new(
                url,
                200,
                "OK",
                br#"{"data":{"title":"MV Tips & Tricks"}}"#.to_vec(),
            ));
        }
        Err(RequestError::new(
            ErrorKind::Transport,
            format!("no ManyVids route for {url}"),
        ))
    }
}

fn manyvids_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(ManyVidsHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

fn manyvids_extractor() -> ManyVidsExtractor {
    ManyVidsExtractor::new(
        ExtractorDescriptor::new(
            "ManyVidsIE",
            "ManyVids",
            r#"(?i)https?://(?:www\.)?manyvids\.com/video/(?P<id>\d+)"#,
            true,
        ),
    )
    .unwrap()
}

#[test]
fn manyvids_native_extractor_maps_full_video_and_metadata() {
    let result = manyvids_extractor()
        .extract_with_context(
            "https://www.manyvids.com/Video/935718/MY-FACE-REVEAL/",
            &manyvids_context(),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_str("id"), Some("935718"));
    assert_eq!(result.get_str("title"), Some("MY FACE REVEAL"));
    assert_eq!(
        result.get_str("description"),
        Some("Native ManyVids description")
    );
    assert_eq!(result.get_str("uploader"), Some("Sarah Calanthe"));
    assert_eq!(result.get_i64("view_count"), Some(1234));
    assert_eq!(result.get_i64("like_count"), Some(56));
    assert_eq!(result.get_i64("release_timestamp"), Some(1_541_851_200));
    assert_eq!(result.get_f64("duration"), Some(224.0));
    assert_eq!(
        result.get("tags"),
        Some(&serde_json::json!(["Redhead", "Interviews"]))
    );
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 3);
    assert_eq!(formats[0].get("preference"), Some(&serde_json::json!(-10)));
    assert_eq!(formats[1].get("height"), Some(&serde_json::json!(720)));
    assert_eq!(formats[2].get("quality"), Some(&serde_json::json!(10)));
}

#[test]
fn manyvids_native_extractor_marks_preview_only_results() {
    let result = manyvids_extractor()
        .extract_with_context(
            "https://www.manyvids.com/video/530341/mv-tips-tricks",
            &manyvids_context(),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_str("id"), Some("530341-preview"));
    assert_eq!(result.get_str("title"), Some("MV Tips & Tricks (Preview)"));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("format_id")),
        Some(&serde_json::json!("preview"))
    );
}
