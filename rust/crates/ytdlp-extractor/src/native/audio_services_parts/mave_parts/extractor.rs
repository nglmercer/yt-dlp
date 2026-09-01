pub struct MaveExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

pub struct MaveChannelExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MaveExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl MaveChannelExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

fn mave_capture(
    matcher: &Regex,
    url: &str,
    name: &str,
    error: &str,
) -> Result<String, ExtractorError> {
    matcher
        .captures(url)
        .ok()
        .flatten()
        .and_then(|captures| captures.name(name))
        .map(|value| value.as_str().to_owned())
        .ok_or_else(|| ExtractorError::new(ExtractorErrorKind::InvalidUrl, error))
}

impl InfoExtractor for MaveExtractor {
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
        let channel_id = mave_capture(
            &self.matcher,
            url,
            "channel_id",
            "Mave URL has no channel ID",
        )?;
        let episode_code = mave_capture(
            &self.matcher,
            url,
            "episode_code",
            "Mave URL has no episode code",
        )?;
        let channel_meta = mave_channel_meta(context, &channel_id)?;
        let episode_meta = mave_episode_meta(context, &channel_id, &episode_code)?;
        Ok(ExtractorResult::single(mave_episode_entry(
            &channel_id,
            &channel_meta,
            &episode_meta,
        )?))
    }
}

impl InfoExtractor for MaveChannelExtractor {
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
        let channel_id = mave_capture(
            &self.matcher,
            url,
            "id",
            "Mave channel URL has no channel ID",
        )?;
        let channel_meta = mave_channel_meta(context, &channel_id)?;
        let episode_count = mave_integer(channel_meta.get("episodes_count")).unwrap_or_default();
        let page_count = if episode_count <= 0 {
            0
        } else {
            (episode_count + MAVE_PAGE_SIZE - 1) / MAVE_PAGE_SIZE
        };
        let mut entries = Vec::new();
        for page in 0..page_count {
            let page_data = mave_episode_page(context, &channel_id, page)?;
            let Some(episodes) = page_data.get("episodes").and_then(serde_json::Value::as_array)
            else {
                continue;
            };
            for episode in episodes {
                if episode.get("audio").is_none() || episode.get("audio") == Some(&serde_json::Value::Null) {
                    continue;
                }
                if episode.get("id").is_none() {
                    continue;
                }
                entries.push(mave_episode_entry(&channel_id, &channel_meta, episode)?);
            }
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(channel_id));
        info.insert("webpage_url", serde_json::json!(url));
        info.insert_if_some("title", json_string(&channel_meta, "title"));
        info.insert_if_some("description", json_string(&channel_meta, "description"));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}
