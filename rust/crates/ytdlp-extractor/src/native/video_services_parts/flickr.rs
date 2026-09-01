/// Native Flickr video REST/API extractor.
pub struct FlickrExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FlickrExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FlickrExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Flickr URL has no photo ID")
            })?;
        let api_key = context
            .get_json("https://www.flickr.com/hermes_error_beacon.gne")?
            .get("site_key")
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Flickr photo {video_id} has no API key"),
                )
            })?;
        let video_info = flickr_api_call(
            context,
            "photos.getInfo",
            &video_id,
            &api_key,
            None,
        )?
        .get("photo")
        .cloned()
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Flickr photo {video_id} has no photo data"),
            )
        })?;
        if json_string(&video_info, "media") != Some("video") {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Flickr photo {video_id} is not a video"),
            ));
        }
        let title = flickr_nested_string(&video_info, "title", "_content").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Flickr video {video_id} has no title"),
            )
        })?;
        let secret = flickr_nested_string(&video_info, "secret", "_content");
        let streams_data = flickr_api_call(
            context,
            "video.getStreamInfo",
            &video_id,
            &api_key,
            secret.as_deref(),
        )?;
        let streams = streams_data
            .get("streams")
            .and_then(|streams| streams.get("stream"))
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Flickr video {video_id} has no stream list"),
                )
            })?;
        let mut formats = Vec::new();
        for stream in streams {
            let Some(media_url) = stream
                .get("_content")
                .and_then(serde_json::Value::as_str)
                .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            else {
                continue;
            };
            let format_id = flickr_nested_string(stream, "type", "_content")
                .unwrap_or_else(|| "http".to_owned());
            let quality = [
                "288p",
                "iphone_wifi",
                "100",
                "300",
                "700",
                "360p",
                "appletv",
                "720p",
                "1080p",
                "orig",
            ]
            .iter()
            .position(|value| *value == format_id)
            .map(|value| value as i64)
            .unwrap_or(-1);
            formats.push(serde_json::json!({
                "format_id": format_id,
                "url": media_url,
                "quality": quality,
                "ext": yt_dlp_core::determine_ext(Some(media_url), "mpg"),
            }));
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Flickr video {video_id} has no playable streams"),
            ));
        }
        let first_format = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let owner = video_info
            .get("owner")
            .unwrap_or(&serde_json::Value::Null);
        let uploader_id = json_string(owner, "nsid").map(str::to_owned);
        let uploader_path = json_string(owner, "path_alias")
            .or(uploader_id.as_deref())
            .map(str::to_owned);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some(
            "description",
            flickr_nested_string(&video_info, "description", "_content"),
        );
        info.insert_if_some(
            "timestamp",
            json_i64(&video_info, "dateuploaded"),
        );
        info.insert_if_some(
            "duration",
            video_info
                .get("video")
                .and_then(|video| json_i64(video, "duration")),
        );
        info.insert_if_some("uploader_id", uploader_id);
        info.insert_if_some("uploader", json_string(owner, "realname"));
        info.insert_if_some(
            "uploader_url",
            uploader_path.map(|path| format!("https://www.flickr.com/photos/{path}/")),
        );
        info.insert_if_some(
            "comment_count",
            video_info
                .get("comments")
                .and_then(|comments| json_i64(comments, "_content")),
        );
        info.insert_if_some("view_count", json_i64(&video_info, "views"));
        info.insert(
            "tags",
            serde_json::Value::Array(
                video_info
                    .get("tags")
                    .and_then(|tags| tags.get("tag"))
                    .and_then(serde_json::Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|tag| tag.get("_content").and_then(serde_json::Value::as_str))
                    .map(|tag| serde_json::json!(tag))
                    .collect(),
            ),
        );
        info.insert_if_some(
            "license",
            json_string(&video_info, "license").and_then(flickr_license),
        );
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
                .unwrap_or_else(|| serde_json::json!("mpg")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}

fn flickr_api_call(
    context: &ExtractionContext,
    method: &str,
    photo_id: &str,
    api_key: &str,
    secret: Option<&str>,
) -> Result<serde_json::Value, ExtractorError> {
    let mut query = vec![
        ("photo_id".to_owned(), photo_id.to_owned()),
        ("method".to_owned(), format!("flickr.{method}")),
        ("api_key".to_owned(), api_key.to_owned()),
        ("format".to_owned(), "json".to_owned()),
        ("nojsoncallback".to_owned(), "1".to_owned()),
    ];
    if let Some(secret) = secret {
        query.push(("secret".to_owned(), secret.to_owned()));
    }
    let mut request = Request::new("https://api.flickr.com/services/rest");
    request.update_query(&query);
    let response = context.request(&request)?;
    let data = serde_json::from_slice::<serde_json::Value>(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Flickr {method} JSON for {photo_id}: {error}"),
        )
    })?;
    if json_string(&data, "stat") != Some("ok") {
        let message = json_string(&data, "message").unwrap_or("Flickr API request failed");
        return Err(ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Flickr {method} for {photo_id}: {message}"),
        ));
    }
    Ok(data)
}

fn flickr_nested_string(
    value: &serde_json::Value,
    object_key: &str,
    value_key: &str,
) -> Option<String> {
    value
        .get(object_key)
        .and_then(|object| json_string(object, value_key))
        .map(str::to_owned)
}

fn flickr_license(value: &str) -> Option<String> {
    Some(
        match value {
            "0" => "All Rights Reserved",
            "1" => "Attribution-NonCommercial-ShareAlike",
            "2" => "Attribution-NonCommercial",
            "3" => "Attribution-NonCommercial-NoDerivs",
            "4" => "Attribution",
            "5" => "Attribution-ShareAlike",
            "6" => "Attribution-NoDerivs",
            "7" => "No known copyright restrictions",
            "8" => "United States government work",
            "9" => "Public Domain Dedication (CC0)",
            "10" => "Public Domain Work",
            _ => return None,
        }
        .to_owned(),
    )
}
