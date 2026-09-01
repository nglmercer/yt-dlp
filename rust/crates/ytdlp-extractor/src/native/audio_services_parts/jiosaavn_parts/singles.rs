/// Native JioSaavn song extractor.
pub struct JioSaavnSongExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl JioSaavnSongExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for JioSaavnSongExtractor {
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
        let display_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "JioSaavn song URL has no song ID",
                )
            })?;
        let response = jiosaavn_call_api(context, "song", &display_id, &[])?;
        let item = jiosaavn_first_item(&response, "songs", "song")?;
        let formats = jiosaavn_format_list(context, item)?;
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = jiosaavn_extract_song_info(item, Some(url));
        info.insert("url", first.get("url").cloned().unwrap_or_default());
        info.insert("ext", first.get("ext").cloned().unwrap_or_default());
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}

/// Native JioSaavn podcast/show episode extractor.
pub struct JioSaavnShowExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl JioSaavnShowExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for JioSaavnShowExtractor {
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
        let display_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "JioSaavn show URL has no episode ID",
                )
            })?;
        let response = jiosaavn_call_api(context, "episode", &display_id, &[])?;
        let item = jiosaavn_first_item(&response, "episodes", "episode")?;
        let formats = jiosaavn_format_list(context, item)?;
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = jiosaavn_extract_episode_info(item, Some(url));
        info.insert("url", first.get("url").cloned().unwrap_or_default());
        info.insert("ext", first.get("ext").cloned().unwrap_or_default());
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}
