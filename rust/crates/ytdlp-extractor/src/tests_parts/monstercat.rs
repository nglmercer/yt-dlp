struct MonstercatHandler {
    body: Vec<u8>,
}

impl RequestHandler for MonstercatHandler {
    fn name(&self) -> &str {
        "monstercat-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        Ok(Response::new(request.url(), 200, "OK", self.body.clone()))
    }
}

#[test]
fn monstercat_native_extractor_materializes_release_tracks_and_metadata() {
    let extractor = MonstercatExtractor::new(ExtractorDescriptor::new(
        "MonstercatIE",
        "Monstercat",
        r#"https?://www\.monstercat\.com/release/(?P<id>\d{12}|MC[A-Z]+\d+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(MonstercatHandler {
        body: br#"<html>
            <h1>The Native Language of Trees</h1>
            <div class="h-normal text-uppercase mb-desktop-medium mb-smallish">Native Artist</div>
            <div class="font-italic mb-medium d-tablet-none d-phone-block">Released July 11, 2023</div>
            <table class="table table-small">
                <tr>
                    <td class="py-xsmall">1</td>
                    <td><div class="d-inline-flex flex-column">First Track <span>extra</span></div>
                    <div class="d-block fs-xxsmall">Producer One</div></td>
                    <button class="btn-play cursor-pointer mr-small" data-track-id="track-1" data-release-id="release-1"></button>
                </tr>
                <tr>
                    <td class="py-xsmall">2</td>
                    <td><div class="d-inline-flex flex-column">Second Track <span>extra</span></div>
                    <div class="d-block fs-xxsmall">Producer Two</div></td>
                    <button class="btn-play cursor-pointer mr-small" data-track-id="track-2" data-release-id="release-1"></button>
                </tr>
            </table>
        </html>"#
        .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.monstercat.com/release/742779548009",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("_type"), Some("playlist"));
    assert_eq!(result.get_str("id"), Some("742779548009"));
    assert_eq!(result.get_str("title"), Some("The Native Language of Trees"));
    assert_eq!(result.get_str("release_date"), Some("20230711"));
    let entries = result
        .get("entries")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get("id").and_then(serde_json::Value::as_str), Some("track-1"));
    assert_eq!(
        entries[0].get("title").and_then(serde_json::Value::as_str),
        Some("First Track")
    );
    assert_eq!(
        entries[0].get("url").and_then(serde_json::Value::as_str),
        Some("https://www.monstercat.com/api/release/release-1/track-stream/track-1")
    );
}
