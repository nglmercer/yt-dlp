/// Native GMA Network page-to-video redirect extractor.
pub struct GmaNetworkVideoExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
    youtube_matcher: Regex,
    network_matcher: Regex,
}

impl GmaNetworkVideoExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        let youtube_matcher = Regex::new(
            r#"(?i)var\s*YOUTUBE_VIDEO\s*=\s*['\"]+(?P<id>[\w-]+)"#,
        )
        .map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid GMA Network YouTube matcher: {error}"),
            )
        })?;
        let network_matcher = Regex::new(
            r#"(?i)NETWORK_URL\s*=\s*['\"](?P<url>[^'\"]+)"#,
        )
        .map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid GMA Network API matcher: {error}"),
            )
        })?;
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
            youtube_matcher,
            network_matcher,
        })
    }
}

impl InfoExtractor for GmaNetworkVideoExtractor {
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
                "GMA Network URL did not match its native pattern",
            )
        })?;
        let content_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "GMA Network URL has no ID")
            })?;
        let webpage_response = context.get(url)?;
        let webpage = String::from_utf8_lossy(webpage_response.body());
        if let Some(youtube_id) = self
            .youtube_matcher
            .captures(&webpage)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
        {
            return Ok(ExtractorResult::Redirect {
                url: gma_youtube_url(&youtube_id),
                ie_key: Some("Youtube".to_owned()),
            });
        }
        let network_url = self
            .network_matcher
            .captures(&webpage)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("url"))
            .map(|value| resolve_url(url, value.as_str()))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("GMA Network page {content_id} has no API base URL"),
                )
            })?;
        let api_url = format!("{network_url}api/data/content/video/{content_id}");
        let data = context.get_json(&api_url)?;
        if let Some(video_url) = json_string(&data, "video_file") {
            return Ok(ExtractorResult::Redirect {
                url: gma_youtube_url(video_url),
                ie_key: Some("Youtube".to_owned()),
            });
        }
        if let Some(video_url) = json_string(&data, "dailymotion_file") {
            return Ok(ExtractorResult::Redirect {
                url: video_url.to_owned(),
                ie_key: Some("Dailymotion".to_owned()),
            });
        }
        Err(ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            format!(
                "TODO: GMA Network video {content_id} has no YouTube or Dailymotion media target"
            ),
        ))
    }
}

fn gma_youtube_url(value: &str) -> String {
    if value.starts_with("http://") || value.starts_with("https://") {
        value.to_owned()
    } else {
        format!("https://www.youtube.com/watch?v={value}")
    }
}
