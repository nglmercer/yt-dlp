struct Kenh14Handler;

impl RequestHandler for Kenh14Handler {
    fn name(&self) -> &str {
        "kenh14-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        let url = request.url();
        let body = if url.contains("api.kinghub.vn/video/api/v1/detailVideoByGet") {
            r#"{"title":"Native Kenh14 title","description":"Native Kenh14 description","duration":"722.86","author":"Native uploader","uploadtime":"2022-05-17 10:00:00","views":"123"}"#
        } else if url.ends_with("native/video.mp4.json") {
            r#"{"hls":"https://cdn.example/kenh14.m3u8","mpd":"https://cdn.example/kenh14.mpd"}"#
        } else if url.contains("/playlist/") {
            r#"<html><head><meta property="og:image" content="https://cdn.example/playlist.png?size=1"></head><body>
                <div class="category-detail"><div class="name">Native Kenh14 playlist</div>
                <div class="description">Native playlist description</div></div>
                <div class="video-item" data-id="101"></div><div class="video-item" data-id="202"></div>
            </body></html>"#
        } else if url.contains("video.kenh14.vn") {
            r#"<html><head>
                <meta property="og:image" content="https://cdn.example/kenh14.jpg">
                <meta property="og:title" content="Fallback title">
                <meta name="keywords" content="news; Vietnam; ">
            </head><body>
                <div type="VideoStream" data-vid="kenh14cdn.com/native/video.mp4"
                     data-thumb="https://cdn.example/stream-thumb.jpg"></div>
            </body></html>"#
        } else {
            return Err(RequestError::new(
                ErrorKind::Transport,
                format!("no Kenh14 route for {url}"),
            ));
        };
        Ok(Response::new(url, 200, "OK", body.as_bytes().to_vec()))
    }
}

fn kenh14_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(Kenh14Handler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

#[test]
fn kenh14_video_native_extractor_maps_direct_and_manifest_formats() {
    let extractor = Kenh14VideoExtractor::new(ExtractorDescriptor::new(
        "Kenh14VideoIE",
        "Kenh14Video",
        r#"https?://video\.kenh14\.vn/(?:video/)?[\w-]+-(?P<id>[0-9]+)\.chn"#,
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://video.kenh14.vn/video/native-title-316173.chn",
            &kenh14_context(),
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("316173"));
    assert_eq!(result.get_str("title"), Some("Native Kenh14 title"));
    assert_eq!(result.get_str("description"), Some("Native Kenh14 description"));
    assert_eq!(result.get_str("uploader"), Some("Native uploader"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(722.86)));
    assert_eq!(result.get_str("upload_date"), Some("20220517"));
    assert_eq!(result.get_str("thumbnail"), Some("https://cdn.example/kenh14.jpg"));
    assert_eq!(result.get("tags"), Some(&serde_json::json!(["news", "Vietnam"])));
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 3);
    assert_eq!(formats[0].get("format_id"), Some(&serde_json::json!("http")));
    assert_eq!(formats[1].get("protocol"), Some(&serde_json::json!("m3u8_native")));
    assert_eq!(
        formats[2].get("protocol"),
        Some(&serde_json::json!("http_dash_segments"))
    );
    assert_eq!(result.get_str("ext"), Some("mp4"));
}

#[test]
fn kenh14_playlist_native_extractor_builds_video_entries() {
    let extractor = Kenh14PlaylistExtractor::new(ExtractorDescriptor::new(
        "Kenh14PlaylistIE",
        "Kenh14Playlist",
        r#"https?://video\.kenh14\.vn/playlist/[\w-]+-(?P<id>[0-9]+)\.chn"#,
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://video.kenh14.vn/playlist/native-71.chn",
            &kenh14_context(),
        )
        .unwrap();
    let ExtractorResult::Playlist { info, entries } = result else {
        panic!("expected Kenh14 playlist");
    };
    assert_eq!(info.get_str("id"), Some("71"));
    assert_eq!(info.get_str("title"), Some("Native Kenh14 playlist"));
    assert_eq!(info.get_str("description"), Some("Native playlist description"));
    assert_eq!(
        info.get_str("thumbnail"),
        Some("https://cdn.example/playlist.png")
    );
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get_str("ie_key"), Some("Kenh14Video"));
    assert_eq!(
        entries[1].get_str("url"),
        Some("https://video.kenh14.vn/video/video-202.chn")
    );
}
