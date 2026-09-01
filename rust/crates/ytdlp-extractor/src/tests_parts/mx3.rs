struct Mx3Handler;

impl RequestHandler for Mx3Handler {
    fn name(&self) -> &str {
        "mx3-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        if request.method() == "HEAD" {
            let mut response = Response::new(request.url(), 200, "OK", Vec::new());
            response.headers_mut().add(
                "Content-Type",
                if request.url().contains("download") {
                    "video/quicktime"
                } else {
                    "audio/wav"
                },
            );
            response.headers_mut().add("Content-Length", "1234");
            return Ok(response);
        }
        if request.url().ends_with(".json") {
            return Ok(Response::new(
                request.url(),
                200,
                "OK",
                br#"{"title":"Native Mx3 track","performer_name":"Performer","artist":"Album Artist","composer_name":"Composer","picture_url_xlarge":"https://cdn.example/native-large.jpg"}"#.to_vec(),
            ));
        }
        Ok(Response::new(
            request.url(),
            200,
            "OK",
            br#"<html>
                <div class="single-band-genre">Rock</div>
                <div class="single-more-info">
                    <dl>
                        <dt>Year of creation</dt><dd>2024</dd>
                        <dt>Description</dt><dd>Native Mx3 description</dd>
                        <dt>Tag</dt><dd>native, rust</dd>
                    </dl>
                </div>
            </html>"#
            .to_vec(),
        ))
    }
}

#[test]
fn mx3_native_extractor_probes_media_and_maps_track_metadata() {
    let extractor = Mx3Extractor::new(ExtractorDescriptor::new(
        "Mx3IE",
        "Mx3",
        r#"https?://(?:www\.)?mx3\.ch/t/(?P<id>\w+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(Mx3Handler);
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://mx3.ch/t/native", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("native"));
    assert_eq!(result.get_str("title"), Some("Native Mx3 track"));
    assert_eq!(result.get_str("artist"), Some("Performer"));
    assert_eq!(result.get_str("album_artist"), Some("Album Artist"));
    assert_eq!(result.get_str("genre"), Some("Rock"));
    assert_eq!(result.get_i64("release_year"), Some(2024));
    assert_eq!(result.get_str("description"), Some("Native Mx3 description"));
    assert_eq!(result.get_str("url"), Some("https://mx3.ch/tracks/native/player_asset"));
    assert_eq!(result.get_str("ext"), Some("wav"));
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 4);
    assert_eq!(formats[2].get("ext").and_then(serde_json::Value::as_str), Some("mov"));
    assert_eq!(formats[2].get("filesize").and_then(serde_json::Value::as_i64), Some(1234));
}

#[test]
fn mx3_native_extractor_selects_the_domain_for_neo_and_volksmusik_descriptors() {
    let mut director = RequestDirector::new();
    director.add_handler(Mx3Handler);
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    for (key, name, url, expected_prefix) in [
        (
            "Mx3NeoIE",
            "Mx3Neo",
            "https://neo.mx3.ch/t/native",
            "https://neo.mx3.ch/tracks/native/",
        ),
        (
            "Mx3VolksmusikIE",
            "Mx3Volksmusik",
            "https://volksmusik.mx3.ch/t/native",
            "https://volksmusik.mx3.ch/tracks/native/",
        ),
    ] {
        let extractor = Mx3Extractor::new(ExtractorDescriptor::new(
            key,
            name,
            if key == "Mx3NeoIE" {
                r#"https?://(?:www\.)?neo\.mx3\.ch/t/(?P<id>\w+)"#
            } else {
                r#"https?://(?:www\.)?volksmusik\.mx3\.ch/t/(?P<id>\w+)"#
            },
            true,
        ))
        .unwrap();
        let result = extractor
            .extract_with_context(url, &context)
            .unwrap()
            .into_info_dict();
        assert!(result
            .get_str("url")
            .is_some_and(|value| value.starts_with(expected_prefix)));
    }
}
