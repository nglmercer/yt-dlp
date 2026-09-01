struct LeHandler;

fn le_test_encrypt(plaintext: &[u8]) -> Vec<u8> {
    assert!(plaintext.len() >= 6);
    let mut rotated = Vec::with_capacity(plaintext.len() * 2);
    for byte in plaintext {
        rotated.push(byte >> 4);
        rotated.push(byte & 0x0f);
    }
    let split = rotated.len() - 11;
    let mut expanded = Vec::with_capacity(rotated.len());
    expanded.extend_from_slice(&rotated[rotated.len() - split..]);
    expanded.extend_from_slice(&rotated[..rotated.len() - split]);

    let mut encrypted = b"vc_01".to_vec();
    for pair in expanded.chunks_exact(2) {
        encrypted.push(pair[0] * 16 + pair[1]);
    }
    encrypted
}

impl RequestHandler for LeHandler {
    fn name(&self) -> &str {
        "le-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        let url = request.url();
        if url.contains("player-pc.le.com/mms/out/video/playJson") {
            let body = serde_json::json!({
                "msgs": {
                    "playstatus": {"status": 1, "flag": 0},
                    "playurl": {
                        "domain": ["https://media.example/"],
                        "title": "Native Le video",
                        "pic": "https://cdn.example/le.jpg",
                        "dispatch": {
                            "720p": ["nodes/720", "mp4"],
                            "480p": ["nodes/480", "mp4"]
                        }
                    }
                }
            });
            return Ok(Response::new(
                url,
                200,
                "OK",
                serde_json::to_vec(&body).unwrap(),
            ));
        }
        if url.contains("media.example/nodes/") {
            let height = if url.contains("/720") { "720" } else { "480" };
            let body = serde_json::json!({
                "nodelist": [{"location": format!("https://media.example/m3u8/{height}")}]
            });
            return Ok(Response::new(
                url,
                200,
                "OK",
                serde_json::to_vec(&body).unwrap(),
            ));
        }
        if url.contains("media.example/m3u8/") {
            let manifest = b"#EXTM3U\n#EXT-X-VERSION:3\n";
            return Ok(Response::new(
                url,
                200,
                "OK",
                le_test_encrypt(manifest),
            ));
        }
        if url.contains("/ptv/vplay/22005890.html") {
            let page = r#"
                <meta name="description" content="Native Le description">
                <span>发布时间&nbsp;2024-01-02 10:00 </span>
            "#;
            return Ok(Response::new(url, 200, "OK", page.as_bytes().to_vec()));
        }
        if url.contains("/tv/46177.html") {
            let page = r#"
                <meta name="keywords" content="美人天下，other keywords">
                <meta name="description" content="Native playlist description">
                <a href="http://www.letv.com/ptv/vplay/1415246.html">one</a>
                <a href="http://www.letv.com/ptv/vplay/1415246.html">duplicate</a>
                <a href="http://www.letv.com/ptv/vplay/22005890.html">two</a>
            "#;
            return Ok(Response::new(url, 200, "OK", page.as_bytes().to_vec()));
        }
        Err(RequestError::new(
            ErrorKind::Transport,
            format!("no Le route for {url}"),
        ))
    }
}

fn le_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(LeHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

fn le_extractor() -> LeExtractor {
    LeExtractor::new(ExtractorDescriptor::new(
        "LeIE",
        "Le",
        r#"https?://(?:www\.le\.com/ptv/vplay|(?:sports\.le|(?:www\.)?lesports)\.com/(?:match|video))/(?P<id>\d+)\.html"#,
        true,
    ))
    .unwrap()
}

#[test]
fn le_native_extractor_maps_legacy_play_json_and_decrypted_manifests() {
    let result = le_extractor()
        .extract_with_context(
            "http://www.le.com/ptv/vplay/22005890.html",
            &le_context(),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_str("id"), Some("22005890"));
    assert_eq!(result.get_str("title"), Some("Native Le video"));
    assert_eq!(result.get_str("description"), Some("Native Le description"));
    assert_eq!(result.get_str("thumbnail"), Some("https://cdn.example/le.jpg"));
    assert_eq!(result.get_i64("timestamp"), Some(1_704_160_800));

    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    for format in formats {
        assert!(format
            .get("url")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|url| url.starts_with("data:application/vnd.apple.mpegurl;base64,")));
        assert_eq!(format.get("protocol"), Some(&serde_json::json!("m3u8_native")));
        assert_eq!(format.get("quality"), None);
    }
    assert!(formats.iter().any(|format| {
        format.get("format_id") == Some(&serde_json::json!("hls-720p"))
            && format.get("height") == Some(&serde_json::json!(720))
    }));
    assert!(formats.iter().any(|format| {
        format.get("format_id") == Some(&serde_json::json!("hls-480p"))
            && format.get("height") == Some(&serde_json::json!(480))
    }));
}

#[test]
fn le_playlist_native_extractor_deduplicates_legacy_video_links() {
    let extractor = LePlaylistExtractor::new(ExtractorDescriptor::new(
        "LePlaylistIE",
        "LePlaylist",
        r#"https?://[a-z]+\.le\.com/(?!video)[a-z]+/(?P<id>[a-z0-9_]+)"#,
        true,
    ))
    .unwrap();
    let ExtractorResult::Playlist { info, entries } = extractor
        .extract_with_context("http://www.le.com/tv/46177.html", &le_context())
        .unwrap()
    else {
        panic!("expected Le playlist");
    };

    assert_eq!(info.get_str("id"), Some("46177"));
    assert_eq!(info.get_str("title"), Some("美人天下"));
    assert_eq!(
        info.get_str("description"),
        Some("Native playlist description")
    );
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get_str("ie_key"), Some("Le"));
    assert_eq!(entries[0].get_str("url"), Some("http://www.le.com/ptv/vplay/1415246.html"));
    assert_eq!(entries[1].get_str("ie_key"), Some("Le"));
    assert_eq!(entries[1].get_str("url"), Some("http://www.le.com/ptv/vplay/22005890.html"));
}
