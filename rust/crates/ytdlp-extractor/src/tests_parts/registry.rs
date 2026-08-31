use yt_dlp_networking::{CookieJar, Request, RequestDirector, RequestError, Response};
use yt_dlp_networking::{ErrorKind, RequestHandler};

struct FakeHandler {
    body: Vec<u8>,
}

impl RequestHandler for FakeHandler {
    fn name(&self) -> &str {
        "extractor-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        Ok(Response::new(request.url(), 200, "OK", self.body.clone()))
    }
}

struct RoutedHandler {
    routes: Vec<(String, Vec<u8>)>,
}

impl RequestHandler for RoutedHandler {
    fn name(&self) -> &str {
        "extractor-route-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        let body = self
            .routes
            .iter()
            .find(|(needle, _)| request.url().contains(needle))
            .map(|(_, body)| body.clone())
            .ok_or_else(|| {
                RequestError::new(
                    ErrorKind::Transport,
                    format!("no test route for {}", request.url()),
                )
            })?;
        Ok(Response::new(request.url(), 200, "OK", body))
    }
}

#[test]
fn registry_preserves_registration_order() {
    let mut registry = ExtractorRegistry::new();
    registry
        .register(
            DescriptorExtractor::new(ExtractorDescriptor::new(
                "first",
                "First",
                r"^https://example\.com/.*$",
                true,
            ))
            .unwrap(),
        )
        .unwrap();
    registry
        .register(
            DescriptorExtractor::new(ExtractorDescriptor::new(
                "second",
                "Second",
                r"^https://example\.com/video$",
                true,
            ))
            .unwrap(),
        )
        .unwrap();

    assert_eq!(registry.len(), 2);
    assert_eq!(
        registry
            .find("https://example.com/video")
            .unwrap()
            .descriptor()
            .key,
        "first"
    );
}

#[test]
fn unported_descriptor_is_explicitly_unsupported() {
    let extractor = DescriptorExtractor::new(ExtractorDescriptor::new(
        "test",
        "Test",
        r"^https://test\.example/",
        false,
    ))
    .unwrap();
    let error = extractor.extract("https://test.example/video").unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
    assert!(error.message.contains("not ported"));
}

#[test]
fn duplicate_keys_and_invalid_patterns_are_rejected() {
    let mut registry = ExtractorRegistry::new();
    let first = DescriptorExtractor::new(ExtractorDescriptor::new(
        "same",
        "Same",
        r"^https://example\.com",
        true,
    ))
    .unwrap();
    registry.register(first).unwrap();
    let duplicate = DescriptorExtractor::new(ExtractorDescriptor::new(
        "same",
        "Same again",
        r"^https://other\.example",
        true,
    ))
    .unwrap();
    assert!(registry.register(duplicate).is_err());
    assert!(
        DescriptorExtractor::new(ExtractorDescriptor::new("broken", "Broken", "[", true,)).is_err()
    );
}

#[test]
fn generated_manifest_preserves_extractor_inventory_and_order() {
    let registry = ExtractorRegistry::generated().unwrap();

    // Refresh this snapshot whenever the source extractor registry is
    // intentionally regenerated.
    assert_eq!(registry.len(), 1_752);
    assert!(registry.native_matchable_count() > 1_000);
    assert!(registry.native_pattern_count() > 1_000);
    assert_eq!(registry.pattern_error_count(), 0);
    assert_eq!(
        registry.iter().last().unwrap().descriptor().key,
        "GenericIE"
    );
    assert_eq!(registry.native_implementation_count(), 130);
}

#[test]
fn generic_extractor_returns_stable_url_metadata() {
    let registry = ExtractorRegistry::generated().unwrap();
    let info = registry
        .extract("https://media.example.test/path/sample-video.MP4?token=1")
        .unwrap();
    assert_eq!(info.get("id"), Some(&serde_json::json!("sample-video")));
    assert_eq!(info.get("title"), Some(&serde_json::json!("sample-video")));
    assert_eq!(info.get("ext"), Some(&serde_json::json!("mp4")));
    assert_eq!(info.get("direct"), Some(&serde_json::json!(true)));
}
