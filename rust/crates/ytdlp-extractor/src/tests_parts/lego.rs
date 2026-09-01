struct LegoHandler;

impl RequestHandler for LegoHandler {
    fn name(&self) -> &str {
        "lego-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        if !request.url().contains(
            "services.slingshot.lego.com/mediaplayer/v2?videoId=55492d82-3b1b-4d5e-9857-87fa8c2973b1_en-us",
        ) {
            return Err(RequestError::new(
                ErrorKind::Transport,
                format!("no LEGO route for {}", request.url()),
            ));
        }
        let body = serde_json::json!({
            "Video": {
                "Id": "55492d82-3b1b-4d5e-9857-87fa8c2973b1_en-US",
                "Title": "Native LEGO video",
                "Description": "Native LEGO description",
                "GeneratedCoverImage": "https://cdn.example/lego/native.jpg",
                "Length": 123,
                "SubFileId": "native-sub-file",
                "NetstoragePath": "native/path",
                "InvariantId": "native-invariant",
                "VideoFileId": "native-video-file",
                "VideoVersion": "4",
                "AgeFrom": 5,
                "SeasonTitle": "Native season",
                "Season": 2,
                "Episode": 7
            },
            "VideoFormats": [
                {"Url": "https://cdn.example/lego/native-low.mp4", "Format": "MP4", "Quality": "Low"},
                {"Url": "https://cdn.example/lego/native.m3u8", "Format": "M3U8"}
            ]
        });
        Ok(Response::new(
            request.url(),
            200,
            "OK",
            serde_json::to_vec(&body).unwrap(),
        ))
    }
}

#[test]
fn lego_native_extractor_maps_media_player_formats_and_subtitles() {
    let mut director = RequestDirector::new();
    director.add_handler(LegoHandler);
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let extractor = LegoExtractor::new(ExtractorDescriptor::new(
        "LEGOIE",
        "LEGO",
        r#"https?://(?:www\.)?lego\.com/(?P<locale>[a-z]{2}-[a-z]{2})/(?:[^/]+/)*videos/(?:[^/]+/)*[^/?#]+-(?P<id>[0-9a-f]{32})"#,
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://www.lego.com/en-us/videos/themes/club/native-video-55492d823b1b4d5e985787fa8c2973b1",
            &context,
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(
        result.get_str("id"),
        Some("55492d82-3b1b-4d5e-9857-87fa8c2973b1_en-US")
    );
    assert_eq!(result.get_str("title"), Some("Native LEGO video"));
    assert_eq!(result.get_i64("duration"), Some(123));
    assert_eq!(result.get_i64("age_limit"), Some(5));
    assert_eq!(result.get_str("season"), Some("Native season"));
    assert_eq!(result.get_i64("season_number"), Some(2));
    assert_eq!(result.get_i64("episode_number"), Some(7));
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(formats[0].get("quality"), Some(&serde_json::json!(1)));
    assert_eq!(formats[0].get("height"), Some(&serde_json::json!(270)));
    assert_eq!(
        formats[1].get("protocol"),
        Some(&serde_json::json!("m3u8_native"))
    );
    assert_eq!(
        result
            .get("subtitles")
            .and_then(|subtitles| subtitles.get("en"))
            .and_then(serde_json::Value::as_array)
            .and_then(|tracks| tracks.first())
            .and_then(|track| track.get("url")),
        Some(&serde_json::json!(
            "https://lc-mediaplayerns-live-s.legocdn.com/public/native/path/native-invariant_native-video-file_en-us_4_sub.srt"
        ))
    );
}
