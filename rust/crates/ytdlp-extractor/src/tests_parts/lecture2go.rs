struct Lecture2GoHandler;

impl RequestHandler for Lecture2GoHandler {
    fn name(&self) -> &str {
        "lecture2go-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        if request
            .url()
            .starts_with("https://lecture2go.uni-hamburg.de/veranstaltungen/-/v/17473")
        {
            let body = r#"
                <em class="title">2 - Endliche Automaten und reguläre Sprachen</em>
                <div id="description">Frank Heitmann</div>
                <em>Duration:</em> <em>1:27:00</em>
                <em>Views:</em> <em class="value">123</em>
                <script>
                    var playerUri0 = "https://cdn.example/lecture/master.m3u8";
                    var playerUri1 = "https://cdn.example/lecture/video.mp4";
                    var playerUri2 = "rtmp://cdn.example/lecture/legacy";
                </script>
            "#;
            return Ok(Response::new(
                request.url(),
                200,
                "OK",
                body.as_bytes().to_vec(),
            ));
        }
        Err(RequestError::new(
            ErrorKind::Transport,
            format!("no Lecture2Go route for {}", request.url()),
        ))
    }
}

fn lecture2go_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(Lecture2GoHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

#[test]
fn lecture2go_native_extractor_maps_embedded_players_and_metadata() {
    let extractor = Lecture2GoExtractor::new(ExtractorDescriptor::new(
        "Lecture2GoIE",
        "Lecture2Go",
        r#"https?://lecture2go\.uni-hamburg\.de/veranstaltungen/-/v/(?P<id>\d+)"#,
        false,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://lecture2go.uni-hamburg.de/veranstaltungen/-/v/17473",
            &lecture2go_context(),
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("17473"));
    assert_eq!(
        result.get_str("title"),
        Some("2 - Endliche Automaten und reguläre Sprachen")
    );
    assert_eq!(result.get_str("creator"), Some("Frank Heitmann"));
    assert_eq!(result.get_f64("duration"), Some(5220.0));
    assert_eq!(result.get_i64("view_count"), Some(123));
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(formats[0].get("format_id"), Some(&serde_json::json!("hls")));
    assert_eq!(formats[0].get("protocol"), Some(&serde_json::json!("m3u8_native")));
    assert_eq!(formats[1].get("format_id"), Some(&serde_json::json!("https")));
    assert_eq!(formats[1].get("ext"), Some(&serde_json::json!("mp4")));
}
