pub struct LrtStreamExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

pub struct LrtVodExtractor {
    descriptor: ExtractorDescriptor,
    matchers: Vec<Regex>,
}

pub struct LrtRadioExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl LrtStreamExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl LrtVodExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        let mut matchers = Vec::new();
        for pattern in &descriptor.valid_urls {
            matchers.push(compile_source_pattern(pattern).map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid LRT VOD URL pattern: {error}"),
                )
            })?);
        }
        Ok(Self {
            matchers,
            descriptor,
        })
    }
}

impl LrtRadioExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

fn lrt_match_id(
    matcher: &Regex,
    url: &str,
    label: &str,
) -> Result<String, ExtractorError> {
    matcher
        .captures(url)
        .ok()
        .flatten()
        .and_then(|captures| captures.name("id"))
        .map(|value| value.as_str().to_owned())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                format!("LRT {label} URL has no video ID"),
            )
        })
}

fn lrt_vod_id(matchers: &[Regex], url: &str) -> Result<String, ExtractorError> {
    matchers
        .iter()
        .find_map(|matcher| {
            matcher
                .captures(url)
                .ok()
                .flatten()
                .and_then(|captures| captures.name("id"))
                .map(|value| value.as_str().to_owned())
        })
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "LRT VOD URL has no video ID",
            )
        })
}

impl InfoExtractor for LrtStreamExtractor {
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
        let video_id = lrt_match_id(&self.matcher, url, "stream")?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let streams_url = lrt_streams_url(&webpage, &video_id)?;
        let streams = context.get_json(&streams_url)?;
        let stream_urls = lrt_stream_data_urls(&streams)
            .into_iter()
            .map(|value| lrt_media_url(&value, &streams_url))
            .collect::<Vec<_>>();
        let formats = lrt_formats_from_urls(stream_urls, &video_id, "mp4", true)?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert(
            "title",
            serde_json::json!(
                html_meta_value(&webpage, "og:title")
                    .or_else(|| html_title_value(&webpage))
                    .unwrap_or_else(|| video_id.clone())
            ),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("is_live", serde_json::json!(true));
        Ok(ExtractorResult::single(info))
    }
}

impl InfoExtractor for LrtVodExtractor {
    fn descriptor(&self) -> &ExtractorDescriptor {
        &self.descriptor
    }

    fn suitable(&self, url: &str) -> bool {
        self.matchers
            .iter()
            .any(|matcher| matcher.is_match(url).unwrap_or(false))
    }

    fn is_native(&self) -> bool {
        true
    }

    fn native_matcher_count(&self) -> usize {
        self.matchers.len()
    }

    fn extract_with_context(
        &self,
        url: &str,
        context: &ExtractionContext,
    ) -> Result<ExtractorResult, ExtractorError> {
        let video_id = lrt_vod_id(&self.matchers, url)?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let canonical_url = lrt_canonical_url(&webpage).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("LRT VOD page {video_id} has no canonical content URL"),
            )
        })?;
        let media = lrt_fetch_vod_media(context, &video_id, &canonical_url)?;
        let playlist_item = media.get("playlist_item").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("LRT VOD media info {video_id} has no playlist item"),
            )
        })?;
        let mut info = lrt_info_from_playlist_item(playlist_item, &video_id, "mp4", url)?;
        if let Some(media_id) = json_value_string(media.get("id")) {
            info.insert("id", serde_json::json!(media_id));
        }
        if let Some(title) = json_string(&media, "title").map(html_text_fragment) {
            info.insert("title", serde_json::json!(title));
        }
        if let Some(description) = json_string(&media, "content").map(html_text_fragment) {
            info.insert("description", serde_json::json!(description));
        }
        if let Some(date) = json_string(&media, "date").and_then(lrt_timestamp) {
            info.insert("timestamp", serde_json::json!(date));
        }
        info.insert_if_some("tags", lrt_tag_values(media.get("tags")));
        info.insert_if_some("channel", json_string(&media, "channel"));
        Ok(ExtractorResult::single(info))
    }
}

impl InfoExtractor for LrtRadioExtractor {
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
        let captures = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "LRT radio URL did not match")
            })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "LRT radio URL has no recording ID",
                )
            })?;
        let path = captures
            .name("path")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "LRT radio URL has no recording path",
                )
            })?;
        let media = lrt_fetch_radio_media(context, &video_id, &path)?;
        let playlist_item = media.get("playlist_item").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("LRT radio media info {video_id} has no playlist item"),
            )
        })?;
        let mut info = lrt_info_from_playlist_item(playlist_item, &video_id, "m4a", url)?;
        if let Some(title) = json_string(&media, "title").map(html_text_fragment) {
            info.insert("title", serde_json::json!(title));
        }
        if let Some(description) = json_string(&media, "content").map(html_text_fragment) {
            info.insert("description", serde_json::json!(description));
        }
        if let Some(date) = json_string(&media, "date").and_then(lrt_timestamp) {
            info.insert("timestamp", serde_json::json!(date));
        }
        info.insert_if_some("tags", lrt_tag_values(media.get("tags")));
        info.insert_if_some(
            "categories",
            lrt_tag_values(playlist_item.get("category")),
        );
        info.insert_if_some(
            "thumbnail",
            json_string(playlist_item, "image").map(|value| lrt_media_url(value, url)),
        );
        Ok(ExtractorResult::single(info))
    }
}
