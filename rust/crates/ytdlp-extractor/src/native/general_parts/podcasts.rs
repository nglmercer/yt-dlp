/// Native Megaphone embedded podcast player extractor.
pub struct MegaphoneExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MegaphoneExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MegaphoneExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Megaphone URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let episode = json_object_after_marker(&html, "var episode").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Megaphone episode {video_id} has no embedded JSON"),
            )
        })?;
        let raw_url = json_string(&episode, "mediaUrl").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Megaphone episode {video_id} has no media URL"),
            )
        })?;
        let media_url = proto_relative_url(raw_url, "https:");
        let ext = yt_dlp_core::determine_ext(Some(&media_url), "mp3");
        let title = html_meta_value(&html, "audio:title")
            .or_else(|| html_meta_value(&html, "og:title"))
            .unwrap_or_else(|| video_id.clone());
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!(ext.clone()));
        info.insert_if_some("thumbnail", html_meta_value(&html, "og:image"));
        info.insert_if_some(
            "creators",
            html_meta_value(&html, "audio:artist").map(|value| vec![value]),
        );
        info.insert_if_some("duration", json_f64(&episode, "duration"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "audio",
                "ext": ext,
                "vcodec": "none",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native Hypem track extractor. Track metadata is embedded in the page and
/// the service's source endpoint returns the final audio URL.
pub struct HypemExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl HypemExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for HypemExtractor {
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
        let page_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Hypem URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let display_data = html_script_json(&html, "displayList-data")?;
        let track = display_data
            .get("tracks")
            .and_then(serde_json::Value::as_array)
            .and_then(|tracks| tracks.first())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Hypem track {page_id} has no embedded track data"),
                )
            })?;
        let track_id = json_value_string(track.get("id")).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Hypem track {page_id} has no source ID"),
            )
        })?;
        let key = json_string(track, "key").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Hypem track {track_id} has no source key"),
            )
        })?;
        let source = native_get_json_with_headers(
            context,
            &format!("http://hypem.com/serve/source/{track_id}/{key}"),
            &[("Content-Type", "application/json")],
        )?;
        let media_url = json_string(&source, "url").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Hypem source {track_id} has no audio URL"),
            )
        })?;
        let title = json_string(track, "song").unwrap_or(&track_id).to_owned();
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(track_id));
        info.insert("title", serde_json::json!(title.clone()));
        info.insert("track", serde_json::json!(title));
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!("mp3"));
        info.insert_if_some("uploader", json_string(track, "artist"));
        info.insert_if_some("duration", json_i64(track, "time"));
        info.insert_if_some("timestamp", json_i64(track, "ts"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "audio",
                "ext": "mp3",
                "vcodec": "none",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native QingTing podcast program extractor.
pub struct QingTingExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl QingTingExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for QingTingExtractor {
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
                "QingTing URL did not match its native pattern",
            )
        })?;
        let channel_id = captures
            .name("channel")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "QingTing URL has no channel",
                )
            })?;
        let program_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "QingTing URL has no program",
                )
            })?;
        let page_url = format!("https://m.qtfm.cn/vchannels/{channel_id}/programs/{program_id}/");
        let webpage = context.get(&page_url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let stores = json_object_after_marker(&html, "window.__initStores").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("QingTing program {program_id} has no store data"),
            )
        })?;
        let program_store = stores
            .get("ProgramStore")
            .unwrap_or(&serde_json::Value::Null);
        let program_info = program_store
            .get("programInfo")
            .unwrap_or(&serde_json::Value::Null);
        let channel_info = program_store
            .get("channelInfo")
            .unwrap_or(&serde_json::Value::Null);
        let podcaster = program_store
            .get("podcasterInfo")
            .and_then(|value| value.get("podcaster"))
            .unwrap_or(&serde_json::Value::Null);
        let media_url = json_string(program_info, "audioUrl").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("QingTing program {program_id} has no audio URL"),
            )
        })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(program_id));
        info.insert_if_some("title", json_string(program_info, "title"));
        info.insert("channel_id", serde_json::json!(channel_id));
        info.insert_if_some("channel", json_string(channel_info, "title"));
        info.insert_if_some("uploader", json_string(podcaster, "nickname"));
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!("m4a"));
        info.insert("vcodec", serde_json::json!("none"));
        info.insert("acodec", serde_json::json!("m4a"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "audio",
                "ext": "m4a",
                "vcodec": "none",
                "acodec": "m4a",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}
