struct LastFmHandler;

impl RequestHandler for LastFmHandler {
    fn name(&self) -> &str {
        "lastfm-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        let url = request.url();
        let body = if url.contains("/music/Native/_/Native-track") {
            br#"<a class="header-new-playlink" href="https://www.youtube.com/watch?v=native-lastfm"></a>"#
                .to_vec()
        } else if url.contains("page=1") {
            br#"<div data-youtube-url="https://www.youtube.com/watch?v=native-one"></div>
                <div data-youtube-url="https://www.youtube.com/watch?v=native-two"></div>"#
                .to_vec()
        } else if url.contains("page=2") {
            br#"<div class="empty"></div>"#.to_vec()
        } else {
            return Err(RequestError::new(
                ErrorKind::Transport,
                format!("no Last.fm route for {url}"),
            ));
        };
        Ok(Response::new(url, 200, "OK", body))
    }
}

fn lastfm_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(LastFmHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

#[test]
fn lastfm_native_track_extractor_returns_youtube_redirect() {
    let extractor = LastFmExtractor::new(ExtractorDescriptor::new(
        "LastFMIE",
        "LastFM",
        r#"https?://(?:www\.)?last\.fm/music(?:/[^/]+){2}/(?P<id>[^/#?]+)"#,
        true,
    ))
    .unwrap();
    assert_eq!(
        extractor
            .extract_with_context(
                "https://www.last.fm/music/Native/_/Native-track",
                &lastfm_context(),
            )
            .unwrap(),
        ExtractorResult::Redirect {
            url: "https://www.youtube.com/watch?v=native-lastfm".to_owned(),
            ie_key: Some("Youtube".to_owned()),
        }
    );
}

#[test]
fn lastfm_native_playlists_paginate_youtube_entries() {
    let playlist = LastFmPlaylistExtractor::new(ExtractorDescriptor::new(
        "LastFMPlaylistIE",
        "LastFMPlaylist",
        r#"https?://(?:www\.)?last\.fm/(music|tag)/(?P<id>[^/]+)(?:/[^/]+)?/?(?:[?#]|$)"#,
        true,
    ))
    .unwrap()
    .extract_with_context("https://www.last.fm/music/Native", &lastfm_context())
    .unwrap()
    .into_info_dict();
    assert_eq!(playlist.get_str("id"), Some("Native"));
    let entries = playlist
        .get("entries")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(
        entries[0].get("ie_key"),
        Some(&serde_json::json!("Youtube"))
    );
    assert_eq!(
        entries[1].get("url"),
        Some(&serde_json::json!(
            "https://www.youtube.com/watch?v=native-two"
        ))
    );
}

#[test]
fn lastfm_native_user_playlist_uses_requested_page() {
    let playlist = LastFmUserExtractor::new(ExtractorDescriptor::new(
        "LastFMUserIE",
        "LastFMUser",
        r#"https?://(?:www\.)?last\.fm/user/[^/]+/playlists/(?P<id>[^/#?]+)"#,
        true,
    ))
    .unwrap()
    .extract_with_context(
        "https://www.last.fm/user/native/playlists/123?page=1",
        &lastfm_context(),
    )
    .unwrap()
    .into_info_dict();
    assert_eq!(playlist.get_str("id"), Some("123"));
    assert_eq!(
        playlist
            .get("entries")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
}
