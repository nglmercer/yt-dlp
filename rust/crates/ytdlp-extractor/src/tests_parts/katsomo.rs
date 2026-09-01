struct KatsomoHandler;

impl RequestHandler for KatsomoHandler {
    fn name(&self) -> &str {
        "katsomo-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        let url = request.url();
        if url.contains("/api/web/asset/1181321.json") {
            let body = serde_json::json!({
                "asset": {
                    "title": "Fallback title",
                    "subtitle": "Native Katsomo title",
                    "description": "<p>Native Katsomo description.</p>",
                    "imageVersions": {
                        "small": {"@type": "small", "url": "https://cdn.example/katsomo-small.jpg"},
                        "large": {"@type": "large", "url": "https://cdn.example/katsomo-large.jpg"}
                    },
                    "createTime": "2019-11-30T10:21:24Z",
                    "accurateDuration": "37.12",
                    "duration": "38",
                    "views": "42",
                    "keywords": "News, Finland",
                    "live": false
                }
            });
            return Ok(Response::new(
                url,
                200,
                "OK",
                serde_json::to_vec(&body).unwrap(),
            ));
        }
        if url.contains("protocol=HLS") {
            let body = serde_json::json!({
                "playback": {
                    "drmProtected": false,
                    "items": {"item": [{
                        "url": "https://cdn.example/katsomo/master.m3u8",
                        "mediaFormat": "HLS"
                    }]}
                }
            });
            return Ok(Response::new(
                url,
                200,
                "OK",
                serde_json::to_vec(&body).unwrap(),
            ));
        }
        if url.contains("protocol=MPD") {
            let body = serde_json::json!({
                "playback": {
                    "drmProtected": false,
                    "items": {"item": {
                        "url": "https://cdn.example/katsomo/manifest.mpd",
                        "mediaFormat": "DASH",
                        "bitrate": 1800,
                        "fileSize": 123456
                    }}
                }
            });
            return Ok(Response::new(
                url,
                200,
                "OK",
                serde_json::to_vec(&body).unwrap(),
            ));
        }
        Err(RequestError::new(
            ErrorKind::Transport,
            format!("no Katsomo route for {url}"),
        ))
    }
}

fn katsomo_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(KatsomoHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

#[test]
fn katsomo_native_extractor_maps_asset_playback_and_metadata() {
    let extractor = KatsomoExtractor::new(ExtractorDescriptor::new(
        "KatsomoIE",
        "Katsomo",
        r#"https?://(?:www\.)?(?:katsomo|mtv(uutiset)?)\.fi/(?:sarja/[0-9a-z-]+-\d+/[0-9a-z-]+-|(?:#!/)?jakso/(?:\d+/[^/]+/)?|video/prog)(?P<id>\d+)"#,
        false,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://www.mtv.fi/sarja/mtv-uutiset-live-33001002003/native-katsomo-1181321",
            &katsomo_context(),
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("1181321"));
    assert_eq!(result.get_str("title"), Some("Native Katsomo title"));
    assert_eq!(
        result.get_str("description"),
        Some("Native Katsomo description.")
    );
    assert_eq!(result.get_f64("duration"), Some(37.12));
    assert_eq!(result.get_i64("view_count"), Some(42));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/katsomo/master.m3u8"));
    assert_eq!(result.get_str("ext"), Some("mp4"));
    assert_eq!(result.get("categories"), Some(&serde_json::json!(["News", "Finland"])));
    assert_eq!(result.get_bool("is_live"), Some(false));
    assert_eq!(
        result
            .get("thumbnails")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(formats[0].get("protocol"), Some(&serde_json::json!("m3u8_native")));
    assert_eq!(formats[1].get("protocol"), Some(&serde_json::json!("http_dash_segments")));
}
