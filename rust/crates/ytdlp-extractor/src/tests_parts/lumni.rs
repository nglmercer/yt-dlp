struct LumniHandler;

impl RequestHandler for LumniHandler {
    fn name(&self) -> &str {
        "lumni-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        if !request.url().contains("lumni.fr/video/native-lumni") {
            return Err(RequestError::new(
                ErrorKind::Transport,
                format!("no Lumni route for {}", request.url()),
            ));
        }
        Ok(Response::new(
            request.url(),
            200,
            "OK",
            br#"<div data-factoryid="native-francetv-id"></div>"#.to_vec(),
        ))
    }
}

#[test]
fn lumni_native_extractor_returns_francetv_transparent_result() {
    let mut director = RequestDirector::new();
    director.add_handler(LumniHandler);
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let extractor = LumniExtractor::new(ExtractorDescriptor::new(
        "LumniIE",
        "Lumni",
        r#"https?://(?:www\.)?lumni\.fr/video/(?P<id>[\w-]+)"#,
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://www.lumni.fr/video/native-lumni",
            &context,
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_str("_type"), Some("url_transparent"));
    assert_eq!(
        result.get_str("url"),
        Some("francetv:native-francetv-id")
    );
    assert_eq!(result.get_str("ie_key"), Some("FranceTV"));
    assert_eq!(result.get_str("id"), Some("native-francetv-id"));
}
