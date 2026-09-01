pub struct LifeNewsExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

pub struct LifeEmbedExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl LifeNewsExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl LifeEmbedExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

fn lifenews_article_entry(
    video_id: &str,
    media_url: &str,
    index: Option<usize>,
    metadata: &LifeNewsMetadata,
    transparent: bool,
) -> InfoDict {
    let entry_id = index.map_or_else(
        || video_id.to_owned(),
        |index| format!("{video_id}-video{index}"),
    );
    let title = index.map_or_else(
        || metadata.title.clone(),
        |index| format!("{} (Видео {index})", metadata.title),
    );
    let mut entry = InfoDict::new();
    if transparent {
        entry.insert("_type", serde_json::json!("url_transparent"));
    }
    entry.insert("id", serde_json::json!(entry_id));
    entry.insert("url", serde_json::json!(media_url));
    entry.insert("title", serde_json::json!(title));
    entry.insert("description", serde_json::json!(metadata.description.clone()));
    entry.insert_if_some("view_count", metadata.view_count);
    entry.insert_if_some("timestamp", metadata.timestamp);
    if !transparent {
        entry.insert(
            "ext",
            serde_json::json!(yt_dlp_core::determine_ext(Some(media_url), "mp4")),
        );
    }
    if transparent {
        entry.insert("ie_key", serde_json::json!("LifeEmbed"));
    }
    entry
}

impl InfoExtractor for LifeNewsExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Life.ru URL has no article ID")
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let (video_urls, iframe_links) = lifenews_page_media(url, &webpage);
        if video_urls.is_empty() && iframe_links.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Life.ru article {video_id} has no media links"),
            ));
        }
        let metadata = lifenews_metadata(&webpage, &video_id)?;
        if video_urls.len() == 1 && iframe_links.is_empty() {
            return Ok(ExtractorResult::single(lifenews_article_entry(
                &video_id,
                &video_urls[0],
                None,
                &metadata,
                false,
            )));
        }
        if iframe_links.len() == 1 && video_urls.is_empty() {
            return Ok(ExtractorResult::single(lifenews_article_entry(
                &video_id,
                &iframe_links[0],
                None,
                &metadata,
                true,
            )));
        }
        let mut entries = Vec::with_capacity(video_urls.len() + iframe_links.len());
        for (index, media_url) in video_urls.iter().enumerate() {
            entries.push(lifenews_article_entry(
                &video_id,
                media_url,
                Some(index + 1),
                &metadata,
                false,
            ));
        }
        for (index, media_url) in iframe_links.iter().enumerate() {
            entries.push(lifenews_article_entry(
                &video_id,
                media_url,
                Some(video_urls.len() + index + 1),
                &metadata,
                true,
            ));
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(metadata.title));
        info.insert("description", serde_json::json!(metadata.description));
        info.insert_if_some("view_count", metadata.view_count);
        info.insert_if_some("timestamp", metadata.timestamp);
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

impl InfoExtractor for LifeEmbedExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Life.ru embed URL has no ID")
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let (formats, thumbnail) = lifenews_embed_media(url, &webpage, &video_id)?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(video_id));
        info.insert_if_some("thumbnail", thumbnail);
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}
