/// Native FranceTV site page wrapper.
pub struct FranceTvSiteExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FranceTvSiteExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FranceTvSiteExtractor {
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
                    "FranceTV site URL has no display ID",
                )
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let video_id = francetv_next_options_id(&webpage)
            .or_else(|| francetv_video_id_from_page(&webpage))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("FranceTV page {display_id} has no video ID"),
                )
            })?;
        Ok(francetv_url_result(&video_id, Some(display_id)))
    }
}

/// Native FranceInfo page-to-FranceTV wrapper.
pub struct FranceTvInfoExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FranceTvInfoExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FranceTvInfoExtractor {
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
                    "FranceInfo URL has no display ID",
                )
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        if webpage.to_ascii_lowercase().contains("dailymotion") {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!(
                    "TODO: FranceInfo page {display_id} embeds Dailymotion, whose native \
                     extractor is not ported yet"
                ),
            ));
        }
        let video_id = francetv_video_id_from_page(&webpage).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("FranceInfo page {display_id} has no FranceTV video ID"),
            )
        })?;
        Ok(francetv_url_result(&video_id, Some(display_id)))
    }
}

fn francetv_url_result(video_id: &str, display_id: Option<String>) -> ExtractorResult {
    let video_id = video_id.split('@').next().unwrap_or(video_id);
    let mut info = InfoDict::new();
    info.insert("_type", serde_json::json!("url_transparent"));
    info.insert("url", serde_json::json!(format!("francetv:{video_id}")));
    info.insert("ie_key", serde_json::json!("FranceTV"));
    info.insert_if_some("display_id", display_id);
    ExtractorResult::single(info)
}
