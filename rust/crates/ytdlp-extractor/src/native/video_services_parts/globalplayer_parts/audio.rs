/// Native Global Player podcast/radio-catchup playlist extractor.
pub struct GlobalPlayerAudioExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl GlobalPlayerAudioExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for GlobalPlayerAudioExtractor {
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
        let captures = self.matcher.captures(url).ok().flatten().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "Global Player audio URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Global Player audio URL has no ID",
                )
            })?;
        let path = captures
            .name("path")
            .map(|value| value.as_str().to_owned())
            .unwrap_or_default();
        let is_podcast = captures.name("podcast").is_some();
        let props = globalplayer_page_props(url, &video_id, context)?;
        let container = if is_podcast {
            props.get("podcastInfo")
        } else {
            props
                .get("catchupShow")
                .or_else(|| props.get("catchupInfo"))
        }
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Global Player audio collection {video_id} is missing"),
            )
        })?;
        let metadata = container.get("metadata").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Global Player audio collection {video_id} has no metadata"),
            )
        })?;
        let blocks = container
            .get("blocks")
            .and_then(serde_json::Value::as_array)
            .and_then(|blocks| blocks.get(1))
            .and_then(|block| block.get("items"))
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Global Player audio collection {video_id} has no episodes"),
                )
            })?;
        let mut entries = Vec::new();
        for block in blocks {
            let entry_id = globalplayer_value_string(block.get("id")).ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Global Player collection {video_id} has an episode without an ID"),
                )
            })?;
            let audio_url = globalplayer_playable(context, &entry_id, &video_id)?;
            let mut entry = globalplayer_audio_info(&entry_id, &audio_url, block);
            entry.insert(
                "webpage_url",
                serde_json::json!(format!(
                    "https://www.globalplayer.com/{path}episodes/{entry_id}"
                )),
            );
            entry.insert(
                "extractor",
                serde_json::json!("GlobalPlayerAudioEpisode"),
            );
            entry.insert(
                "extractor_key",
                serde_json::json!("GlobalPlayerAudioEpisode"),
            );
            entries.push(entry);
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        globalplayer_insert_meta(&mut info, metadata);
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

/// Native Global Player podcast/catchup episode extractor.
pub struct GlobalPlayerAudioEpisodeExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl GlobalPlayerAudioEpisodeExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for GlobalPlayerAudioEpisodeExtractor {
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
        let captures = self.matcher.captures(url).ok().flatten().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "Global Player episode URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Global Player episode URL has no ID",
                )
            })?;
        let props = globalplayer_page_props(url, &video_id, context)?;
        let meta = if captures.name("podcast").is_some() {
            props.get("podcastEpisode")
        } else {
            props.get("catchupEpisode")
        }
        .and_then(|episode| episode.get("metadata"))
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Global Player episode {video_id} has no metadata"),
            )
        })?;
        let audio_url = globalplayer_playable(context, &video_id, &video_id)?;
        Ok(ExtractorResult::single(globalplayer_audio_info(
            &video_id,
            &audio_url,
            meta,
        )))
    }
}
