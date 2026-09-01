struct KickerHandler;

impl RequestHandler for KickerHandler {
    fn name(&self) -> &str {
        "kicker-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        if !request.url().contains("www.kicker.de/native-article/video") {
            return Err(RequestError::new(
                ErrorKind::Transport,
                format!("no Kicker route for {}", request.url()),
            ));
        }
        Ok(Response::new(
            request.url(),
            200,
            "OK",
            br#"<html><head><title>Native Kicker article</title></head>
                <body><div data-dmprivateid="native_dailymotion"></div></body></html>"#
                .to_vec(),
        ))
    }
}

fn kicker_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(KickerHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

#[test]
fn kicker_native_extractor_returns_dailymotion_redirect() {
    let extractor = KickerExtractor::new(ExtractorDescriptor::new(
        "KickerIE",
        "Kicker",
        r#"https?://(?:www\.)kicker\.(?:de)/(?P<id>[\w-]+)/video"#,
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://www.kicker.de/native-article/video",
            &kicker_context(),
        )
        .unwrap();
    assert_eq!(
        result,
        ExtractorResult::Redirect {
            url: "https://www.dailymotion.com/video/native_dailymotion".to_owned(),
            ie_key: Some("Dailymotion".to_owned()),
        }
    );
}
