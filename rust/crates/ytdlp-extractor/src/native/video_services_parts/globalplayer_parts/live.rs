/// Native Global Player live station extractor.
pub struct GlobalPlayerLiveExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl GlobalPlayerLiveExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for GlobalPlayerLiveExtractor {
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
                    "Global Player live URL has no station ID",
                )
            })?;
        let props = globalplayer_page_props(url, &video_id, context)?;
        let station = props.get("station").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Global Player station {video_id} is missing"),
            )
        })?;
        let station_id = globalplayer_value_string(station.get("id")).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Global Player station {video_id} has no playable ID"),
            )
        })?;
        let audio_url = globalplayer_playable(context, &station_id, &video_id)?;
        let mut info = globalplayer_audio_info(&station_id, &audio_url, station);
        info.insert("is_live", serde_json::json!(true));
        info.insert("ext", serde_json::json!("aac"));
        if let Some(formats) = info.get("formats").and_then(serde_json::Value::as_array) {
            let formats = formats
                .iter()
                .map(|format| {
                    let mut format = format.clone();
                    format["ext"] = serde_json::json!("aac");
                    format
                })
                .collect();
            info.insert("formats", serde_json::Value::Array(formats));
        }
        info.insert_if_some("thumbnail", globalplayer_url(station.get("brandLogo")));
        info.insert_if_some("description", globalplayer_string(station, "tagline"));
        info.insert_if_some("title", globalplayer_string(station, "name"));
        Ok(ExtractorResult::single(info))
    }
}

/// Native Global Player live-playlist stream extractor.
pub struct GlobalPlayerLivePlaylistExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl GlobalPlayerLivePlaylistExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for GlobalPlayerLivePlaylistExtractor {
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
                    "Global Player playlist URL has no ID",
                )
            })?;
        let props = globalplayer_page_props(url, &video_id, context)?;
        let playlist = props.get("playlistData").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Global Player playlist {video_id} is missing"),
            )
        })?;
        let stream_url = globalplayer_url(playlist.get("streamUrl")).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Global Player playlist {video_id} has no stream URL"),
            )
        })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("url", serde_json::json!(stream_url.clone()));
        info.insert("ext", serde_json::json!("aac"));
        info.insert("vcodec", serde_json::json!("none"));
        info.insert("is_live", serde_json::json!(true));
        info.insert(
            "formats",
            serde_json::Value::Array(vec![globalplayer_format(&stream_url, "aac", true)]),
        );
        globalplayer_insert_meta(&mut info, playlist);
        Ok(ExtractorResult::single(info))
    }
}
