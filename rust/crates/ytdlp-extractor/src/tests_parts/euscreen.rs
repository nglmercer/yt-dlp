use std::sync::Mutex;

struct EuscreenHandler {
    responses: Mutex<Vec<Vec<u8>>>,
}

impl RequestHandler for EuscreenHandler {
    fn name(&self) -> &str {
        "euscreen-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        let body = self
            .responses
            .lock()
            .map_err(|_| RequestError::new(ErrorKind::Transport, "EUScreen test lock poisoned"))?
            .pop()
            .ok_or_else(|| {
                RequestError::new(
                    ErrorKind::Transport,
                    format!("no EUScreen response for {}", request.url()),
                )
            })?;
        Ok(Response::new(request.url(), 200, "OK", body))
    }
}

#[test]
fn euscreen_native_extractor_maps_two_step_player_and_metadata() {
    let extractor = EuscreenExtractor::new(ExtractorDescriptor::new(
        "EUScreenIE",
        "EUScreen",
        r"https?://(?:www\.)?euscreen\.eu/item.html\?id=(?P<id>[^&?$/]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(EuscreenHandler {
        responses: Mutex::new(vec![
            br#"setVideo({sources:[{src:'https://cdn.example/euscreen.mp4'},{src:'https://cdn.example/euscreen.m3u8'}],screenshot:'https://cdn.example/video.jpg'})($end$)put setData({originalTitle:'Native EUScreen',title:'Native alternate',duration:'05:18',summaryOriginal:'Original summary',summaryEnglish:'English summary',series:'Native series',episodeNumber:'-',provider:'Native provider',screenshot:'https://cdn.example/meta.jpg'})($end$)"#.to_vec(),
            br#"<args screenid=\"-1\" />"#.to_vec(),
        ]),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://euscreen.eu/item.html?id=EUS_NATIVE_ITEM",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("EUS_NATIVE_ITEM"));
    assert_eq!(result.get_str("title"), Some("Native EUScreen"));
    assert_eq!(result.get_str("alt_title"), Some("Native alternate"));
    assert_eq!(result.get_f64("duration"), Some(318.0));
    assert_eq!(
        result.get_str("description"),
        Some("Original summary\nEnglish summary")
    );
    assert_eq!(result.get_str("series"), Some("Native series"));
    assert_eq!(result.get_str("episode"), Some("-"));
    assert_eq!(result.get_str("uploader"), Some("Native provider"));
    assert_eq!(result.get_str("thumbnail"), Some("https://cdn.example/meta.jpg"));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
}
