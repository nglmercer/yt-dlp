fn globalplayer_next_data(props: serde_json::Value) -> Vec<u8> {
    format!(
        "<script id=\"__NEXT_DATA__\" type=\"application/json\">{props}</script>"
    )
    .into_bytes()
}

#[test]
fn globalplayer_live_native_extractor_maps_station_and_playable() {
    let extractor = GlobalPlayerLiveExtractor::new(ExtractorDescriptor::new(
        "GlobalPlayerLiveIE",
        "GlobalPlayerLive",
        r#"https?://www\.globalplayer\.com/live/(?P<id>\w+)/\w+"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "globalplayer.com/live/smoothchill/uk".to_owned(),
                globalplayer_next_data(serde_json::json!({
                    "props": {"pageProps": {"station": {
                        "id": "2mx1E",
                        "brandLogo": "https://cdn.example/logo.jpg",
                        "tagline": "Native station tagline",
                        "name": "Smooth Chill UK"
                    }}}
                })),
            ),
            (
                "musicradio.com/playables/2mx1E".to_owned(),
                br#"{"playback":[
                    {"canUse":"false","url":"https://cdn.example/blocked.aac"},
                    {"canUse":"true","url":"https://cdn.example/smooth.aac"}
                ]}"#
                .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.globalplayer.com/live/smoothchill/uk/",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("2mx1E"));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/smooth.aac"));
    assert_eq!(result.get_str("ext"), Some("aac"));
    assert_eq!(result.get_str("title"), Some("Smooth Chill UK"));
    assert_eq!(result.get_str("description"), Some("Native station tagline"));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/logo.jpg")
    );
    assert_eq!(result.get_bool("is_live"), Some(true));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("vcodec"))
            .and_then(serde_json::Value::as_str),
        Some("none")
    );
}

#[test]
fn globalplayer_live_playlist_native_extractor_maps_stream() {
    let extractor = GlobalPlayerLivePlaylistExtractor::new(ExtractorDescriptor::new(
        "GlobalPlayerLivePlaylistIE",
        "GlobalPlayerLivePlaylist",
        r#"https?://www\.globalplayer\.com/playlists/(?P<id>\w+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: globalplayer_next_data(serde_json::json!({
            "props": {"pageProps": {"playlistData": {
                "streamUrl": "https://cdn.example/playlist.aac",
                "image": "https://cdn.example/playlist.jpg",
                "description": "Native playlist description",
                "title": "Native playlist title"
            }}}
        })),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.globalplayer.com/playlists/8bLk/",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("8bLk"));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/playlist.aac"));
    assert_eq!(result.get_str("title"), Some("Native playlist title"));
    assert_eq!(result.get_str("thumbnail"), Some("https://cdn.example/playlist.jpg"));
}

#[test]
fn globalplayer_audio_episode_native_extractor_maps_playable() {
    let extractor = GlobalPlayerAudioEpisodeExtractor::new(ExtractorDescriptor::new(
        "GlobalPlayerAudioEpisodeIE",
        "GlobalPlayerAudioEpisode",
        r#"https?://www\.globalplayer\.com/(?:(?P<podcast>podcasts)|catchup/\w+/\w+)/episodes/(?P<id>\w+)/?(?:$|[?#])"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "globalplayer.com/podcasts/episodes/7DrorSc".to_owned(),
                globalplayer_next_data(serde_json::json!({
                    "props": {"pageProps": {"podcastEpisode": {
                        "metadata": {
                            "image": {"url": "https://cdn.example/episode.jpg"},
                            "description": "Native episode description",
                            "title": "Native episode title"
                        }
                    }}}
                })),
            ),
            (
                "musicradio.com/playables/7DrorSc".to_owned(),
                br#"{"playback":[{"canUse":"true","url":"https://cdn.example/episode.mp3"}]}"#
                    .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.globalplayer.com/podcasts/episodes/7DrorSc/",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("7DrorSc"));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/episode.mp3"));
    assert_eq!(result.get_str("title"), Some("Native episode title"));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/episode.jpg")
    );
    assert_eq!(result.get_str("vcodec"), Some("none"));
}

#[test]
fn globalplayer_audio_native_extractor_builds_episode_playlist() {
    let extractor = GlobalPlayerAudioExtractor::new(ExtractorDescriptor::new(
        "GlobalPlayerAudioIE",
        "GlobalPlayerAudio",
        r#"https?://www\.globalplayer\.com/(?P<path>(?P<podcast>podcasts)/|catchup/\w+/\w+/)(?P<id>\w+)/?(?:$|[?#])"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "globalplayer.com/podcasts/42KuaM".to_owned(),
                globalplayer_next_data(serde_json::json!({
                    "props": {"pageProps": {"podcastInfo": {
                        "metadata": {
                            "image": {"url": "https://cdn.example/podcast.jpg"},
                            "description": "Native podcast description",
                            "title": "Native podcast title"
                        },
                        "blocks": [{}, {"items": [
                            {"id": "episode-1", "title": "Episode 1",
                             "image": {"url": "https://cdn.example/e1.jpg"}}
                        ]}]
                    }}}
                })),
            ),
            (
                "musicradio.com/playables/episode-1".to_owned(),
                br#"{"playback":[{"canUse":"true","url":"https://cdn.example/e1.mp3"}]}"#
                    .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.globalplayer.com/podcasts/42KuaM/",
            &context,
        )
        .unwrap();
    let info = result.clone().into_info_dict();

    assert_eq!(info.get_str("id"), Some("42KuaM"));
    assert_eq!(info.get_str("title"), Some("Native podcast title"));
    assert_eq!(
        info.get("entries")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        info.get("entries")
            .and_then(serde_json::Value::as_array)
            .and_then(|entries| entries.first())
            .and_then(|entry| entry.get("url"))
            .and_then(serde_json::Value::as_str),
        Some("https://cdn.example/e1.mp3")
    );
}

#[test]
fn globalplayer_video_native_extractor_maps_video_data() {
    let extractor = GlobalPlayerVideoExtractor::new(ExtractorDescriptor::new(
        "GlobalPlayerVideoIE",
        "GlobalPlayerVideo",
        r#"https?://www\.globalplayer\.com/videos/(?P<id>\w+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: globalplayer_next_data(serde_json::json!({
            "props": {"pageProps": {"videoData": {
                "url": "https://cdn.example/video.mp4",
                "image": {"url": "https://cdn.example/video.jpg"},
                "description": "Native video description",
                "title": "Native video title"
            }}}
        })),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.globalplayer.com/videos/2JsSZ7Gm2uP/",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("2JsSZ7Gm2uP"));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/video.mp4"));
    assert_eq!(result.get_str("title"), Some("Native video title"));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/video.jpg")
    );
}
