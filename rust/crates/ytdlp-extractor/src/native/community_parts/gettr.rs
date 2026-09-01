const GETTR_API_BASE: &str = "https://api.gettr.com/u/";
const GETTR_MEDIA_BASE: &str = "https://media.gettr.com/";

/// Native GETTR post extractor.
pub struct GettrExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl GettrExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for GettrExtractor {
    fn descriptor(&self) -> &ExtractorDescriptor {
        &self.descriptor
    }

    fn suitable(&self, url: &str) -> bool {
        self.matcher.is_match(url).unwrap_or(false)
    }

    fn is_native(&self) -> bool {
        true
    }

    fn native_matcher_count(&self) -> usize {
        1
    }

    fn extract_with_context(
        &self,
        url: &str,
        context: &ExtractionContext,
    ) -> Result<ExtractorResult, ExtractorError> {
        let post_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "GETTR post has no ID")
            })?;
        let webpage_response = context.get(url)?;
        let webpage = String::from_utf8_lossy(webpage_response.body());
        let api_data = gettr_api_result(context, &format!("post/{post_id}"), Some((
            "incl",
            "\"poststats|userinfo\"",
        )))?;
        let post_data = api_data.get("data").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("GETTR post {post_id} has no data"),
            )
        })?;

        if gettr_string(post_data, "p_type").as_deref() == Some("stream") {
            return Ok(ExtractorResult::Redirect {
                url: format!("https://gettr.com/streaming/{post_id}"),
                ie_key: Some("GettrStreaming".to_owned()),
            });
        }

        let vid = gettr_string(post_data, "vid");
        let ovid = gettr_string(post_data, "ovid");
        if vid.is_none() && ovid.is_none() {
            if let Some(embed_url) = gettr_string(post_data, "prevsrc") {
                return Ok(ExtractorResult::Redirect {
                    url: embed_url,
                    ie_key: None,
                });
            }
            if let Some(shared_post_id) = gettr_shared_post_id(&api_data, post_data) {
                return Ok(ExtractorResult::Redirect {
                    url: format!("https://gettr.com/post/{shared_post_id}"),
                    ie_key: Some("Gettr".to_owned()),
                });
            }
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("GETTR post {post_id} has no video"),
            ));
        }

        let description = gettr_string(post_data, "txt")
            .or_else(|| html_meta_value(&webpage, "og:description"));
        let uploader = gettr_user_data(&api_data, post_data)
            .and_then(|user| {
                gettr_string(user, "nickname").or_else(|| gettr_string(user, "username"))
            })
            .or_else(|| gettr_page_uploader(&webpage));
        let title = description.clone().unwrap_or_else(|| post_id.clone());
        let title = uploader
            .as_ref()
            .map_or(title.clone(), |uploader| format!("{uploader} - {title}"));
        let mut formats = Vec::new();
        if let Some(vid) = vid {
            formats.push(serde_json::json!({
                "url": gettr_media_url(&vid),
                "format_id": "hls",
                "protocol": "m3u8_native",
                "ext": "mp4",
            }));
        }
        if let Some(ovid) = ovid {
            let mut format = serde_json::Map::new();
            format.insert("url".to_owned(), serde_json::json!(gettr_media_url(&ovid)));
            format.insert("format_id".to_owned(), serde_json::json!("ovid"));
            format.insert("ext".to_owned(), serde_json::json!("mp4"));
            if let Some(width) = json_i64(post_data, "vid_wid") {
                format.insert("width".to_owned(), serde_json::json!(width));
            }
            if let Some(height) = json_i64(post_data, "vid_hgt") {
                format.insert("height".to_owned(), serde_json::json!(height));
            }
            formats.push(serde_json::Value::Object(format));
        }
        let first_format = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(post_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", description);
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert(
            "url",
            first_format
                .get("url")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
        info.insert(
            "ext",
            first_format
                .get("ext")
                .cloned()
                .unwrap_or_else(|| serde_json::json!("mp4")),
        );
        info.insert_if_some("uploader", uploader);
        info.insert_if_some(
            "uploader_id",
            gettr_user_data(&api_data, post_data)
                .and_then(|user| gettr_string(user, "_id").or_else(|| gettr_string(user, "username")))
                .or_else(|| gettr_string(post_data, "uid")),
        );
        info.insert_if_some(
            "thumbnail",
            gettr_string(post_data, "main")
                .map(|value| gettr_media_url(&value))
                .or_else(|| {
                    html_meta_value(&webpage, "og:image")
                        .map(|value| resolve_url(url, &value))
                }),
        );
        info.insert_if_some(
            "timestamp",
            gettr_number(post_data, "cdate")
                .or_else(|| gettr_number(post_data, "udate"))
                .map(|value| value / 1000.0),
        );
        info.insert_if_some("duration", gettr_number(post_data, "vid_dur"));
        if let Some(tags) = post_data.get("htgs").and_then(serde_json::Value::as_array) {
            info.insert("tags", serde_json::Value::Array(tags.clone()));
        }
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}

/// Native GETTR live-stream extractor.
pub struct GettrStreamingExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl GettrStreamingExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for GettrStreamingExtractor {
    fn descriptor(&self) -> &ExtractorDescriptor {
        &self.descriptor
    }

    fn suitable(&self, url: &str) -> bool {
        self.matcher.is_match(url).unwrap_or(false)
    }

    fn is_native(&self) -> bool {
        true
    }

    fn native_matcher_count(&self) -> usize {
        1
    }

    fn extract_with_context(
        &self,
        url: &str,
        context: &ExtractionContext,
    ) -> Result<ExtractorResult, ExtractorError> {
        let video_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "GETTR streaming URL has no ID",
                )
            })?;
        let video_info = gettr_api_result(
            context,
            &format!("live/join/{video_id}"),
            None,
        )?;
        let live_info = video_info.get("broadcast").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("GETTR stream {video_id} has no broadcast data"),
            )
        })?;
        let live_url = gettr_string(live_info, "url");
        let mut formats = Vec::new();
        if let Some(live_url) = live_url {
            formats.push(serde_json::json!({
                "url": live_url,
                "format_id": "hls",
                "protocol": "m3u8_native",
                "ext": "mp4",
            }));
        }
        let first_format = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some(
            "url",
            first_format.get("url").and_then(serde_json::Value::as_str),
        );
        let post_data = video_info.get("postData");
        info.insert_if_some(
            "title",
            post_data.and_then(|post| gettr_string(post, "ttl")),
        );
        info.insert_if_some(
            "description",
            post_data.and_then(|post| gettr_string(post, "dsc")),
        );
        info.insert_if_some(
            "uploader",
            video_info
                .get("liveHostInfo")
                .and_then(|host| gettr_string(host, "nickname")),
        );
        info.insert_if_some(
            "uploader_id",
            video_info
                .get("liveHostInfo")
                .and_then(|host| gettr_string(host, "_id")),
        );
        info.insert_if_some("view_count", json_i64(live_info, "viewsCount"));
        info.insert_if_some(
            "timestamp",
            gettr_number(live_info, "startAt").map(|value| value / 1000.0),
        );
        info.insert_if_some(
            "duration",
            gettr_number(live_info, "duration").map(|value| value / 1000.0),
        );
        info.insert_if_some("is_live", json_bool(live_info, "isLive"));
        if let Some(images) = post_data
            .and_then(|post| post.get("imgs"))
            .and_then(serde_json::Value::as_array)
        {
            let thumbnails = images
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(|value| serde_json::json!({ "url": gettr_media_url(value) }))
                .collect::<Vec<_>>();
            info.insert("thumbnails", serde_json::Value::Array(thumbnails));
        }
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}

fn gettr_api_result(
    context: &ExtractionContext,
    endpoint: &str,
    query: Option<(&str, &str)>,
) -> Result<serde_json::Value, ExtractorError> {
    let mut request = Request::new(format!("{GETTR_API_BASE}{endpoint}"));
    if let Some((key, value)) = query {
        request.update_query(&[(key.to_owned(), value.to_owned())]);
    } else if endpoint.starts_with("live/join/") {
        request.set_method("POST").map_err(map_request_error)?;
        request.set_data(Some(Vec::new()));
    }
    request.headers_mut().set("Accept", "application/json");
    let response = context.request(&request)?;
    let envelope = serde_json::from_slice::<serde_json::Value>(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid GETTR API JSON for {endpoint}: {error}"),
        )
    })?;
    envelope.get("result").cloned().ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("GETTR API response for {endpoint} has no result"),
        )
    })
}

fn gettr_string(value: &serde_json::Value, key: &str) -> Option<String> {
    json_string(value, key)
        .map(str::to_owned)
        .filter(|value| !value.is_empty())
}

fn gettr_number(value: &serde_json::Value, key: &str) -> Option<f64> {
    value.get(key).and_then(|value| {
        value
            .as_f64()
            .or_else(|| value.as_i64().map(|value| value as f64))
            .or_else(|| value.as_u64().map(|value| value as f64))
            .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
    })
}

fn gettr_media_url(value: &str) -> String {
    resolve_url(GETTR_MEDIA_BASE, value)
}

fn gettr_user_data<'a>(
    api_data: &'a serde_json::Value,
    post_data: &serde_json::Value,
) -> Option<&'a serde_json::Value> {
    let uid = json_string(post_data, "uid")?;
    api_data
        .get("aux")
        .and_then(|aux| aux.get("uinf"))
        .and_then(|users| users.get(uid))
}

fn gettr_shared_post_id(
    api_data: &serde_json::Value,
    post_data: &serde_json::Value,
) -> Option<String> {
    api_data
        .get("aux")
        .and_then(|aux| aux.get("shrdpst"))
        .and_then(|shared| gettr_string(shared, "_id"))
        .or_else(|| {
            post_data
                .get("rpstIds")
                .and_then(serde_json::Value::as_array)
                .and_then(|ids| ids.first())
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
}

fn gettr_page_uploader(html: &str) -> Option<String> {
    let page_title = html_meta_value(html, "og:title").or_else(|| {
        Regex::new(r#"(?is)<title\b[^>]*>(.*?)</title>"#)
            .ok()
            .and_then(|matcher| matcher.captures(html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| html_text_fragment(value.as_str()))
    })?;
    Regex::new(r#"(?is)^(.+?)\s+on\s+GETTR"#)
        .ok()
        .and_then(|matcher| matcher.captures(&page_title).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().trim().to_owned())
        .filter(|value| !value.is_empty())
}
