use yt_dlp_networking::ResponseHeaders;

struct CinetecaHandler {
    body: Vec<u8>,
}

impl RequestHandler for CinetecaHandler {
    fn name(&self) -> &str {
        "cineteca-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        assert!(request.url().contains("/api/catalogo/1942/?"));
        assert_eq!(
            request.headers().get("Referer"),
            Some("https://www.cinetecamilano.it/film/1942")
        );
        assert_eq!(
            request.headers().get("Authorization"),
            Some("Bearer native-token")
        );
        Ok(Response::new(request.url(), 200, "OK", self.body.clone()))
    }
}

#[test]
fn cineteca_milano_native_extractor_maps_api_auth_and_hls() {
    let extractor = CinetecaMilanoExtractor::new(ExtractorDescriptor::new(
        "CinetecaMilanoIE",
        "Cineteca Milano",
        r#"https?://(?:www\.)?cinetecamilano\.it/film/(?P<id>\d+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(CinetecaHandler {
        body: r#"{
            "success": true,
            "archive": {
                "title": "Il draghetto Grisù (4 episodi)",
                "description": "  Native archive description  ",
                "duration": 52.5,
                "updated_at": "2022-01-29 00:00:00",
                "created_at": "2020-05-20 00:00:00",
                "thumb": {"src": "/public/covers/1942.png"},
                "drm": {"hls": "/storage/video/1942/playlist.m3u8"}
            }
        }"#
        .as_bytes()
        .to_vec(),
    });
    let cookie_jar = CookieJar::new().shared();
    {
        let mut jar = cookie_jar.lock().unwrap();
        let mut headers = ResponseHeaders::new();
        headers.add("Set-Cookie", "cnt-token=native-token; Path=/");
        jar.store_response("https://www.cinetecamilano.it/login", &headers)
            .unwrap();
    }
    let context = ExtractionContext::new(director, cookie_jar);
    let result = extractor
        .extract_with_context("https://www.cinetecamilano.it/film/1942", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("1942"));
    assert_eq!(
        result.get_str("title"),
        Some("Il draghetto Grisù (4 episodi)")
    );
    assert_eq!(
        result.get_str("description"),
        Some("Native archive description")
    );
    assert_eq!(result.get("duration"), Some(&serde_json::json!(3150.0)));
    assert_eq!(
        result.get("release_timestamp"),
        Some(&serde_json::json!(1643414400i64))
    );
    assert_eq!(
        result.get("modified_timestamp"),
        Some(&serde_json::json!(1589932800i64))
    );
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://www.cinetecamilano.it/storage/covers/1942.png")
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://www.cinetecamilano.it/storage/video/1942/playlist.m3u8")
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("protocol"))
            .and_then(serde_json::Value::as_str),
        Some("m3u8_native")
    );
}
