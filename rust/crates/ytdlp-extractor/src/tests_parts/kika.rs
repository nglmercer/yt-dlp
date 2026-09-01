struct KikaHandler;

impl RequestHandler for KikaHandler {
    fn name(&self) -> &str {
        "kika-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        let url = request.url();
        let body = if url.contains("/_next-api/proxy/v1/videos/kika-100") {
            r#"{"assets":{"url":"https://api.example/kika/assets/100"},"hasSubtitle":true,"title":"Native KiKA video","description":"Native KiKA description","date":"2024-01-02T03:04:05Z","modificationDate":"2024-01-03T04:05:06Z","durationInSeconds":436,"episodeNumber":7,"season":1}"#
        } else if url.contains("/kika/assets/100") {
            r#"{"assets":[{"url":"https://cdn.example/kika-100.mp4","frameWidth":1920,"frameHeight":1080,"fileSize":123456,"bitrateAudio":128,"bitrateVideo":2500},{"url":"https://cdn.example/kika-100.m3u8","frameWidth":1920,"frameHeight":1080,"fileSize":0,"bitrateAudio":-1,"bitrateVideo":-1}],"videoSubtitle":"https://cdn.example/kika-100.ttml","webvttUrl":"https://cdn.example/kika-100.vtt"}"#
        } else if url.contains("/_next-api/proxy/v1/brands/brand-562") {
            r#"{"title":"Native KiKA brand","description":"Native brand description","videoSubchannel":{"videosPageUrl":"https://api.example/kika/page/1"}}"#
        } else if url.contains("/kika/page/1") {
            r#"{"content":[{"api":{"url":"https://api.example/kika/videos/kika-100"},"id":"kika-100","title":"First KiKA entry","duration":436,"date":"2024-01-02T03:04:05Z"}],"links":{"next":"https://api.example/kika/page/2"}}"#
        } else if url.contains("/kika/page/2") {
            r#"{"content":[{"api":{"url":"https://api.example/kika/videos/kika-200"},"id":"kika-200","title":"Second KiKA entry","duration":1574,"date":"2024-01-04T03:04:05Z"}],"links":{}}"#
        } else {
            return Err(RequestError::new(
                ErrorKind::Transport,
                format!("no KiKA route for {url}"),
            ));
        };
        Ok(Response::new(url, 200, "OK", body.as_bytes().to_vec()))
    }
}

fn kika_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(KikaHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

#[test]
fn kika_native_extractor_maps_assets_subtitles_and_metadata() {
    let extractor = KikaExtractor::new(ExtractorDescriptor::new(
        "KikaIE",
        "Kika",
        r#"https?://(?:www\.)?kika\.de/[\w/-]+/videos/(?P<id>[a-z-]+\d+)"#,
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://www.kika.de/logo/videos/kika-100",
            &kika_context(),
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("kika-100"));
    assert_eq!(result.get_str("title"), Some("Native KiKA video"));
    assert_eq!(result.get_str("description"), Some("Native KiKA description"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(436)));
    assert_eq!(result.get("episode_number"), Some(&serde_json::json!(7)));
    assert_eq!(result.get("season_number"), Some(&serde_json::json!(1)));
    assert_eq!(result.get("timestamp"), Some(&serde_json::json!(1704164645)));
    assert_eq!(result.get("modified_timestamp"), Some(&serde_json::json!(1704254706)));
    assert_eq!(result.get_str("ext"), Some("mp4"));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/kika-100.mp4")
    );
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(formats[0].get("width"), Some(&serde_json::json!(1920)));
    assert_eq!(formats[0].get("height"), Some(&serde_json::json!(1080)));
    assert_eq!(formats[0].get("filesize"), Some(&serde_json::json!(123456)));
    assert_eq!(formats[1].get("format_id"), Some(&serde_json::json!("hls")));
    assert_eq!(formats[1].get("protocol"), Some(&serde_json::json!("m3u8_native")));
    assert_eq!(
        result.get("subtitles"),
        Some(&serde_json::json!({
            "de": [
                {"url": "https://cdn.example/kika-100.ttml", "ext": "ttml"},
                {"url": "https://cdn.example/kika-100.vtt", "ext": "vtt"}
            ]
        }))
    );
}

#[test]
fn kika_playlist_native_extractor_follows_pages_and_builds_transparent_entries() {
    let extractor = KikaPlaylistExtractor::new(ExtractorDescriptor::new(
        "KikaPlaylistIE",
        "KikaPlaylist",
        r#"https?://(?:www\.)?kika\.de/[\w-]+/(?P<id>[a-z-]+\d+)"#,
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://www.kika.de/logo/brand-562",
            &kika_context(),
        )
        .unwrap();
    let ExtractorResult::Playlist { info, entries } = result else {
        panic!("expected KiKA playlist");
    };

    assert_eq!(info.get_str("id"), Some("brand-562"));
    assert_eq!(info.get_str("title"), Some("Native KiKA brand"));
    assert_eq!(info.get_str("description"), Some("Native brand description"));
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get_str("ie_key"), Some("Kika"));
    assert_eq!(entries[0].get_str("id"), Some("kika-100"));
    assert_eq!(entries[0].get_str("title"), Some("First KiKA entry"));
    assert_eq!(entries[0].get("duration"), Some(&serde_json::json!(436)));
    assert_eq!(
        entries[1].get_str("url"),
        Some("https://api.example/kika/videos/kika-200")
    );
}
