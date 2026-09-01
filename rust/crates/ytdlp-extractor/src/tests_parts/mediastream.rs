struct MediaStreamHandler;

impl RequestHandler for MediaStreamHandler {
    fn name(&self) -> &str {
        "mediastream-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        if request.url().contains("winsports.co") {
            return Ok(Response::new(
                request.url(),
                200,
                "OK",
                br#"<html>
                    <script type="application/json" data-drupal-selector="drupal-settings-json">
                        {"settings":{"mediastream_formatter":{"player":{"mediastream_id":{"url":"https://mdstrm.com/live-stream/win-native"}}}}}
                    </script>
                    <script type="application/ld+json">{"@type":"VideoObject","name":"Native WinSports | Win Sports"}</script>
                </html>"#
                .to_vec(),
            ));
        }
        Ok(Response::new(
            request.url(),
            200,
            "OK",
            br#"<html>
                <meta property="og:title" content="Native MediaStream title">
                <meta property="og:description" content="Native MediaStream description">
                <meta property="og:image" content="https://cdn.example/mediastream.jpg">
                <script>
                    window.MDSTRMUID = "uid-native";
                    window.MDSTRMSID = "sid-native";
                    window.MDSTRMPID = "pid-native";
                    window.VERSION = "av-native";
                    window.MDSTRM.OPTIONS = {
                        src: {
                            hls: "https://cdn.example/native.m3u8",
                            mpd: "https://cdn.example/native.mpd",
                            mp4: "https://cdn.example/native.mp4"
                        },
                        title: "Config title",
                        type: "live"
                    };
                </script>
            </html>"#
            .to_vec(),
        ))
    }
}

#[test]
fn mediastream_native_extractor_maps_configured_manifests_and_access_query() {
    let extractor = MediaStreamExtractor::new(ExtractorDescriptor::new(
        "MediaStreamIE",
        "MediaStream",
        r#"https?://mdstrm\.com/(?:embed|live-stream)/(?P<id>\w+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(MediaStreamHandler);
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://mdstrm.com/embed/native?access_token=token-native",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("native"));
    assert_eq!(result.get_str("title"), Some("Native MediaStream title"));
    assert_eq!(result.get_str("description"), Some("Native MediaStream description"));
    assert_eq!(result.get_str("thumbnail"), Some("https://cdn.example/mediastream.jpg"));
    assert_eq!(result.get_bool("is_live"), Some(true));
    assert_eq!(result.get_str("live_status"), Some("is_live"));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/native.m3u8?at=web-app&access_token=token-native&uid=uid-native&sid=sid-native&pid=pid-native&av=av-native")
    );
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 3);
    assert_eq!(
        formats[0]
            .get("protocol")
            .and_then(serde_json::Value::as_str),
        Some("m3u8_native")
    );
    let dash = formats
        .iter()
        .find(|format| format.get("format_id") == Some(&serde_json::json!("mpd")))
        .unwrap();
    assert_eq!(
        dash.get("protocol").and_then(serde_json::Value::as_str),
        Some("http_dash_segments")
    );
}

#[test]
fn winsports_native_extractor_preserves_transparent_title_and_embed_key() {
    let extractor = WinSportsExtractor::new(ExtractorDescriptor::new(
        "WinSportsVideoIE",
        "WinSportsVideo",
        r#"https?://www\.winsports\.co/videos/(?P<id>[\w-]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(MediaStreamHandler);
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.winsports.co/videos/native-win-sports",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("_type"), Some("url_transparent"));
    assert_eq!(result.get_str("url"), Some("https://mdstrm.com/live-stream/win-native"));
    assert_eq!(result.get_str("ie_key"), Some("MediaStream"));
    assert_eq!(result.get_str("display_id"), Some("native-win-sports"));
    assert_eq!(result.get_str("title"), Some("Native WinSports"));
}
