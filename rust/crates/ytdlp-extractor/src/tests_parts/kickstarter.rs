struct KickstarterHandler;

impl RequestHandler for KickstarterHandler {
    fn name(&self) -> &str {
        "kickstarter-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        let url = request.url();
        let body = if url.contains("/projects/1404461844/") {
            r#"<html><head><title>Native Kickstarter project &mdash; Kickstarter</title><meta property="og:description" content="Native project description"><meta property="og:image" content="https://cdn.example/kickstarter.jpg"></head><body><video data-video-url="https://cdn.example/kickstarter.mp4"></video></body></html>"#
        } else if url.contains("/projects/597507018/") {
            r#"<html><head><title>Embedded Kickstarter project &mdash; Kickstarter</title></head><body><iframe src="https://player.example/embed/78704821"></iframe></body></html>"#
        } else {
            return Err(RequestError::new(
                ErrorKind::Transport,
                format!("no Kickstarter route for {url}"),
            ));
        };
        Ok(Response::new(url, 200, "OK", body.as_bytes().to_vec()))
    }
}

fn kickstarter_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(KickstarterHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

#[test]
fn kickstarter_native_extractor_maps_direct_project_video() {
    let extractor = KickstarterExtractor::new(ExtractorDescriptor::new(
        "KickStarterIE",
        "KickStarter",
        r#"https?://(?:www\.)?kickstarter\.com/projects/(?P<id>[^/]*)/.*"#,
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://www.kickstarter.com/projects/1404461844/native-project/description",
            &kickstarter_context(),
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("1404461844"));
    assert_eq!(
        result.get_str("title"),
        Some("Native Kickstarter project")
    );
    assert_eq!(
        result.get_str("description"),
        Some("Native project description")
    );
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/kickstarter.jpg")
    );
    assert_eq!(result.get_str("ext"), Some("mp4"));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/kickstarter.mp4")
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
}

#[test]
fn kickstarter_native_extractor_marks_provider_fallback_as_todo() {
    let extractor = KickstarterExtractor::new(ExtractorDescriptor::new(
        "KickStarterIE",
        "KickStarter",
        r#"https?://(?:www\.)?kickstarter\.com/projects/(?P<id>[^/]*)/.*"#,
        true,
    ))
    .unwrap();
    let error = extractor
        .extract_with_context(
            "https://www.kickstarter.com/projects/597507018/embedded-project/posts/659178",
            &kickstarter_context(),
        )
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}
