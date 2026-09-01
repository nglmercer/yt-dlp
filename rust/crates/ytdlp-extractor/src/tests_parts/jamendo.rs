struct JamendoHandler;

impl RequestHandler for JamendoHandler {
    fn name(&self) -> &str {
        "jamendo-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        let url = request.url();
        if request.method() == "HEAD" {
            let mut response = Response::new(url, 200, "OK", Vec::new());
            response.headers_mut().add("Content-Type", "image/webp");
            return Ok(response);
        }
        if !url.contains("X-Jam-Call") && request.headers().get("X-Jam-Call").is_none() {
            return Err(RequestError::new(
                ErrorKind::Transport,
                "Jamendo API request has no signed header",
            ));
        }
        let body = if url.contains("/api/tracks") {
            r#"[{"id":196219,"name":"Stories from Emona I","artistId":17,"albumId":29279,"description":"Native Jamendo description","duration":210,"dateCreated":1217438117,"licenseCC":["by","nc","nd"],"cover":{"album":{"size300":"https://cdn.example/cover.webp"}},"stats":{"listenedAll":1234,"favorited":56,"averageNote":4},"tags":[{"name":"piano"},{"name":"peaceful"}]}]"#
        } else if url.contains("/api/artists") {
            r#"[{"name":"Maya Filipič"}]"#
        } else if url.contains("/api/albums") {
            r#"[{"id":121486,"name":"Duck On Cover","description":{"en":"<p>Native album description</p>"},"tracks":[{"id":1032333},{"id":1032330}]}]"#
        } else {
            return Err(RequestError::new(
                ErrorKind::Transport,
                format!("no Jamendo route for {url}"),
            ));
        };
        Ok(Response::new(url, 200, "OK", body.as_bytes().to_vec()))
    }
}

fn jamendo_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(JamendoHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

#[test]
fn jamendo_sha1_and_track_native_extractor_match_source_contract() {
    assert_eq!(
        jamendo_sha1_hex(b"abc"),
        "a9993e364706816aba3e25717850c26c9cd0d89d"
    );
    let extractor = JamendoExtractor::new(ExtractorDescriptor::new(
        "JamendoIE",
        "Jamendo",
        r#"https?://(?:licensing\.jamendo\.com/[^/]+|(?:www\.)?jamendo\.com)/track/(?P<id>[0-9]+)(?:/(?P<display_id>[^/?#&]+))?"#,
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://www.jamendo.com/track/196219/stories-from-emona-i",
            &jamendo_context(),
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("196219"));
    assert_eq!(result.get_str("display_id"), Some("stories-from-emona-i"));
    assert_eq!(result.get_str("title"), Some("Stories from Emona I"));
    assert_eq!(result.get_str("artist"), Some("Maya Filipič"));
    assert_eq!(result.get_str("album"), Some("Duck On Cover"));
    assert_eq!(result.get_str("track"), Some("Stories from Emona I"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(210)));
    assert_eq!(result.get("timestamp"), Some(&serde_json::json!(1217438117)));
    assert_eq!(result.get_str("upload_date"), Some("20080730"));
    assert_eq!(result.get_str("license"), Some("by-nc-nd"));
    assert_eq!(result.get("tags"), Some(&serde_json::json!(["piano", "peaceful"])));
    assert_eq!(
        result
            .get("thumbnails")
            .and_then(serde_json::Value::as_array)
            .and_then(|thumbnails| thumbnails.first())
            .and_then(|thumbnail| thumbnail.get("ext")),
        Some(&serde_json::json!("webp"))
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(4)
    );
}

#[test]
fn jamendo_album_native_extractor_builds_transparent_track_entries() {
    let extractor = JamendoAlbumExtractor::new(ExtractorDescriptor::new(
        "JamendoAlbumIE",
        "JamendoAlbum",
        r#"https?://(?:www\.)?jamendo\.com/album/(?P<id>[0-9]+)"#,
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context("https://www.jamendo.com/album/121486/duck-on-cover", &jamendo_context())
        .unwrap();
    let ExtractorResult::Playlist { info, entries } = result else {
        panic!("expected Jamendo album playlist");
    };
    assert_eq!(info.get_str("id"), Some("121486"));
    assert_eq!(info.get_str("title"), Some("Duck On Cover"));
    assert_eq!(info.get_str("description"), Some("Native album description"));
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get_str("_type"), Some("url_transparent"));
    assert_eq!(entries[0].get_str("ie_key"), Some("Jamendo"));
    assert_eq!(entries[0].get_str("url"), Some("https://www.jamendo.com/track/1032333"));
    assert_eq!(entries[0].get_str("album"), Some("Duck On Cover"));
}
