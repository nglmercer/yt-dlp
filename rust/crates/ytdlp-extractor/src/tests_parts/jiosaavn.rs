struct JioSaavnHandler;

impl RequestHandler for JioSaavnHandler {
    fn name(&self) -> &str {
        "jiosaavn-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        let url = request.url();
        let body = if request.method() == "POST" {
            if String::from_utf8_lossy(request.data().unwrap_or_default()).contains("bitrate=128")
            {
                r#"{"auth_url":"https://cdn.example/native-128.m4a","type":"mp4"}"#
            } else {
                r#"{"auth_url":"https://cdn.example/native-320.mp3","type":"mp3"}"#
            }
        } else if url.contains("www.jiosaavn.com/shows/native-show/1/") {
            r#"<script>window.__INITIAL_DATA__ = {"showView":{"current_id":"show-42","show":{"title":{"text":"Native Show"}}}};</script>"#
        } else if url.contains("type=song") {
            r#"{"songs":[{"id":"song-42","song":{"title":"<b>Native Song</b>"},"more_info":{"album":"<i>Native Album</i>","duration":"205","label":"Native Label","label_id":"label-42","label_url":"/label/native","release_date":"2024-09-26","encrypted_media_url":"encrypted-song"},"year":"2024","image":"https://cdn.example/art-150x150.jpg","play_count":"17","language":"Hindi","perma_url":"https://www.jiosaavn.com/song/native-song/song-display","primary_artists":"Artist One","featured_artists":"Artist Two","more_info":{"album":"<i>Native Album</i>","duration":"205","label":"Native Label","label_id":"label-42","label_url":"/label/native","release_date":"2024-09-26","encrypted_media_url":"encrypted-song","artistMap":{"primary_artists":[{"name":"Artist One"}]}}}]}"#
        } else if url.contains("type=episode") {
            r#"{"episodes":[{"id":"episode-42","song":{"title":"Native Episode"},"more_info":{"duration":"311","encrypted_media_url":"encrypted-episode","description":"Native description","release_time":"1640563200","show_title":"Native Series","show_id":"series-42","season_title":"Native Season","season_no":"1","season_id":"season-42","episode_number":"1"},"year":"2021","image":"https://cdn.example/episode.jpg","play_count":"3","language":"English","perma_url":"https://www.jiosaavn.com/shows/native-show/NativeEpisode42","starring":"Host One, Host Two"}]}"#
        } else if url.contains("type=album") {
            r#"{"title":"Native Album Playlist","songs":[{"id":"song-1","song":{"title":"Album Song"},"more_info":{"duration":"100","encrypted_media_url":"encrypted-1"},"perma_url":"https://www.jiosaavn.com/song/album-song/song-1"},{"id":"song-2","song":{"title":"Second Song"},"more_info":{"duration":"200","encrypted_media_url":"encrypted-2"},"perma_url":"https://www.jiosaavn.com/song/second-song/song-2"}]}"#
        } else if url.contains("type=playlist") {
            r#"{"list_count":"2","listname":"Native Playlist","songs":[{"id":"song-3","song":{"title":"Playlist Song"},"more_info":{"encrypted_media_url":"encrypted-3"},"perma_url":"https://www.jiosaavn.com/song/playlist-song/song-3"}]}"#
        } else if url.contains("type=artist") {
            if url.contains("p=0") {
                r#"{"name":"Native Artist","topSongs":[{"id":"song-4","song":{"title":"Artist Song"},"more_info":{"encrypted_media_url":"encrypted-4"},"perma_url":"https://www.jiosaavn.com/song/artist-song/song-4"}]}"#
            } else {
                r#"{"topSongs":[]}"#
            }
        } else if url.contains("type=show") {
            if url.contains("p=1") {
                r#"{"episodes":[{"id":"episode-43","song":{"title":"Season Episode"},"more_info":{"encrypted_media_url":"encrypted-43"},"perma_url":"https://www.jiosaavn.com/shows/native-show/SeasonEpisode43"}]}"#
            } else {
                r#"{"episodes":[]}"#
            }
        } else {
            return Err(RequestError::new(
                ErrorKind::Transport,
                format!("no JioSaavn route for {url}"),
            ));
        };
        Ok(Response::new(
            request.url(),
            200,
            "OK",
            body.as_bytes().to_vec(),
        ))
    }
}

fn jiosaavn_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(JioSaavnHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

#[test]
fn jiosaavn_song_native_extractor_maps_metadata_and_authorized_bitrates() {
    let extractor = JioSaavnSongExtractor::new(ExtractorDescriptor::new(
        "JioSaavnSongIE",
        "jiosaavn:song",
        r#"https?://(?:www\.)?(?:jio)?saavn\.com(?:/song/[^/?#]+/|/s/song/(?:[^/?#]+/){3})(?P<id>[^/?#]+)"#,
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://www.jiosaavn.com/song/native-song/song-display",
            &jiosaavn_context(),
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("song-42"));
    assert_eq!(result.get_str("display_id"), Some("song-display"));
    assert_eq!(result.get_str("title"), Some("Native Song"));
    assert_eq!(result.get_str("album"), Some("Native Album"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(205)));
    assert_eq!(result.get_str("language"), Some("hin"));
    assert_eq!(result.get_str("release_date"), Some("20240926"));
    assert_eq!(result.get_str("channel_url"), Some("https://www.jiosaavn.com/label/native"));
    assert_eq!(
        result.get("artists"),
        Some(&serde_json::json!(["Artist One", "Artist Two"]))
    );
    assert_eq!(
        result.get("_old_archive_ids"),
        Some(&serde_json::json!(["jiosaavnsong song-display"]))
    );
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(formats[0].get("format_id"), Some(&serde_json::json!("128")));
    assert_eq!(formats[1].get("abr"), Some(&serde_json::json!(320)));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/native-128.m4a"));
}

#[test]
fn jiosaavn_show_native_extractor_maps_episode_metadata() {
    let extractor = JioSaavnShowExtractor::new(ExtractorDescriptor::new(
        "JioSaavnShowIE",
        "jiosaavn:show",
        r#"https?://(?:www\.)?(?:jio)?saavn\.com/shows/[^/?#]+/(?P<id>[^/?#]{11,})/?(?:$|[?#])"#,
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://www.jiosaavn.com/shows/native-show/NativeEpisode42",
            &jiosaavn_context(),
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("episode-42"));
    assert_eq!(result.get_str("title"), Some("Native Episode"));
    assert_eq!(result.get_str("description"), Some("Native description"));
    assert_eq!(result.get("timestamp"), Some(&serde_json::json!(1640563200)));
    assert_eq!(result.get_str("series"), Some("Native Series"));
    assert_eq!(result.get_str("season"), Some("Native Season"));
    assert_eq!(result.get("episode_number"), Some(&serde_json::json!(1)));
    assert_eq!(
        result.get("cast"),
        Some(&serde_json::json!(["Host One", "Host Two"]))
    );
    assert!(!result.contains_key("_old_archive_ids"));
}

#[test]
fn jiosaavn_native_playlist_extractors_materialize_native_entries() {
    let album = JioSaavnAlbumExtractor::new(ExtractorDescriptor::new(
        "JioSaavnAlbumIE",
        "jiosaavn:album",
        r#"https?://(?:www\.)?(?:jio)?saavn\.com/album/[^/?#]+/(?P<id>[^/?#]+)"#,
        true,
    ))
    .unwrap();
    let album_result = album
        .extract_with_context(
            "https://www.jiosaavn.com/album/native/album-42",
            &jiosaavn_context(),
        )
        .unwrap();
    let ExtractorResult::Playlist { info, entries } = album_result else {
        panic!("expected JioSaavn album playlist");
    };
    assert_eq!(info.get_str("title"), Some("Native Album Playlist"));
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get_str("_type"), Some("url_transparent"));
    assert_eq!(entries[0].get_str("ie_key"), Some("JioSaavnSong"));
    assert_eq!(entries[0].get_str("title"), Some("Album Song"));

    let playlist = JioSaavnPlaylistExtractor::new(ExtractorDescriptor::new(
        "JioSaavnPlaylistIE",
        "jiosaavn:playlist",
        r#"https?://(?:www\.)?(?:jio)?saavn\.com/(?:s/playlist/(?:[^/?#]+/){2}|featured/[^/?#]+/)(?P<id>[^/?#]+)"#,
        true,
    ))
    .unwrap();
    let ExtractorResult::Playlist { entries, .. } = playlist
        .extract_with_context(
            "https://www.jiosaavn.com/featured/native/playlist-42",
            &jiosaavn_context(),
        )
        .unwrap()
    else {
        panic!("expected JioSaavn playlist");
    };
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].get_str("ie_key"), Some("JioSaavnSong"));

    let artist = JioSaavnArtistExtractor::new(ExtractorDescriptor::new(
        "JioSaavnArtistIE",
        "jiosaavn:artist",
        r#"https?://(?:www\.)?(?:jio)?saavn\.com/artist/[^/?#]+/(?P<id>[^/?#]+)"#,
        true,
    ))
    .unwrap();
    let ExtractorResult::Playlist { info, entries } = artist
        .extract_with_context(
            "https://www.jiosaavn.com/artist/native/artist-42",
            &jiosaavn_context(),
        )
        .unwrap()
    else {
        panic!("expected JioSaavn artist playlist");
    };
    assert_eq!(info.get_str("title"), Some("Native Artist"));
    assert_eq!(entries.len(), 1);

    let show_playlist = JioSaavnShowPlaylistExtractor::new(ExtractorDescriptor::new(
        "JioSaavnShowPlaylistIE",
        "jiosaavn:show:playlist",
        r#"https?://(?:www\.)?(?:jio)?saavn\.com/shows/(?P<show>[^#/?]+)/(?P<season>\d+)/[^/?#]+"#,
        true,
    ))
    .unwrap();
    let ExtractorResult::Playlist { info, entries } = show_playlist
        .extract_with_context(
            "https://www.jiosaavn.com/shows/native-show/1/season-42",
            &jiosaavn_context(),
        )
        .unwrap()
    else {
        panic!("expected JioSaavn show playlist");
    };
    assert_eq!(info.get_str("id"), Some("native-show-1"));
    assert_eq!(info.get_str("title"), Some("Native Show"));
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].get_str("ie_key"), Some("JioSaavnShow"));
}
