struct KakaoHandler;

impl RequestHandler for KakaoHandler {
    fn name(&self) -> &str {
        "kakao-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        let url = request.url();
        let body = if url.contains("/api/v1/ft/playmeta/cliplink/") {
            r#"{"clipLink":{"displayTitle":"Fallback Kakao title","channel":{"name":"Native Kakao channel"},"channelId":2671005,"createTime":"2017-02-27T00:00:00Z","clip":{"title":"Native Kakao title","description":"  Native Kakao description  ","duration":1503,"playCount":101,"likeCount":12,"commentCount":4,"tagList":["乃木坂","music"],"thumbnailUrl":"https://cdn.example/top-thumb.png","clipChapterThumbnailList":[{"thumbnailUrl":"https://cdn.example/chapter.png","timeInSec":30,"isDefault":true}],"videoOutputList":[{"profile":"HD","width":1920,"height":1080,"label":"1080p","filesize":123456,"kbps":2500},{"profile":"AUDIO","width":0,"height":0},{"profile":"SD","width":640,"height":360,"label":"360p","kbps":700}]}}}"#
        } else if url.contains("/readyNplay") && url.contains("profile=HD") {
            r#"{"videoLocation":{"url":"https://cdn.example/kakao-hd.mp4"}}"#
        } else if url.contains("/readyNplay") && url.contains("profile=SD") {
            r#"{"videoLocation":{"url":"https://cdn.example/kakao-sd.mp4"}}"#
        } else {
            return Err(RequestError::new(
                ErrorKind::Transport,
                format!("no Kakao route for {url}"),
            ));
        };
        Ok(Response::new(url, 200, "OK", body.as_bytes().to_vec()))
    }
}

#[test]
fn kakao_native_extractor_maps_clip_renditions_and_metadata() {
    let extractor = KakaoExtractor::new(ExtractorDescriptor::new(
        "KakaoIE",
        "Kakao",
        r#"https?://(?:play-)?tv\.kakao\.com/(?:channel/\d+|embed/player)/cliplink/(?P<id>\d+|[^?#&]+@my)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(KakaoHandler);
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://tv.kakao.com/channel/2671005/cliplink/301965083",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("301965083"));
    assert_eq!(result.get_str("title"), Some("Native Kakao title"));
    assert_eq!(result.get_str("description"), Some("Native Kakao description"));
    assert_eq!(result.get_str("uploader"), Some("Native Kakao channel"));
    assert_eq!(result.get_str("uploader_id"), Some("2671005"));
    assert_eq!(result.get("timestamp"), Some(&serde_json::json!(1488153600)));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(1503)));
    assert_eq!(result.get("tags"), Some(&serde_json::json!(["乃木坂", "music"])));
    assert_eq!(
        result
            .get("thumbnails")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(formats[0].get("height"), Some(&serde_json::json!(1080)));
    assert_eq!(formats[1].get("format_id"), Some(&serde_json::json!("SD")));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/kakao-hd.mp4"));
}
