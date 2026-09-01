/// Native GodTube XML metadata and direct-media extractor.
pub struct GodTubeExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl GodTubeExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for GodTubeExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "GodTube URL has no ID")
            })?;
        let config_response = context.get(&format!(
            "http://www.godtube.com/resource/mediaplayer/{}.xml",
            video_id.to_ascii_lowercase()
        ))?;
        let config = String::from_utf8_lossy(config_response.body());
        let media_url = xml_element_text(&config, "file")
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("GodTube video {video_id} has no media URL"),
                )
            })?;
        let media_title_response =
            context.get(&format!("http://www.godtube.com/media/xml/?v={video_id}"))?;
        let media = String::from_utf8_lossy(media_title_response.body());
        let title = xml_element_text(&media, "title").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("GodTube video {video_id} has no title"),
            )
        })?;
        let extension = yt_dlp_core::determine_ext(Some(&media_url), "mp4");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!(extension.clone()));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "http",
                "ext": extension,
            }]),
        );
        info.insert_if_some("thumbnail", xml_element_text(&config, "image"));
        info.insert_if_some("uploader", xml_element_text(&config, "author"));
        info.insert_if_some(
            "timestamp",
            xml_element_text(&config, "date").and_then(parse_timestamp),
        );
        info.insert_if_some(
            "duration",
            xml_element_text(&config, "duration")
                .and_then(|value| yt_dlp_core::parse_duration(value.trim())),
        );
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}
