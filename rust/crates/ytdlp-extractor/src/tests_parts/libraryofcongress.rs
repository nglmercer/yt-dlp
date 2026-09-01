struct LibraryOfCongressHandler;

impl RequestHandler for LibraryOfCongressHandler {
    fn name(&self) -> &str {
        "library-of-congress-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        let url = request.url();
        if url.contains("loc.gov/item/nativeloc") {
            return Ok(Response::new(
                url,
                200,
                "OK",
                br#"<meta property="og:title" content="Fallback LOC title">
                    <meta property="og:image" content="https://cdn.example/loc.jpg">
                    <div id="media-player-player42"></div>
                    <option value="https://download.example/loc/low.mp4" data-file-download="yes">MP4 (1.5 MB)<</option>
                    <option value="https://download.example/loc/cover.jpg" data-file-download="yes">JPEG<</option>"#
                    .to_vec(),
            ));
        }
        if url.contains("loc.gov/today/cyberlc/feature_wdesc.php?rec=nativewebcast") {
            return Ok(Response::new(
                url,
                200,
                "OK",
                br#"<title>Native webcast</title>
                    <div>mediaObjectId: "webcast42"</div>"#
                    .to_vec(),
            ));
        }
        if url.contains("media.loc.gov/services/v1/media?id=player42") {
            let body = serde_json::json!({
                "mediaObject": {
                    "mediaType": "v",
                    "duration": "12.5",
                    "viewCount": "77",
                    "ccUrl": "https://cdn.example/loc/native.ttml",
                    "derivatives": [{
                        "derivativeUrl": "rtmp://media.example/vod/mp4:native42",
                        "shortName": "Native LOC video"
                    }]
                }
            });
            return Ok(Response::new(
                url,
                200,
                "OK",
                serde_json::to_vec(&body).unwrap(),
            ));
        }
        if url.contains("media.loc.gov/services/v1/media?id=webcast42") {
            let body = serde_json::json!({
                "mediaObject": {
                    "mediaType": "a",
                    "duration": 8,
                    "viewCount": 9,
                    "derivatives": [{
                        "derivativeUrl": "rtmp://media.example/audio/mp3:native42",
                        "shortName": "Native LOC audio"
                    }]
                }
            });
            return Ok(Response::new(
                url,
                200,
                "OK",
                serde_json::to_vec(&body).unwrap(),
            ));
        }
        Err(RequestError::new(
            ErrorKind::Transport,
            format!("no Library of Congress route for {url}"),
        ))
    }
}

fn library_of_congress_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(LibraryOfCongressHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

fn library_of_congress_extractor() -> LibraryOfCongressExtractor {
    LibraryOfCongressExtractor::new(ExtractorDescriptor::new(
        "LibraryOfCongressIE",
        "Library of Congress",
        r#"https?://(?:www\.)?loc\.gov/(?:item/|today/cyberlc/feature_wdesc\.php\?.*\brec=)(?P<id>[0-9a-z_.]+)"#,
        true,
    ))
    .unwrap()
}

#[test]
fn library_of_congress_native_extractor_maps_media_api_and_downloads() {
    let result = library_of_congress_extractor()
        .extract_with_context(
            "https://www.loc.gov/item/nativeloc/",
            &library_of_congress_context(),
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("nativeloc"));
    assert_eq!(result.get_str("title"), Some("Native LOC video"));
    assert_eq!(result.get_str("thumbnail"), Some("https://cdn.example/loc.jpg"));
    assert_eq!(result.get_f64("duration"), Some(12.5));
    assert_eq!(result.get_i64("view_count"), Some(77));
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 3);
    assert_eq!(formats[0].get("format_id"), Some(&serde_json::json!("hls")));
    assert_eq!(
        formats[0].get("url"),
        Some(&serde_json::json!(
            "https://media.example/hls-vod/media/native42.mp4.m3u8"
        ))
    );
    assert_eq!(
        formats[1].get("url"),
        Some(&serde_json::json!("https://media.example/native42.mp4"))
    );
    assert_eq!(
        formats[2].get("filesize_approx"),
        Some(&serde_json::json!(1_500_000))
    );
    assert_eq!(
        result
            .get("subtitles")
            .and_then(|subtitles| subtitles.get("en"))
            .and_then(serde_json::Value::as_array)
            .and_then(|tracks| tracks.first())
            .and_then(|track| track.get("ext")),
        Some(&serde_json::json!("ttml"))
    );
}

#[test]
fn library_of_congress_native_extractor_handles_audio_media_object_ids() {
    let result = library_of_congress_extractor()
        .extract_with_context(
            "https://www.loc.gov/today/cyberlc/feature_wdesc.php?rec=nativewebcast",
            &library_of_congress_context(),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_str("id"), Some("nativewebcast"));
    assert_eq!(result.get_str("title"), Some("Native LOC audio"));
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 1);
    assert_eq!(formats[0].get("vcodec"), Some(&serde_json::json!("none")));
    assert_eq!(
        formats[0].get("url"),
        Some(&serde_json::json!("https://media.example/native42.mp3"))
    );
}
