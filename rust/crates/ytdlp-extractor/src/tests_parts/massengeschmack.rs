struct MassengeschmackHandler;

impl RequestHandler for MassengeschmackHandler {
    fn name(&self) -> &str {
        "massengeschmack-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        let url = request.url();
        if url.contains("/play/fktv202") {
            let page = r#"
                <span id="clip-title">Native <strong>Fernsehkritik-TV #202</strong></span>
                POSTER = "https://cache.example/massengeschmack/poster.jpg";
                MEDIA = [
                    {"src": "//cdn.example/massengeschmack/master.m3u8", "type": "application/x-mpegURL"},
                    {"src": "https://cdn.example/massengeschmack/source.mp4", "type": "video/mp4"}
                ];
                <a href="//cdn.example/massengeschmack/download-1080.mp4">
                    <strong>Video 1080p</strong>
                    <small>1920x1080 (1,234 MiB)</small>
                </a>
                <a href="https://cdn.example/massengeschmack/download-audio.mp3">
                    <strong>Audio MP3</strong>
                    <small>(12 MiB)</small>
                </a>
            "#;
            return Ok(Response::new(url, 200, "OK", page.as_bytes().to_vec()));
        }
        Err(RequestError::new(
            ErrorKind::Transport,
            format!("no Massengeschmack route for {url}"),
        ))
    }
}

fn massengeschmack_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(MassengeschmackHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

#[test]
fn massengeschmack_native_extractor_maps_media_and_download_renditions() {
    let extractor = MassengeschmackExtractor::new(ExtractorDescriptor::new(
        "MassengeschmackTVIE",
        "massengeschmack.tv",
        r#"https?://(?:www\.)?massengeschmack\.tv/play/(?P<id>[^?&#]+)"#,
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://massengeschmack.tv/play/fktv202",
            &massengeschmack_context(),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_str("id"), Some("fktv202"));
    assert_eq!(
        result.get_str("title"),
        Some("Native Fernsehkritik-TV #202")
    );
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cache.example/massengeschmack/poster.jpg")
    );
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 4);
    assert_eq!(formats[0].get("protocol"), Some(&serde_json::json!("m3u8_native")));
    assert_eq!(formats[2].get("height"), Some(&serde_json::json!(1080)));
    assert_eq!(
        formats[2].get("filesize"),
        Some(&serde_json::json!(1_293_942_784_i64))
    );
    assert_eq!(formats[3].get("vcodec"), Some(&serde_json::json!("none")));
}
