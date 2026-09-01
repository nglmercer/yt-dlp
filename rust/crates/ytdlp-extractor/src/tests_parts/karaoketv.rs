struct KaraoketvHandler;

impl RequestHandler for KaraoketvHandler {
    fn name(&self) -> &str {
        "karaoketv-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        let url = request.url();
        let body = if url.contains("karaoketv.co.il") {
            "<meta property=\"og:title\" content=\"קריוקי של איזון\">
                <iframe src=\"https://www.karaoke.co.il/api_play.php?id=native\"></iframe>"
                .as_bytes()
                .to_vec()
        } else if url.contains("karaoke.co.il/api_play.php") {
            br#"<iframe src="https://www.video-cdn.com/embed/iframe/native-player"></iframe>"#
                .to_vec()
        } else if url.contains("video-cdn.com/embed/iframe/") {
            br#"var options = {clip: {url: 'mp4:native/58356.flv'}};
                var settings = {servers: ['wowzail.video-cdn.com:80/vodcdn',
                    'rtmps://backup.example/vodcdn']};"#
                .to_vec()
        } else {
            return Err(RequestError::new(
                ErrorKind::Transport,
                format!("no Karaoketv route for {url}"),
            ));
        };
        Ok(Response::new(url, 200, "OK", body))
    }
}

fn karaoketv_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(KaraoketvHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

#[test]
fn karaoketv_native_extractor_maps_rtmp_servers_and_player_data() {
    let extractor = KaraoketvExtractor::new(ExtractorDescriptor::new(
        "KaraoketvIE",
        "Karaoketv",
        r#"https?://(?:www\.)?karaoketv\.co\.il/[^/]+/(?P<id>\d+)"#,
        false,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "http://www.karaoketv.co.il/%D7%A9%D7%99%D7%A8%D7%99_%D7%A7%D7%A8%D7%99%D7%95%D7%A7%D7%99/58356/%D7%90%D7%99%D7%96%D7%95%D7%9F",
            &karaoketv_context(),
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("58356"));
    assert_eq!(result.get_str("title"), Some("קריוקי של איזון"));
    assert_eq!(result.get_str("ext"), Some("flv"));
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(
        formats[0].get("url"),
        Some(&serde_json::json!("rtmp://wowzail.video-cdn.com:80/vodcdn"))
    );
    assert_eq!(
        formats[0].get("play_path"),
        Some(&serde_json::json!("mp4:native/58356.flv"))
    );
    assert_eq!(
        formats[1].get("url"),
        Some(&serde_json::json!("rtmps://backup.example/vodcdn"))
    );
}
