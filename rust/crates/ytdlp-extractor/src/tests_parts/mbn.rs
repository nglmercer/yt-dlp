struct MbnHandler;

impl RequestHandler for MbnHandler {
    fn name(&self) -> &str {
        "mbn-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        let url = request.url();
        let body = if url.contains("mbn.co.kr/vod/programContents/previewlist/861/5433/1276155") {
            br#"<script>var state = "?content_cls_cd=42&content_id=1276155&";</script>"#.to_vec()
        } else if url.contains("mbnVodPlayer_2020.mbn") {
            br#"{
                "movie_title": "Native MBN episode",
                "play_sec": 3891,
                "bcast_date": "2021.07.03",
                "movie_start_Img": "https://img.example/mbn/native.jpg",
                "prog_nm": "Native MBN series",
                "ad_contentnumber": "19",
                "movie_list": [
                    {"url": "https://stream.example/video/chunklist_pd720.m3u8"},
                    {"url": "https://stream.example/video/playlist.m3u8"}
                ]
            }"#
            .to_vec()
        } else if url.contains("mbnStreamAuth_new_vod.mbn") {
            if url.contains("chunklist_pd720") {
                b"https://cdn.example/mbn/native-720.m3u8\n".to_vec()
            } else {
                b"https://cdn.example/mbn/native-auto.m3u8\n".to_vec()
            }
        } else {
            return Err(RequestError::new(
                ErrorKind::Transport,
                format!("no MBN route for {url}"),
            ));
        };
        Ok(Response::new(request.url(), 200, "OK", body))
    }
}

fn mbn_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(MbnHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

#[test]
fn mbn_native_extractor_maps_authenticated_hls_and_metadata() {
    let extractor = MbnExtractor::new(ExtractorDescriptor::new(
        "MBNIE",
        "MBN",
        r#"https?://(?:www\.)?mbn\.co\.kr/vod/programContents/preview(?:list)?/\d+/\d+/(?P<id>\d+)"#,
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://mbn.co.kr/vod/programContents/previewlist/861/5433/1276155",
            &mbn_context(),
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("1276155"));
    assert_eq!(result.get_str("title"), Some("Native MBN episode"));
    assert_eq!(result.get_i64("duration"), Some(3891));
    assert_eq!(result.get_str("release_date"), Some("20210703"));
    assert_eq!(result.get_str("series"), Some("Native MBN series"));
    assert_eq!(result.get_i64("episode_number"), Some(19));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://img.example/mbn/native.jpg")
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/mbn/native-auto.m3u8")
    );
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(formats[0].get("format_id"), Some(&serde_json::json!("hls-0")));
    assert_eq!(formats[1].get("protocol"), Some(&serde_json::json!("m3u8_native")));
}
