pub struct LocoExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl LocoExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for LocoExtractor {
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
            ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Loco URL did not match")
        })?;
        let video_type = captures
            .name("type")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Loco URL has no type")
            })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Loco URL has no ID")
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let stream = loco_page_stream(&webpage, &video_id)?;
        let is_live = video_type == "streamers";
        if let Some(access_token) = loco_access_token(context, &video_id) {
            if let Some(stream_uid) = json_string(&stream, "uid") {
                loco_authorize(context, &video_id, stream_uid, &access_token);
            }
        }
        Ok(ExtractorResult::single(loco_stream_info(
            &stream, &video_id, is_live,
        )?))
    }
}
