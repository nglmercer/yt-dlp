pub struct LoomExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl LoomExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

fn loom_video_metadata(
    data: &serde_json::Value,
    video_id: &str,
) -> Result<InfoDict, ExtractorError> {
    let video = data
        .get("data")
        .and_then(|data| data.get("getVideo"))
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Loom video {video_id} has no GraphQL metadata"),
            )
        })?;
    let typename = json_string(video, "__typename").unwrap_or_default();
    if typename == "VideoPasswordMissingOrIncorrect" {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            format!(
                "TODO: Loom video {video_id} is password-protected; native password parameters are not implemented"
            ),
        ));
    }
    if typename == "PrivateVideo" {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            format!("TODO: Loom video {video_id} is private and requires account authorization"),
        ));
    }
    if typename != "RegularUserVideo" {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Loom video {video_id} returned unsupported GraphQL type {typename}"),
        ));
    }
    Ok(loom_build_metadata(video, video_id))
}

impl InfoExtractor for LoomExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Loom URL has no video ID")
            })?;
        let metadata_data = loom_graphql(context, "GetVideoSSR", &video_id, false)?;
        let mut info = loom_video_metadata(&metadata_data, &video_id)?;
        let source_data = loom_graphql(context, "GetVideoSource", &video_id, false)?;
        let formats = loom_formats(context, &video_id, &info, &source_data)?;
        let duration = info.get_i64("duration");
        let subtitles_data = loom_graphql(context, "FetchVideoTranscript", &video_id, true).ok();
        let chapters_data = loom_graphql(context, "FetchChapters", &video_id, true).ok();
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", loom_subtitles(subtitles_data.as_ref()));
        info.insert_if_some("chapters", loom_chapters(chapters_data.as_ref(), duration));
        Ok(ExtractorResult::single(info))
    }
}
