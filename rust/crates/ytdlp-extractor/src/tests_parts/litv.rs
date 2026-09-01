struct LitvHandler;

impl RequestHandler for LitvHandler {
    fn name(&self) -> &str {
        "litv-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        let url = request.url();
        if url.contains("/drama/watch/VOD00040000") {
            let page = r#"<script id="__NEXT_DATA__">{
                "props": {
                    "pageProps": {
                        "programInformation": {
                            "content_id": "VOD00040000",
                            "content_type": "vod",
                            "title": "Native LiTV episode",
                            "secondary_mark": " HD",
                            "description": "Native LiTV description",
                            "picture": "images/native.jpg",
                            "episode": 7,
                            "assets": [{"asset_id": "ASSET-40000"}],
                            "genres": [{"name": "Drama"}, {"name": "Fantasy"}]
                        }
                    }
                }
            }</script>"#;
            return Ok(Response::new(url, 200, "OK", page.as_bytes().to_vec()));
        }
        if url.contains("/drama/watch/VOD00041610") {
            let page = r#"<script id="__NEXT_DATA__">{
                "props": {
                    "pageProps": {
                        "programInformation": {
                            "content_id": "VOD00041610",
                            "content_type": "vod",
                            "series_id": "SERIES-41610"
                        },
                        "seriesTree": {
                            "content_id": "SERIES-41610",
                            "content_type": "drama",
                            "title": "Native LiTV series",
                            "seasons": [
                                {"episodes": [
                                    {"content_id": "VOD00041610"},
                                    {"content_id": "VOD00041611"}
                                ]}
                            ]
                        }
                    }
                }
            }</script>"#;
            return Ok(Response::new(url, 200, "OK", page.as_bytes().to_vec()));
        }
        if url.contains("www.litv.tv/api/get-urls-no-auth") {
            return Ok(Response::new(
                url,
                200,
                "OK",
                br#"{"result":{"AssetURLs":["https://cdn.example/litv/native.m3u8"]}}"#.to_vec(),
            ));
        }
        Err(RequestError::new(
            ErrorKind::Transport,
            format!("no LiTV route for {url}"),
        ))
    }
}

fn litv_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(LitvHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

fn litv_extractor() -> LitvExtractor {
    LitvExtractor::new(ExtractorDescriptor::new(
        "LiTVIE",
        "LiTV",
        r#"https?://(?:www\.)?litv\.tv/(?:[^/?#]+/watch/|vod/[^/?#]+/content\.do\?content_id=)(?P<id>[\w-]+)"#,
        true,
    ))
    .unwrap()
}

#[test]
fn litv_native_extractor_maps_next_state_and_hls_playback() {
    let result = litv_extractor()
        .extract_with_context(
            "https://www.litv.tv/drama/watch/VOD00040000",
            &litv_context(),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_str("id"), Some("VOD00040000"));
    assert_eq!(result.get_str("title"), Some("Native LiTV episode HD"));
    assert_eq!(result.get_str("description"), Some("Native LiTV description"));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://p-cdnstatic.svc.litv.tv/images/native.jpg")
    );
    assert_eq!(result.get_i64("episode_number"), Some(7));
    assert_eq!(
        result.get("categories"),
        Some(&serde_json::json!(["Drama", "Fantasy"]))
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("url")),
        Some(&serde_json::json!("https://cdn.example/litv/native.m3u8"))
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("http_headers")),
        Some(&serde_json::json!({"Accept-Encoding": "identity"}))
    );
}

#[test]
fn litv_native_extractor_builds_no_playlist_episode_entries() {
    let result = litv_extractor()
        .extract_with_context(
            "https://www.litv.tv/drama/watch/VOD00041610",
            &litv_context(),
        )
        .unwrap();
    let ExtractorResult::Playlist { info, entries } = result else {
        panic!("expected LiTV playlist");
    };
    assert_eq!(info.get_str("id"), Some("SERIES-41610"));
    assert_eq!(info.get_str("title"), Some("Native LiTV series"));
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get_str("ie_key"), Some("LiTV"));
    assert_eq!(entries[0].get_str("id"), Some("VOD00041610"));
    assert_eq!(
        entries[0].get_str("url"),
        Some("https://www.litv.tv/drama/watch/VOD00041610?force_noplaylist=1")
    );
}
