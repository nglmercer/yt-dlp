#[test]
fn dropbox_native_extractor_decodes_prefetch_hls_and_original() {
    let extractor = DropboxExtractor::new(ExtractorDescriptor::new(
        "DropboxIE",
        "Dropbox",
        r"https?://(?:www\.)?dropbox\.com/(?:(?:e/)?scl/f[io]|sh?)/(?P<id>\w+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<script>
            registerStreamedPrefetch("token", "CmFub255bW91czoJYW5vbnltb3VzCmh0dHBzOi8vY2RuLmV4YW1wbGUvdmlkZW8vbWFzdGVyLm0zdTgKaHR0cHM6Ly93d3cuZHJvcGJveC5jb20vdGVtcF90aHVtYl9mcm9tX3Rva2VuL2FiYz94PTE=");
        </script>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.dropbox.com/s/nativeid/sample%20video.mp4?dl=0",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("nativeid"));
    assert_eq!(result.get_str("title"), Some("sample video"));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/video/master.m3u8"));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://www.dropbox.com/temp_thumb_from_token/abc?x=1")
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.get(1))
            .and_then(|format| format.get("url")),
        Some(&serde_json::json!(
            "https://www.dropbox.com/s/nativeid/sample%20video.mp4?dl=1"
        ))
    );
}

#[test]
fn dropbox_native_extractor_marks_password_shares_as_todo() {
    let extractor = DropboxExtractor::new(ExtractorDescriptor::new(
        "DropboxIE",
        "Dropbox",
        r"https?://(?:www\.)?dropbox\.com/(?:(?:e/)?scl/f[io]|sh?)/(?P<id>\w+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<script>
            registerStreamedPrefetch("token", "aHR0cHM6Ly93d3cuZHJvcGJveC5jb20vc20vcGFzc3dvcmQ/Y29udGVudF9pZD1uYXRpdmUtY29udGVudA==");
        </script>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context("https://www.dropbox.com/s/nativeid/video.mp4", &context)
        .unwrap_err();

    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}
