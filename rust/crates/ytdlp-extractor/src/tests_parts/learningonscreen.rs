struct LearningOnScreenHandler;

impl RequestHandler for LearningOnScreenHandler {
    fn name(&self) -> &str {
        "learningonscreen-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        if !request
            .url()
            .contains("learningonscreen.ac.uk/ondemand/index.php/prog/005D81B2")
        {
            return Err(RequestError::new(
                ErrorKind::Transport,
                format!("no Learning on Screen route for {}", request.url()),
            ));
        }
        let body = br#"
            <div id="programme-details">
                <h2>Planet Earth</h2>
                <div class="broadcast-date">Sunday 26 November 2006 7:00pm</div>
                <div class="prog-running-time">1:00:00</div>
            </div>
            <video poster="https://stream.learningonscreen.ac.uk/trilt-cover-images/005D81B2-Planet-Earth-2006-11-26T190000Z-BBC4.jpg">
                <source src="https://stream.learningonscreen.ac.uk/trilt/005D81B2.mp4">
            </video>
        "#;
        Ok(Response::new(
            request.url(),
            200,
            "OK",
            body.to_vec(),
        ))
    }
}

fn learningonscreen_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(LearningOnScreenHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

#[test]
fn learningonscreen_native_extractor_maps_authenticated_html5_media_contract() {
    let extractor = LearningOnScreenExtractor::new(ExtractorDescriptor::new(
        "LearningOnScreenIE",
        "LearningOnScreen",
        r"https?://learningonscreen\.ac\.uk/ondemand/index\.php/prog/(?P<id>\w+)",
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://learningonscreen.ac.uk/ondemand/index.php/prog/005D81B2?bcast=22757013",
            &learningonscreen_context(),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_str("id"), Some("005D81B2"));
    assert_eq!(result.get_str("title"), Some("Planet Earth"));
    assert_eq!(result.get_f64("duration"), Some(3600.0));
    assert_eq!(result.get_i64("timestamp"), Some(1_164_567_600));
    assert_eq!(
        result.get_str("thumbnail"),
        Some(
            "https://stream.learningonscreen.ac.uk/trilt-cover-images/005D81B2-Planet-Earth-2006-11-26T190000Z-BBC4.jpg"
        )
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://stream.learningonscreen.ac.uk/trilt/005D81B2.mp4")
    );
    assert_eq!(result.get_str("ext"), Some("mp4"));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("http_headers"))
            .and_then(|headers| headers.get("Origin")),
        Some(&serde_json::json!("https://learningonscreen.ac.uk"))
    );
}
