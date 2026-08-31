/// Native Slideshare video extractor. The legacy page contains a JSON object
/// assigned to slideshare_object; extracting that object directly avoids a
/// browser or embedded interpreter.
pub struct SlideshareExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl SlideshareExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for SlideshareExtractor {
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
                "Slideshare URL did not match its native pattern",
            )
        })?;
        let page_title = captures
            .name("title")
            .map(|value| value.as_str().to_owned())
            .unwrap_or_else(|| "slideshare".to_owned());
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let payload = json_object_after_marker(&html, "slideshare_object,").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Slideshare page {page_title} has no slideshare_object JSON"),
            )
        })?;
        let slideshow = payload.get("slideshow").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Slideshare object has no slideshow metadata",
            )
        })?;
        let slideshow_type = json_string(slideshow, "type").unwrap_or("unknown");
        if slideshow_type != "video" {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!("TODO: Slideshare slideshow type {slideshow_type:?} is not a video"),
            ));
        }
        let player = payload.get("jsplayer").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Slideshare object has no jsplayer metadata",
            )
        })?;
        let document = json_string(&payload, "doc").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Slideshare object has no document name",
            )
        })?;
        let bucket = json_string(player, "video_bucket").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Slideshare object has no video bucket",
            )
        })?;
        let extension = json_string(player, "video_extension").unwrap_or("mp4");
        let bucket_url =
            url::Url::parse(&proto_relative_url(bucket, "https:")).map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid Slideshare video bucket {bucket:?}: {error}"),
                )
            })?;
        let video_url = bucket_url
            .join(&format!("{document}-SD.{extension}"))
            .map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid Slideshare video path: {error}"),
                )
            })?
            .to_string();
        let slideshow_id = json_value_string(slideshow.get("id")).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Slideshare slideshow has no ID",
            )
        })?;
        let title = json_string(slideshow, "title")
            .map(str::to_owned)
            .unwrap_or(page_title);
        let description = html_element_by_id(&html, "slideshow-description-paragraph")
            .map(|value| html_text_fragment(&value))
            .filter(|value| !value.is_empty())
            .or_else(|| {
                Regex::new(r#"(?is)<p[^>]*\bitemprop\s*=\s*["']description["'][^>]*>(.*?)</p>"#)
                    .ok()
                    .and_then(|matcher| matcher.captures(&html).ok().flatten())
                    .and_then(|captures| captures.get(1))
                    .map(|value| html_text_fragment(value.as_str()))
                    .filter(|value| !value.is_empty())
            });
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(slideshow_id));
        info.insert("title", serde_json::json!(title));
        info.insert("url", serde_json::json!(video_url));
        info.insert("ext", serde_json::json!(extension));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": video_url,
                "format_id": "sd",
                "ext": extension,
            }]),
        );
        info.insert_if_some("thumbnail", json_string(slideshow, "pin_image_url"));
        info.insert_if_some("description", description);
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}
