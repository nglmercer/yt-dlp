struct MuseScoreHandler;

impl RequestHandler for MuseScoreHandler {
    fn name(&self) -> &str {
        "musescore-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        let url = request.url();
        if url.contains("musescore.com/api/jmuse") {
            assert_eq!(
                request.headers().get("authorization"),
                Some("096c")
            );
            return Ok(Response::new(
                request.url(),
                200,
                "OK",
                br#"{"info":{"url":"https://cdn.example/musescore/native.mp3"}}"#.to_vec(),
            ));
        }
        Ok(Response::new(
            request.url(),
            200,
            "OK",
            b"<meta property=\"og:url\" content=\"https://musescore.com/user/73797/scores/142975\">
                <meta property=\"og:title\" content=\"Native MuseScore title\">
                <meta name=\"description\" content=\"Native score description\">
                <meta property=\"og:image\" content=\"https://cdn.example/musescore/native.jpg\">
                <meta property=\"musescore:author\" content=\"Native Pianist\">
                <meta property=\"musescore:composer\" content=\"Native Composer\">"
                .to_vec(),
        ))
    }
}

fn musescore_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(MuseScoreHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

#[test]
fn musescore_native_extractor_maps_authenticated_mp3_and_metadata() {
    let extractor = MuseScoreExtractor::new(ExtractorDescriptor::new(
        "MuseScoreIE",
        "MuseScore",
        r#"https?://(?:www\.)?musescore\.com/(?:user/\d+|[^/]+)(?:/scores)?/(?P<id>[^#&?]+)"#,
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://musescore.com/user/73797/scores/142975",
            &musescore_context(),
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("142975"));
    assert_eq!(result.get_str("title"), Some("Native MuseScore title"));
    assert_eq!(
        result.get_str("description"),
        Some("Native score description")
    );
    assert_eq!(result.get_str("uploader"), Some("Native Pianist"));
    assert_eq!(result.get_str("creator"), Some("Native Composer"));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/musescore/native.jpg")
    );
    assert_eq!(result.get_str("url"), Some("https://cdn.example/musescore/native.mp3"));
    assert_eq!(result.get_str("ext"), Some("mp3"));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
}
