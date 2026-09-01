struct LocipoHandler;

impl RequestHandler for LocipoHandler {
    fn name(&self) -> &str {
        "locipo-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        if request
            .url()
            .contains("web-api.locipo.jp/creatives/fb5ffeaa-398d-45ce-bb49-0e221b5f94f1")
        {
            return Ok(Response::new(
                request.url(),
                200,
                "OK",
                br#"{
                    "media_id": "streaks-media",
                    "name": "Native Locipo creative",
                    "description": "<p>Native Locipo description</p>",
                    "publication_started_at": "2024-01-03T00:00:00Z",
                    "keyword": "tag1, tag2",
                    "company": {"name": "Native company"},
                    "series": {"id": 42, "name": "Native series"}
                }"#.to_vec(),
            ));
        }
        if request
            .url()
            .contains("/creative/fb5ffeaa-398d-45ce-bb49-0e221b5f94f1")
        {
            return Ok(Response::new(
                request.url(),
                200,
                "OK",
                br#"<script>window.__NUXT__.config={"public":{"streaksVodPlaybackApiKey":"native-key"}};</script>"#.to_vec(),
            ));
        }
        if request
            .url()
            .contains("playback.api.streaks.jp/v1/projects/locipo-prod/medias/streaks-media")
        {
            return Ok(Response::new(
                request.url(),
                200,
                "OK",
                br#"{
                    "id": "streaks-id",
                    "type": "file",
                    "sources": [{"id": "source-1", "src": "https://cdn.example/locipo/master.m3u8", "type": "application/x-mpegURL"}],
                    "tracks": [{"kind": "captions", "src": "https://cdn.example/locipo/captions.vtt", "srclang": "ja"}],
                    "name": "Native Streaks name",
                    "description": "Native Streaks description",
                    "duration": 123.5,
                    "created_at": "2024-01-02T03:04:05Z",
                    "updated_at": "2024-01-02T04:04:05Z",
                    "tags": [{"name": "streaks-tag"}],
                    "thumbnail": {"src": "https://cdn.example/locipo/poster.jpg"}
                }"#.to_vec(),
            ));
        }
        if request
            .url()
            .contains(
                "web-api.locipo.jp/playlists/35d3dd2b-531d-4824-8575-b1c527d29538/creatives",
            )
        {
            let second_page = request.url().contains("offset=100");
            let body = if second_page {
                serde_json::json!({
                    "total": 101,
                    "items": [{"id": "creative-2"}]
                })
            } else {
                serde_json::json!({
                    "total": 101,
                    "items": [{
                        "id": "creative-1",
                        "playlist": {
                            "name": "Native playlist",
                            "description": "Native playlist description"
                        }
                    }]
                })
            };
            return Ok(Response::new(
                request.url(),
                200,
                "OK",
                serde_json::to_vec(&body).unwrap(),
            ));
        }
        Err(RequestError::new(
            ErrorKind::Transport,
            format!("no Locipo route for {}", request.url()),
        ))
    }
}

fn locipo_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(LocipoHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

#[test]
fn locipo_native_extractor_maps_creative_and_streaks_playback() {
    let extractor = LocipoExtractor::new(ExtractorDescriptor::with_valid_urls(
        "LocipoIE",
        "Locipo",
        vec![
            r"https?://locipo\.jp/creative/(?P<id>[\da-f]{8}(?:-[\da-f]{4}){3}-[\da-f]{12})"
                .to_owned(),
            r"https?://locipo\.jp/embed/?\?(?:[^#]+&)?id=(?P<id>[\da-f]{8}(?:-[\da-f]{4}){3}-[\da-f]{12})"
                .to_owned(),
        ],
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://locipo.jp/creative/fb5ffeaa-398d-45ce-bb49-0e221b5f94f1",
            &locipo_context(),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(
        result.get_str("id"),
        Some("fb5ffeaa-398d-45ce-bb49-0e221b5f94f1")
    );
    assert_eq!(result.get_str("display_id"), Some("streaks-media"));
    assert_eq!(result.get_str("title"), Some("Native Locipo creative"));
    assert_eq!(
        result.get_str("description"),
        Some("Native Locipo description")
    );
    assert_eq!(result.get_f64("duration"), Some(123.5));
    assert_eq!(result.get_str("live_status"), Some("not_live"));
    assert_eq!(result.get_i64("timestamp"), Some(1_704_164_645));
    assert_eq!(result.get_str("uploader"), Some("Native company"));
    assert_eq!(result.get_str("series"), Some("Native series"));
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 1);
    assert_eq!(
        formats[0].get("protocol"),
        Some(&serde_json::json!("m3u8_native"))
    );
    assert_eq!(
        result
            .get("subtitles")
            .and_then(|subtitles| subtitles.get("ja"))
            .and_then(serde_json::Value::as_array)
            .and_then(|tracks| tracks.first())
            .and_then(|track| track.get("url")),
        Some(&serde_json::json!(
            "https://cdn.example/locipo/captions.vtt"
        ))
    );
}

#[test]
fn locipo_playlist_native_extractor_fetches_all_pages() {
    let extractor = LocipoPlaylistExtractor::new(ExtractorDescriptor::with_valid_urls(
        "LocipoPlaylistIE",
        "LocipoPlaylist",
        vec![
            r"https?://locipo\.jp/(?P<type>playlist)/(?P<id>[\da-f]{8}(?:-[\da-f]{4}){3}-[\da-f]{12})"
                .to_owned(),
            r"https?://locipo\.jp/(?P<type>series)/(?P<id>\d+)"
                .to_owned(),
        ],
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://locipo.jp/playlist/35d3dd2b-531d-4824-8575-b1c527d29538",
            &locipo_context(),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(
        result.get_str("id"),
        Some("35d3dd2b-531d-4824-8575-b1c527d29538")
    );
    assert_eq!(result.get_str("title"), Some("Native playlist"));
    assert_eq!(
        result.get_str("description"),
        Some("Native playlist description")
    );
    let entries = result
        .get("entries")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(
        entries[0].get("url"),
        Some(&serde_json::json!("https://locipo.jp/creative/creative-1"))
    );
    assert_eq!(
        entries[0].get("ie_key"),
        Some(&serde_json::json!("Locipo"))
    );
}

#[test]
fn locipo_creative_list_url_redirects_to_native_playlist() {
    let extractor = LocipoExtractor::new(ExtractorDescriptor::new(
        "LocipoIE",
        "Locipo",
        r"https?://locipo\.jp/creative/(?P<id>[\da-f]{8}(?:-[\da-f]{4}){3}-[\da-f]{12})",
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://locipo.jp/creative/fb5ffeaa-398d-45ce-bb49-0e221b5f94f1?list=native-playlist",
            &locipo_context(),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(
        result.get_str("url"),
        Some("https://locipo.jp/playlist/native-playlist")
    );
    assert_eq!(result.get_str("ie_key"), Some("LocipoPlaylist"));
}
