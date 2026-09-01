struct LemondeLentaHandler;

impl RequestHandler for LemondeLentaHandler {
    fn name(&self) -> &str {
        "lemonde-lenta-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        let body = if request.url().contains("native-lemonde") {
            br#"<script>const player = {url: '//www.digiteka.net/deliver/native-lemonde'};</script>"#.to_vec()
        } else if request.url().contains("fallback-lemonde") {
            br#"<html><head><title>Native fallback</title></head><body></body></html>"#.to_vec()
        } else if request.url().contains("native-lenta") {
            br#"<script>var player = {vid: '12345'};</script>"#.to_vec()
        } else if request.url().contains("fallback-lenta") {
            br#"<html><head><title>Native Lenta fallback</title></head><body></body></html>"#.to_vec()
        } else {
            return Err(RequestError::new(
                ErrorKind::Transport,
                format!("no Lemonde/Lenta route for {}", request.url()),
            ));
        };
        Ok(Response::new(request.url(), 200, "OK", body))
    }
}

fn lemonde_lenta_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(LemondeLentaHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

#[test]
fn lemonde_native_extractor_redirects_to_digiteka_or_generic() {
    let extractor = LemondeExtractor::new(ExtractorDescriptor::new(
        "LemondeIE",
        "Lemonde",
        r#"https?://(?:.+?\.)?lemonde\.fr/(?:[^/]+/)*(?P<id>[^/]+)\.html"#,
        true,
    ))
    .unwrap();

    assert_eq!(
        extractor
            .extract_with_context(
                "https://www.lemonde.fr/native-lemonde.html",
                &lemonde_lenta_context(),
            )
            .unwrap(),
        ExtractorResult::Redirect {
            url: "https://www.digiteka.net/deliver/native-lemonde".to_owned(),
            ie_key: Some("Digiteka".to_owned()),
        }
    );
    assert_eq!(
        extractor
            .extract_with_context(
                "https://www.lemonde.fr/fallback-lemonde.html",
                &lemonde_lenta_context(),
            )
            .unwrap(),
        ExtractorResult::Redirect {
            url: "https://www.lemonde.fr/fallback-lemonde.html".to_owned(),
            ie_key: Some("Generic".to_owned()),
        }
    );
}

#[test]
fn lenta_native_extractor_marks_eagleplatform_as_todo() {
    let extractor = LentaExtractor::new(ExtractorDescriptor::new(
        "LentaIE",
        "Lenta",
        r#"https?://(?:www\.)?lenta\.ru/[^/]+/\d+/\d+/\d+/(?P<id>[^/?#&]+)"#,
        false,
    ))
    .unwrap();
    let error = extractor
        .extract_with_context(
            "https://www.lenta.ru/news/2024/01/02/native-lenta",
            &lemonde_lenta_context(),
        )
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
    assert!(error.message.contains("EaglePlatform"));
}

#[test]
fn lenta_native_extractor_falls_back_to_generic_without_video_id() {
    let extractor = LentaExtractor::new(ExtractorDescriptor::new(
        "LentaIE",
        "Lenta",
        r#"https?://(?:www\.)?lenta\.ru/[^/]+/\d+/\d+/\d+/(?P<id>[^/?#&]+)"#,
        false,
    ))
    .unwrap();
    assert_eq!(
        extractor
            .extract_with_context(
                "https://www.lenta.ru/news/2024/01/02/fallback-lenta",
                &lemonde_lenta_context(),
            )
            .unwrap(),
        ExtractorResult::Redirect {
            url: "https://www.lenta.ru/news/2024/01/02/fallback-lenta".to_owned(),
            ie_key: Some("Generic".to_owned()),
        }
    );
}
