#[test]
fn flickr_native_extractor_maps_rest_metadata_and_streams() {
    let extractor = FlickrExtractor::new(ExtractorDescriptor::new(
        "FlickrIE",
        "Flickr",
        r"https?://(?:www\.|secure\.)?flickr\.com/photos/[\w\-_@]+/(?P<id>\d+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "hermes_error_beacon.gne".to_owned(),
                br#"{"site_key":"native-flickr-key"}"#.to_vec(),
            ),
            (
                "method=flickr.photos.getInfo".to_owned(),
                br#"{"stat":"ok","photo":{
                    "media":"video",
                    "secret":{"_content":"native-secret"},
                    "title":{"_content":"Dark Hollow Waterfalls"},
                    "description":{"_content":"Native Flickr description"},
                    "dateuploaded":"1303528740",
                    "video":{"duration":"19"},
                    "owner":{"nsid":"10922353@N03","realname":"Forest Wander","path_alias":"forestwander-nature-pictures"},
                    "comments":{"_content":"7"},
                    "views":"1234",
                    "tags":{"tag":[{"_content":"waterfalls"},{"_content":"spring"}]},
                    "license":"5"
                }}"#
                .to_vec(),
            ),
            (
                "method=flickr.video.getStreamInfo".to_owned(),
                br#"{"stat":"ok","streams":{"stream":[
                    {"_content":"https://cdn.example/flickr/360.mpg","type":{"_content":"360p"}},
                    {"_content":"https://cdn.example/flickr/original.mpg","type":{"_content":"orig"}}
                ]}}"#
                .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.flickr.com/photos/forestwander-nature-pictures/5645318632",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("5645318632"));
    assert_eq!(result.get_str("title"), Some("Dark Hollow Waterfalls"));
    assert_eq!(
        result.get_str("description"),
        Some("Native Flickr description")
    );
    assert_eq!(result.get_i64("timestamp"), Some(1303528740));
    assert_eq!(result.get_i64("duration"), Some(19));
    assert_eq!(result.get_str("uploader_id"), Some("10922353@N03"));
    assert_eq!(result.get_str("uploader"), Some("Forest Wander"));
    assert_eq!(
        result.get_str("uploader_url"),
        Some("https://www.flickr.com/photos/forestwander-nature-pictures/")
    );
    assert_eq!(result.get_i64("comment_count"), Some(7));
    assert_eq!(result.get_i64("view_count"), Some(1234));
    assert_eq!(
        result.get("tags"),
        Some(&serde_json::json!(["waterfalls", "spring"]))
    );
    assert_eq!(
        result.get_str("license"),
        Some("Attribution-ShareAlike")
    );
    assert_eq!(result.get_str("ext"), Some("mpg"));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
}

#[test]
fn flickr_native_extractor_rejects_non_video_photos() {
    let extractor = FlickrExtractor::new(ExtractorDescriptor::new(
        "FlickrIE",
        "Flickr",
        r"https?://(?:www\.|secure\.)?flickr\.com/photos/[\w\-_@]+/(?P<id>\d+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "hermes_error_beacon.gne".to_owned(),
                br#"{"site_key":"native-flickr-key"}"#.to_vec(),
            ),
            (
                "method=flickr.photos.getInfo".to_owned(),
                br#"{"stat":"ok","photo":{"media":"photo","title":{"_content":"Still image"}}}"#
                    .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context("https://www.flickr.com/photos/user/123", &context)
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Extraction);
    assert!(error.message.contains("is not a video"));
}
