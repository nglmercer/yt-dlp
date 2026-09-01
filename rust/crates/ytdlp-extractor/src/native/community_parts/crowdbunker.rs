/// Native CrowdBunker video API and channel pagination extractors.
pub struct CrowdBunkerExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

pub struct CrowdBunkerChannelExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl CrowdBunkerExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl CrowdBunkerChannelExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for CrowdBunkerExtractor {
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
                    "CrowdBunker URL has no video ID",
                )
            })?;
        let data = crowdbunker_api_json(
            context,
            &format!("https://api.divulg.org/post/{video_id}/details"),
            &[],
        )?;
        let video = data.get("video").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("CrowdBunker video {video_id} has no video data"),
            )
        })?;
        let mut formats = Vec::new();
        if let Some(manifest_url) = crowdbunker_manifest_url(video, "dashManifest") {
            formats.push(serde_json::json!({
                "url": manifest_url,
                "format_id": "dash",
                "protocol": "http_dash_segments",
                "ext": "mp4",
            }));
        }
        if let Some(manifest_url) = crowdbunker_manifest_url(video, "hlsManifest") {
            formats.push(serde_json::json!({
                "url": manifest_url,
                "format_id": "hls",
                "protocol": "m3u8_native",
                "ext": "mp4",
            }));
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("CrowdBunker video {video_id} has no playable manifests"),
            ));
        }
        let mut subtitles = serde_json::Map::new();
        if let Some(captions) = video.get("captions").and_then(serde_json::Value::as_array) {
            for caption in captions {
                let Some(caption_url) = caption
                    .get("file")
                    .and_then(|file| json_string(file, "url"))
                    .filter(|value| {
                        value.starts_with("http://") || value.starts_with("https://")
                    })
                else {
                    continue;
                };
                let language = json_string(caption, "languageCode").unwrap_or("fr");
                crowdbunker_add_subtitle(&mut subtitles, language, caption_url);
            }
        }
        let thumbnails = video
            .get("thumbnails")
            .and_then(serde_json::Value::as_array)
            .map(|images| {
                images
                    .iter()
                    .filter_map(|image| {
                        let image_url = json_string(image, "url").filter(|value| {
                            value.starts_with("http://") || value.starts_with("https://")
                        })?;
                        let mut thumbnail = serde_json::Map::new();
                        thumbnail.insert("url".to_owned(), serde_json::json!(image_url));
                        if let Some(width) = json_i64(image, "width") {
                            thumbnail.insert("width".to_owned(), serde_json::json!(width));
                        }
                        if let Some(height) = json_i64(image, "height") {
                            thumbnail.insert("height".to_owned(), serde_json::json!(height));
                        }
                        Some(serde_json::Value::Object(thumbnail))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let first_url = formats
            .first()
            .and_then(|format| format.get("url"))
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let mut output = InfoDict::new();
        output.insert("id", serde_json::json!(video_id));
        output.insert_if_some("title", json_string(video, "title"));
        output.insert_if_some("description", json_string(video, "description"));
        output.insert_if_some("view_count", json_i64(video, "viewCount"));
        output.insert_if_some("duration", json_i64(video, "duration"));
        output.insert_if_some(
            "uploader",
            data.get("channel")
                .and_then(|channel| json_string(channel, "name")),
        );
        output.insert_if_some(
            "uploader_id",
            data.get("channel")
                .and_then(|channel| json_string(channel, "id")),
        );
        output.insert_if_some("like_count", json_i64(&data, "likesCount"));
        output.insert_if_some(
            "upload_date",
            json_string(video, "publishedAt")
                .or_else(|| json_string(video, "createdAt"))
                .and_then(date_digits),
        );
        output.insert("thumbnails", serde_json::Value::Array(thumbnails));
        output.insert("url", first_url);
        output.insert("ext", serde_json::json!("mp4"));
        output.insert("formats", serde_json::Value::Array(formats));
        output.insert("subtitles", serde_json::Value::Object(subtitles));
        Ok(ExtractorResult::single(output))
    }
}

impl InfoExtractor for CrowdBunkerChannelExtractor {
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
        let channel_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "CrowdBunker channel URL has no channel ID",
                )
            })?;
        let endpoint = format!("https://api.divulg.org/organization/{channel_id}/posts");
        let mut after: Option<String> = None;
        let mut entries = Vec::new();
        loop {
            let query = after
                .as_ref()
                .map(|cursor| vec![("after".to_owned(), cursor.clone())])
                .unwrap_or_default();
            let page = crowdbunker_api_json(context, &endpoint, &query)?;
            if let Some(items) = page.get("items").and_then(serde_json::Value::as_array) {
                for item in items {
                    let Some(video_id) = json_string(item, "uid") else {
                        continue;
                    };
                    let mut entry =
                        native_url_result(&format!("https://crowdbunker.com/v/{video_id}"));
                    entry.insert("ie_key", serde_json::json!("CrowdBunker"));
                    entries.push(entry);
                }
            }
            let next_after = json_string(&page, "last").map(str::to_owned);
            if next_after.is_none() {
                break;
            }
            if next_after == after {
                return Err(ExtractorError::new(
                    ExtractorErrorKind::Unsupported,
                    format!(
                        "TODO: CrowdBunker channel {channel_id} returned a non-advancing page cursor"
                    ),
                ));
            }
            after = next_after;
        }
        if entries.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("CrowdBunker channel {channel_id} has no video entries"),
            ));
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(channel_id));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

fn crowdbunker_manifest_url(video: &serde_json::Value, key: &str) -> Option<String> {
    video
        .get(key)
        .and_then(|manifest| json_string(manifest, "url"))
        .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
        .map(str::to_owned)
}

fn crowdbunker_add_subtitle(
    subtitles: &mut serde_json::Map<String, serde_json::Value>,
    language: &str,
    url: &str,
) {
    let entries = subtitles
        .entry(language.to_owned())
        .or_insert_with(|| serde_json::json!([]));
    if let Some(entries) = entries.as_array_mut() {
        entries.push(serde_json::json!({"url": url}));
    }
}

fn crowdbunker_api_json(
    context: &ExtractionContext,
    endpoint: &str,
    query: &[(String, String)],
) -> Result<serde_json::Value, ExtractorError> {
    let mut request = Request::new(endpoint);
    if !query.is_empty() {
        request.update_query(query);
    }
    request
        .headers_mut()
        .set("Accept", "application/json, text/plain, */*");
    let response = context.request(&request)?;
    serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid CrowdBunker API JSON from {}: {error}", response.url()),
        )
    })
}
