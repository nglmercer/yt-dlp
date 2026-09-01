struct LeFigaroHandler;

impl RequestHandler for LeFigaroHandler {
    fn name(&self) -> &str {
        "lefigaro-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        if request.url().contains("/embed/") {
            let body = br#"<script id="__NEXT_DATA__">{"props":{"pageProps":{"initialProps":{"pageData":{"playerData":{"videoId":"g9j7Eovo","title":"Native Le Figaro video","description":"<p>Native description</p>","poster":"/img/poster.jpg"}}}}}}</script>"#;
            return Ok(Response::new(
                request.url(),
                200,
                "OK",
                body.to_vec(),
            ));
        }
        if request.url().contains("api-graphql.lefigaro.fr/graphql") {
            let page = url::Url::parse(request.url())
                .ok()
                .and_then(|url| {
                    url.query_pairs()
                        .find(|(key, _)| key == "variables")
                        .map(|(_, value)| value.contains("\"page\":2"))
                })
                .unwrap_or(false);
            let body = if page {
                serde_json::json!({
                    "data": {
                        "playlist": {
                            "title": "Native section",
                            "videoCount": 21,
                            "jsonLd": [{
                                "itemListElement": [{
                                    "videoId": "second-id",
                                    "embedUrl": "https://video.lefigaro.fr/embed/figaro/video/second-video",
                                    "name": "Second video",
                                    "description": "<p>Second description</p>",
                                    "thumbnailUrl": "/img/second.jpg"
                                }]
                            }]
                        }
                    }
                })
            } else {
                serde_json::json!({
                    "data": {
                        "playlist": {
                            "title": "Native section",
                            "videoCount": 21,
                            "jsonLd": [{
                                "itemListElement": [{
                                    "videoId": "first-id",
                                    "embedUrl": "https://video.lefigaro.fr/embed/figaro/video/first-video",
                                    "name": "First video",
                                    "description": "<p>First description</p>",
                                    "thumbnailUrl": "/img/first.jpg"
                                }]
                            }]
                        }
                    }
                })
            };
            return Ok(Response::new(
                request.url(),
                200,
                "OK",
                serde_json::to_vec(&body).unwrap(),
            ));
        }
        Err(RequestError::new(
            ErrorKind::Transport,
            format!("no Le Figaro route for {}", request.url()),
        ))
    }
}

fn lefigaro_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(LeFigaroHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

#[test]
fn lefigaro_embed_native_extractor_maps_jwplatform_transparent_result() {
    let extractor = LeFigaroVideoEmbedExtractor::new(ExtractorDescriptor::new(
        "LeFigaroVideoEmbedIE",
        "LeFigaroVideoEmbed",
        r"https?://video\.lefigaro\.fr/embed/[^?#]+/(?P<id>[\w-]+)",
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://video.lefigaro.fr/embed/figaro/video/native-embed/",
            &lefigaro_context(),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(
        result.get_str("_type"),
        Some("url_transparent")
    );
    assert_eq!(result.get_str("id"), Some("g9j7Eovo"));
    assert_eq!(result.get_str("url"), Some("jwplatform:g9j7Eovo"));
    assert_eq!(result.get_str("ie_key"), Some("JWPlatform"));
    assert_eq!(result.get_str("title"), Some("Native Le Figaro video"));
    assert_eq!(
        result.get_str("description"),
        Some("Native description")
    );
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://video.lefigaro.fr/img/poster.jpg")
    );
}

#[test]
fn lefigaro_section_native_extractor_fetches_all_graphql_pages() {
    let extractor = LeFigaroVideoSectionExtractor::new(ExtractorDescriptor::new(
        "LeFigaroVideoSectionIE",
        "LeFigaroVideoSection",
        r"https?://video\.lefigaro\.fr/figaro/(?P<id>[\w-]+)/?(?:[#?]|$)",
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://video.lefigaro.fr/figaro/native-section/",
            &lefigaro_context(),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_str("id"), Some("native-section"));
    assert_eq!(result.get_str("title"), Some("Native section"));
    let entries = result
        .get("entries")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(
        entries[0].get("url"),
        Some(&serde_json::json!("jwplatform:first-id"))
    );
    assert_eq!(
        entries[1].get("url"),
        Some(&serde_json::json!("jwplatform:second-id"))
    );
    assert_eq!(
        entries[0].get("ie_key"),
        Some(&serde_json::json!("JWPlatform"))
    );
}
