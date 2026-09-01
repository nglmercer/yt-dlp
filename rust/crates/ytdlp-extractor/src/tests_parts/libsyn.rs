struct LibsynHandler;

impl RequestHandler for LibsynHandler {
    fn name(&self) -> &str {
        "libsyn-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        if !request
            .url()
            .contains("html5-player.libsyn.com/embed/episode/id/6385796")
        {
            return Err(RequestError::new(
                ErrorKind::Transport,
                format!("no Libsyn route for {}", request.url()),
            ));
        }
        let webpage = r#"
            <script>
                var playlistItem = {
                    "item_title": "Native growth episode",
                    "thumbnail_url": "https://assets.example/libsyn/native.jpg",
                    "release_date": "2024-03-20",
                    "duration": 834,
                    "media_url_libsyn": "https://cdn.example/libsyn/native-libsyn.mp3",
                    "media_url": "https://cdn.example/libsyn/native-main.mp3",
                    "download_link": "https://cdn.example/libsyn/native-download.mp3"
                };
            </script>
            <h3>Native podcast</h3>
            <p id="info_text_body">Native&nbsp;description</p>
        "#;
        Ok(Response::new(
            request.url(),
            200,
            "OK",
            webpage.as_bytes().to_vec(),
        ))
    }
}

fn libsyn_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(LibsynHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

#[test]
fn libsyn_native_extractor_maps_playlist_item_and_audio_formats() {
    let extractor = LibsynExtractor::new(ExtractorDescriptor::new(
        "LibsynIE",
        "Libsyn",
        r#"(?P<mainurl>https?://html5-player\.libsyn\.com/embed/episode/id/(?P<id>[0-9]+))"#,
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://html5-player.libsyn.com/embed/episode/id/6385796/",
            &libsyn_context(),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_str("id"), Some("6385796"));
    assert_eq!(
        result.get_str("title"),
        Some("Native podcast - Native growth episode")
    );
    assert_eq!(result.get_str("description"), Some("Native description"));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://assets.example/libsyn/native.jpg")
    );
    assert_eq!(result.get_str("upload_date"), Some("20240320"));
    assert_eq!(result.get_f64("duration"), Some(834.0));
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 3);
    assert_eq!(formats[0].get("format_id"), Some(&serde_json::json!("libsyn")));
    assert_eq!(
        formats[2].get("format_id"),
        Some(&serde_json::json!("download"))
    );
}
