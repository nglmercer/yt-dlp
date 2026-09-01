/// Native LEGO media-player API extractor.
pub struct LegoExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl LegoExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for LegoExtractor {
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
            ExtractorError::new(ExtractorErrorKind::InvalidUrl, "LEGO URL did not match")
        })?;
        let locale = captures
            .name("locale")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "LEGO URL has no locale")
            })?;
        let source_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "LEGO URL has no video ID")
            })?;
        let item = lego_item(context, &source_id, &locale)?;
        let video = item.get("Video").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("LEGO video {source_id} has no Video object"),
            )
        })?;
        let video_id = json_string(video, "Id")
            .filter(|value| !value.is_empty())
            .unwrap_or(&source_id)
            .to_owned();
        let formats = lego_formats(&item, &video_id)?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(video, "Title"));
        info.insert_if_some("description", json_string(video, "Description"));
        info.insert_if_some(
            "thumbnail",
            json_string(video, "GeneratedCoverImage")
                .or_else(|| json_string(video, "GeneratedThumbnail")),
        );
        info.insert_if_some("duration", json_i64(video, "Length"));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", lego_subtitles(video, &locale));
        info.insert_if_some("age_limit", json_i64(video, "AgeFrom"));
        info.insert_if_some("season", json_string(video, "SeasonTitle"));
        info.insert_if_some("season_number", json_i64(video, "Season"));
        info.insert_if_some("episode_number", json_i64(video, "Episode"));
        Ok(ExtractorResult::single(info))
    }
}
