struct LikeeHandler;

impl RequestHandler for LikeeHandler {
    fn name(&self) -> &str {
        "likee-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        if request.url().contains("api.like-video.com/likee-activity-flow-micro/videoApi/getUserVideo") {
            let request_data = request
                .data()
                .and_then(|data| serde_json::from_slice::<serde_json::Value>(data).ok())
                .unwrap_or(serde_json::Value::Null);
            let last_post_id = request_data
                .get("lastPostId")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let body = if last_post_id.is_empty() {
                serde_json::json!({
                    "data": {
                        "videoList": [
                            {"postId": "post-1"},
                            {"postId": "post-2"}
                        ]
                    }
                })
            } else {
                serde_json::json!({"data": {"videoList": []}})
            };
            return Ok(Response::new(
                request.url(),
                200,
                "OK",
                serde_json::to_vec(&body).unwrap(),
            ));
        }
        if !request
            .url()
            .contains("likee.video/@native_creator")
        {
            return Err(RequestError::new(
                ErrorKind::Transport,
                format!("no Likee route for {}", request.url()),
            ));
        }
        let payload = serde_json::json!({
            "userinfo": {
                "uid": 925638334,
                "user_name": "@native_creator"
            },
            "video_url": "https://cdn.example/likee/clip_4.mp4",
            "video_width": 1080,
            "video_height": 1920,
            "msgText": "Native Likee video",
            "share_desc": "Native Likee description",
            "video_count": 77,
            "likeCount": 12,
            "comment_count": 3,
            "nick_name": "Native creator",
            "likeeId": "native_creator",
            "sound": {"owner_name": "Native artist"},
            "uploadDate": "2022-05-03T04:15:20Z",
            "coverUrl": "https://cdn.example/likee/cover.jpg",
            "option_data": {"dur": 123},
        });
        let webpage = format!("window.data = {payload};");
        Ok(Response::new(
            request.url(),
            200,
            "OK",
            webpage.into_bytes(),
        ))
    }
}

fn likee_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(LikeeHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

#[test]
fn likee_user_native_extractor_maps_paginated_video_entries() {
    let extractor = LikeeUserExtractor::new(ExtractorDescriptor::new(
        "LikeeUserIE",
        "likee:user",
        r#"https?://(www\.)?likee\.video/(?P<id>[^/]+)/?$"#,
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://likee.video/@native_creator",
            &likee_context(),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_i64("id"), Some(925638334));
    assert_eq!(result.get_str("title"), Some("@native_creator"));
    let entries = result
        .get("entries")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(
        entries[0].get("url"),
        Some(&serde_json::json!(
            "https://likee.video/@native_creator/video/post-1"
        ))
    );
    assert_eq!(
        entries[1].get("url"),
        Some(&serde_json::json!(
            "https://likee.video/@native_creator/video/post-2"
        ))
    );
}

#[test]
fn likee_native_extractor_maps_page_payload_and_watermark_variants() {
    let extractor = LikeeExtractor::new(ExtractorDescriptor::new(
        "LikeeIE",
        "likee",
        r#"(?x)https?://(www\.)?likee\.video/(?:(?P<channel_name>[^/]+)/video/|v/)(?P<id>\w+)"#,
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://likee.video/@native_creator/video/7093444807096327263",
            &likee_context(),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_str("id"), Some("7093444807096327263"));
    assert_eq!(result.get_str("title"), Some("Native Likee video"));
    assert_eq!(result.get_str("description"), Some("Native Likee description"));
    assert_eq!(result.get_i64("view_count"), Some(77));
    assert_eq!(result.get_i64("like_count"), Some(12));
    assert_eq!(result.get_i64("comment_count"), Some(3));
    assert_eq!(result.get_str("uploader"), Some("Native creator"));
    assert_eq!(result.get_str("uploader_id"), Some("native_creator"));
    assert_eq!(result.get_str("artist"), Some("Native artist"));
    assert_eq!(result.get_i64("timestamp"), Some(1_651_551_320));
    assert_eq!(result.get_i64("duration"), Some(123));
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(
        formats[0].get("url"),
        Some(&serde_json::json!("https://cdn.example/likee/clip_4.mp4"))
    );
    assert_eq!(
        formats[1].get("url"),
        Some(&serde_json::json!("https://cdn.example/likee/clip.mp4"))
    );
    assert_eq!(formats[1].get("quality"), Some(&serde_json::json!(1)));
    assert_eq!(formats[0].get("width"), Some(&serde_json::json!(1080)));
    assert_eq!(formats[0].get("height"), Some(&serde_json::json!(1920)));
}
