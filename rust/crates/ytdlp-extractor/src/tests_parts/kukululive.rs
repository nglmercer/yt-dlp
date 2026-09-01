struct KukuluLiveHandler;

impl RequestHandler for KukuluLiveHandler {
    fn name(&self) -> &str {
        "kukulukive-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        let url = request.url();
        let body = if url.contains("live.php?h100") {
            r#"<html><head><meta name="Description" content="Native live description"><meta property="og:image" content="https://cdn.example/kukulu-live.jpg"></head><body><span id="livetitle">Native live title</span><script>var timeshift = false;</script></body></html>"#
        } else if url.contains("live.php?h200") {
            r#"<html><head><meta name="Description" content="Native VOD description"><meta property="og:image" content="https://cdn.example/kukulu-vod.jpg"></head><body><span id="livetitle">Native VOD title</span><script>var timeshift = true;</script></body></html>"#
        } else if url.contains("action=getZliveByAjax") && url.contains("force_h264=1") {
            "vcodec=H264&now_quality=high&hlsaddr=https%3A%2F%2Fcdn.example%2Fforced-high.m3u8&hlsaddr_audioonly=https%3A%2F%2Fcdn.example%2Fforced-audio.m3u8"
        } else if url.contains("action=getZliveByAjax") {
            "vcodec=HEVC&now_quality=high&hlsaddr=https%3A%2F%2Fcdn.example%2Fhevc-high.m3u8&hlsaddr_audioonly=https%3A%2F%2Fcdn.example%2Fhevc-audio.m3u8"
        } else if url.contains("action=getForceLowliveByAjax") {
            "vcodec=H264&now_quality=low&hlsaddr=https%3A%2F%2Fcdn.example%2Flow.m3u8"
        } else if url.contains("live.timeshift.fplayer.php") {
            r#"var fplayer_source = [{'file':'/vod/native-part-1.m3u8','time_start':1702689148},{'file':'/vod/native-part-2.m3u8','time_start':1702690148}];"#
        } else {
            return Err(RequestError::new(
                ErrorKind::Transport,
                format!("no KukuluLive route for {url}"),
            ));
        };
        Ok(Response::new(url, 200, "OK", body.as_bytes().to_vec()))
    }
}

fn kukulu_live_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(KukuluLiveHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

#[test]
fn kukulu_live_native_extractor_maps_quality_endpoints_and_metadata() {
    let extractor = KukuluLiveExtractor::new(ExtractorDescriptor::new(
        "KukuluLiveIE",
        "KukuluLive",
        r#"https?://live\.erinn\.biz/live\.php\?h(?P<id>\d+)"#,
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://live.erinn.biz/live.php?h100",
            &kukulu_live_context(),
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("100"));
    assert_eq!(result.get_str("title"), Some("Native live title"));
    assert_eq!(
        result.get_str("description"),
        Some("Native live description")
    );
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/kukulu-live.jpg")
    );
    assert_eq!(result.get("is_live"), Some(&serde_json::json!(true)));
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 5);
    assert_eq!(formats[0].get("vcodec"), Some(&serde_json::json!("HEVC")));
    assert_eq!(
        formats[2].get("url"),
        Some(&serde_json::json!("https://cdn.example/forced-high.m3u8"))
    );
    assert_eq!(
        formats[4].get("format_id"),
        Some(&serde_json::json!("low"))
    );
}

#[test]
fn kukulu_vod_native_extractor_builds_segment_playlist() {
    let extractor = KukuluLiveExtractor::new(ExtractorDescriptor::new(
        "KukuluLiveIE",
        "KukuluLive",
        r#"https?://live\.erinn\.biz/live\.php\?h(?P<id>\d+)"#,
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://live.erinn.biz/live.php?h200",
            &kukulu_live_context(),
        )
        .unwrap();
    let ExtractorResult::Playlist { info, entries } = result else {
        panic!("expected KukuluLive VOD playlist");
    };

    assert_eq!(info.get_str("id"), Some("200"));
    assert_eq!(info.get_str("title"), Some("Native VOD title"));
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get_str("id"), Some("200_1"));
    assert_eq!(
        entries[0].get_str("title"),
        Some("Native VOD title (Part 1)")
    );
    assert_eq!(entries[0].get("timestamp"), Some(&serde_json::json!(1702689148)));
    assert_eq!(
        entries[0].get_str("url"),
        Some("https://live.erinn.biz/vod/native-part-1.m3u8")
    );
    assert_eq!(
        entries[1]
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("protocol"))
            .and_then(serde_json::Value::as_str),
        Some("m3u8_native")
    );
}
