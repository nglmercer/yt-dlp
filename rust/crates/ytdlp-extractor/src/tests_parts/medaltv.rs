struct MedalTvHandler;

impl RequestHandler for MedalTvHandler {
    fn name(&self) -> &str {
        "medaltv-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        let url = request.url();
        if url.ends_with("/api/content/jTBFnLKdLy15K") {
            return Ok(Response::new(
                url,
                200,
                "OK",
                br#"{
                    "contentTitle": "Native Medal clip",
                    "contentDescription": "",
                    "created": 1651628243000,
                    "videoLengthSeconds": 13,
                    "views": 1234,
                    "likes": 98,
                    "comments": 7,
                    "poster": {"displayName": "Aciel", "userId": "19335460"},
                    "thumbnailUrl": "https://cdn.example/medal/clip.jpg",
                    "tags": ["valorant", "clutch"],
                    "contentUrlHls": "https://cdn.example/medal/clip.m3u8",
                    "contentUrl": "https://cdn.example/medal/clip.mp4"
                }"#
                .to_vec(),
            ));
        }
        if url.ends_with("/api/content/2WRj40tpY_EU9") {
            return Ok(Response::new(
                url,
                200,
                "OK",
                br#"{
                    "contentTitle": "Native fallback clip",
                    "poster": {"displayName": "Adny", "userId": 6256941},
                    "contentUrl": "https://cdn.example/video/privacy-protected-guest/clip.mp4"
                }"#
                .to_vec(),
            ));
        }
        if url.ends_with("/api/content/2WRj40tpY_EU9/socialVideoUrl") {
            return Ok(Response::new(
                "https://cdn.example/medal/social-fallback.mp4",
                200,
                "OK",
                Vec::new(),
            ));
        }
        Err(RequestError::new(
            ErrorKind::Transport,
            format!("no Medal.tv route for {url}"),
        ))
    }
}

fn medaltv_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(MedalTvHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

fn medaltv_extractor() -> MedalTvExtractor {
    MedalTvExtractor::new(ExtractorDescriptor::new(
        "MedalTVIE",
        "MedalTV",
        r#"https?://(?:www\.)?medal\.tv/games/[^/?#&]+/clips/(?P<id>[^/?#&]+)"#,
        true,
    ))
    .unwrap()
}

#[test]
fn medaltv_native_extractor_maps_api_metadata_and_formats() {
    let result = medaltv_extractor()
        .extract_with_context(
            "https://medal.tv/games/valorant/clips/jTBFnLKdLy15K",
            &medaltv_context(),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_str("id"), Some("jTBFnLKdLy15K"));
    assert_eq!(result.get_str("title"), Some("Native Medal clip"));
    assert_eq!(result.get_str("uploader"), Some("Aciel"));
    assert_eq!(result.get_str("uploader_id"), Some("19335460"));
    assert_eq!(result.get_i64("timestamp"), Some(1_651_628_243));
    assert_eq!(result.get_i64("duration"), Some(13));
    assert_eq!(
        result.get("tags"),
        Some(&serde_json::json!(["valorant", "clutch"]))
    );
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(formats[0].get("protocol"), Some(&serde_json::json!("m3u8_native")));
}

#[test]
fn medaltv_native_extractor_falls_back_to_social_video() {
    let result = medaltv_extractor()
        .extract_with_context(
            "https://medal.tv/games/valorant/clips/2WRj40tpY_EU9",
            &medaltv_context(),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("url")),
        Some(&serde_json::json!(
            "https://cdn.example/medal/social-fallback.mp4"
        ))
    );
}
