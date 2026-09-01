struct MaveHandler;

impl RequestHandler for MaveHandler {
    fn name(&self) -> &str {
        "mave-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        let url = request.url();
        let body = if url.contains("api.mave.digital/v1/website/native-show/")
            && url.contains("/episodes/25")
        {
            r#"{
                "id": "episode-25",
                "code": 25,
                "audio": "storage/podcasts/native.mp3",
                "title": "Native Mave episode",
                "description": "<p>Native <b>episode</b> description</p>",
                "image": "storage/podcasts/native.jpg",
                "duration": 3744,
                "season": 3,
                "number": 3,
                "listenings": 42,
                "reactions": [
                    {"type": "like", "count": 7},
                    {"type": "dislike", "count": 1}
                ],
                "is_explicit": true,
                "publish_date": "2025-05-21T12:08:20Z"
            }"#
        } else if url.contains("api.mave.digital/v1/website/native-show/")
            && url.contains("/episodes?")
        {
            if url.contains("page=1") {
                r#"{
                    "episodes": [
                        {"id": "episode-1", "code": 1, "audio": "storage/podcasts/one.mp3", "title": "First"},
                        {"id": "episode-no-audio", "code": 2, "audio": null, "title": "Unavailable"}
                    ]
                }"#
            } else {
                r#"{
                    "episodes": [
                        {"id": "episode-3", "code": 3, "audio": "storage/podcasts/three.mp3", "title": "Third"}
                    ]
                }"#
            }
        } else if url.contains("api.mave.digital/v1/website/native-show/") {
            r#"{
                "podcast": {
                    "id": "series-42",
                    "title": "Native Mave show",
                    "description": "Native channel description",
                    "author": "Native host",
                    "episodes_count": 51
                }
            }"#
        } else {
            return Err(RequestError::new(
                ErrorKind::Transport,
                format!("no Mave route for {url}"),
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

fn mave_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(MaveHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

#[test]
fn mave_native_extractor_maps_episode_metadata_and_audio() {
    let extractor = MaveExtractor::new(ExtractorDescriptor::new(
        "MaveIE",
        "mave",
        r#"https?://(?P<channel_id>[\w-]+)\.mave\.digital/ep-(?P<episode_code>\d+)"#,
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://native-show.mave.digital/ep-25",
            &mave_context(),
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("episode-25"));
    assert_eq!(result.get_str("display_id"), Some("native-show-25"));
    assert_eq!(result.get_str("title"), Some("Native Mave episode"));
    assert_eq!(
        result.get_str("description"),
        Some("Native episode description")
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://store.cloud.mts.ru/mave/storage/podcasts/native.mp3")
    );
    assert_eq!(result.get_str("ext"), Some("mp3"));
    assert_eq!(result.get_i64("duration"), Some(3744));
    assert_eq!(result.get_i64("season_number"), Some(3));
    assert_eq!(result.get_i64("episode_number"), Some(3));
    assert_eq!(result.get_i64("like_count"), Some(7));
    assert_eq!(result.get_i64("dislike_count"), Some(1));
    assert_eq!(result.get_i64("age_limit"), Some(18));
    assert_eq!(result.get_i64("timestamp"), Some(1_747_829_300));
    assert_eq!(result.get_str("series"), Some("Native Mave show"));
    assert_eq!(result.get_str("uploader"), Some("Native host"));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://store.cloud.mts.ru/mave/storage/podcasts/native.jpg")
    );
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 1);
    assert_eq!(formats[0].get("acodec"), Some(&serde_json::json!("mp3")));
}

#[test]
fn mave_channel_native_extractor_materializes_audio_pages() {
    let extractor = MaveChannelExtractor::new(ExtractorDescriptor::new(
        "MaveChannelIE",
        "mave:channel",
        r#"https?://(?P<id>[\w-]+)\.mave\.digital/?(?:$|[?#])"#,
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context("https://native-show.mave.digital/", &mave_context())
        .unwrap();
    let ExtractorResult::Playlist { info, entries } = result else {
        panic!("expected Mave channel playlist");
    };

    assert_eq!(info.get_str("id"), Some("native-show"));
    assert_eq!(info.get_str("title"), Some("Native Mave show"));
    assert_eq!(
        info.get_str("description"),
        Some("Native channel description")
    );
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get_str("id"), Some("episode-1"));
    assert_eq!(entries[0].get_str("display_id"), Some("native-show-1"));
    assert_eq!(entries[1].get_str("id"), Some("episode-3"));
    assert_eq!(entries[1].get_str("url"), Some("https://store.cloud.mts.ru/mave/storage/podcasts/three.mp3"));
}
