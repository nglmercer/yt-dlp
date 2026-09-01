fn khan_video_result(video: &serde_json::Value) -> Result<InfoDict, ExtractorError> {
    let youtube_id = json_string(video, "youtubeId").ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            "Khan Academy video has no YouTube ID",
        )
    })?;
    let mut info = native_url_result(youtube_id);
    info.insert("_type", serde_json::json!("url_transparent"));
    info.insert("id", serde_json::json!(youtube_id));
    info.insert("ie_key", serde_json::json!("Youtube"));
    info.insert_if_some("display_id", json_string(video, "id"));
    info.insert_if_some("title", json_string(video, "translatedTitle"));
    info.insert_if_some("thumbnail", khan_thumbnails(video.get("thumbnailUrls")));
    info.insert_if_some("description", json_string(video, "description"));
    info.insert_if_some("duration", json_i64(video, "duration"));
    info.insert_if_some(
        "timestamp",
        json_string(video, "dateAdded")
            .and_then(|value| parse_timestamp(value.to_owned())),
    );
    info.insert_if_some(
        "upload_date",
        json_string(video, "dateAdded").and_then(date_digits),
    );
    info.insert_if_some("license", json_string(video, "kaUserLicense"));
    info.insert_if_some("creators", khan_string_list(video.get("authorNames")));
    // The target YouTube extractor is still a native TODO. Keeping this
    // transparent result allows the eventual Rust YouTube port to consume it
    // without introducing a Python compatibility path.
    Ok(info)
}

/// Native Khan Academy video-page extractor.
pub struct KhanAcademyExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl KhanAcademyExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for KhanAcademyExtractor {
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
                    "Khan Academy video URL has no display ID",
                )
            })?;
        let content = khan_content(context, &display_id)?;
        let video = content.get("content").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Khan Academy video {display_id} has no content object"),
            )
        })?;
        Ok(ExtractorResult::single(khan_video_result(video)?))
    }
}
