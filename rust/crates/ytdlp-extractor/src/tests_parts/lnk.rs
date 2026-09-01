struct LnkHandler;

impl RequestHandler for LnkHandler {
    fn name(&self) -> &str {
        "lnk-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        if request
            .url()
            .starts_with("https://lnk.lt/api/video/video-config/79791")
        {
            let body = serde_json::json!({
                "videoInfo": {
                    "title": "Native LNK report",
                    "description": "Native LNK description",
                    "viewsCount": 1234,
                    "duration": 233,
                    "airDate": "2019-11-23",
                    "posterImage": "native-poster.jpg",
                    "episodeNumber": 13431,
                    "programTitle": "Native LNK series",
                    "videoUrl": "https://cdn.example/lnk/master.m3u8",
                    "videoFairplayUrl": "https://cdn.example/lnk/fairplay.m3u8",
                    "drm": false,
                    "subtitleUrl": "https://cdn.example/lnk/native.vtt"
                }
            });
            return Ok(Response::new(
                request.url(),
                200,
                "OK",
                serde_json::to_vec(&body).unwrap(),
            ));
        }
        Err(RequestError::new(
            ErrorKind::Transport,
            format!("no LNK route for {}", request.url()),
        ))
    }
}

fn lnk_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(LnkHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

#[test]
fn lnk_native_extractor_maps_video_config_and_hls_sources() {
    let extractor = LnkExtractor::new(ExtractorDescriptor::new(
        "LnkIE",
        "Lnk",
        r#"https?://(?:www\.)?lnk\.lt/[^/]+/(?P<id>\d+)"#,
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context("https://lnk.lt/zinios/79791", &lnk_context())
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("79791"));
    assert_eq!(result.get_str("title"), Some("Native LNK report"));
    assert_eq!(result.get_str("description"), Some("Native LNK description"));
    assert_eq!(result.get_i64("view_count"), Some(1234));
    assert_eq!(result.get_f64("duration"), Some(233.0));
    assert_eq!(result.get_str("upload_date"), Some("20191123"));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://lnk.lt/all-images/native-poster.jpg")
    );
    assert_eq!(result.get_i64("episode_number"), Some(13431));
    assert_eq!(result.get_str("series"), Some("Native LNK series"));
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(formats[0].get("format_id"), Some(&serde_json::json!("hls")));
    assert_eq!(
        formats[1].get("format_id"),
        Some(&serde_json::json!("fairplay"))
    );
    assert_eq!(
        result
            .get("subtitles")
            .and_then(|subtitles| subtitles.get("lt"))
            .and_then(serde_json::Value::as_array)
            .and_then(|tracks| tracks.first())
            .and_then(|track| track.get("url")),
        Some(&serde_json::json!("https://cdn.example/lnk/native.vtt"))
    );
}
