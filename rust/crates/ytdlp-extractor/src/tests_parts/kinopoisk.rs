struct KinopoiskHandler;

impl RequestHandler for KinopoiskHandler {
    fn name(&self) -> &str {
        "kinopoisk-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        let url = request.url();
        if !url.contains("ott-widget.kinopoisk.ru/v1/kp/") {
            return Err(RequestError::new(
                ErrorKind::Transport,
                format!("no KinoPoisk route for {url}"),
            ));
        }
        let body = r#"<html><script type="application/json">{"models":{"filmStatus":{"title":"Native KinoPoisk title","originalTitle":"Original title","description":"Native KinoPoisk description","coverUrl":"https://cdn.example/kinopoisk-cover.jpg","duration":4533,"restrictionAge":12},"playlistEntity":{"uri":"https://cdn.example/kinopoisk.m3u8"}}}</script></html>"#;
        Ok(Response::new(url, 200, "OK", body.as_bytes().to_vec()))
    }
}

#[test]
fn kinopoisk_native_extractor_maps_widget_metadata_and_hls() {
    let extractor = KinopoiskExtractor::new(ExtractorDescriptor::new(
        "KinoPoiskIE",
        "KinoPoisk",
        r#"https?://(?:www\.)?kinopoisk\.ru/film/(?P<id>\d+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(KinopoiskHandler);
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://www.kinopoisk.ru/film/81041/watch/", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("81041"));
    assert_eq!(result.get_str("title"), Some("Native KinoPoisk title"));
    assert_eq!(
        result.get_str("description"),
        Some("Native KinoPoisk description")
    );
    assert_eq!(result.get_str("thumbnail"), Some("https://cdn.example/kinopoisk-cover.jpg"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(4533)));
    assert_eq!(result.get("age_limit"), Some(&serde_json::json!(12)));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/kinopoisk.m3u8")
    );
    assert_eq!(result.get_str("ext"), Some("mp4"));
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
