/// Native Library of Congress media/API extractor.
pub struct LibraryOfCongressExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl LibraryOfCongressExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for LibraryOfCongressExtractor {
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
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Library of Congress URL has no item ID",
                )
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let media_id = loc_media_id(&webpage)?;
        let data = loc_media_object(context, &media_id)?;
        let derivative = data
            .get("derivatives")
            .and_then(serde_json::Value::as_array)
            .and_then(|derivatives| derivatives.first())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Library of Congress media {media_id} has no derivatives"),
                )
            })?;
        let raw_media_url = json_string(derivative, "derivativeUrl").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Library of Congress media {media_id} has no derivative URL"),
            )
        })?;
        let is_video = json_string(&data, "mediaType")
            .unwrap_or("v")
            .eq_ignore_ascii_case("v");
        let media_url = loc_normalize_media_url(raw_media_url, is_video);
        let formats = loc_formats(&webpage, &media_url, is_video);
        let title = json_string(derivative, "shortName")
            .or_else(|| json_string(&data, "shortName"))
            .map(str::to_owned)
            .or_else(|| html_meta_value(&webpage, "og:title"))
            .or_else(|| html_title_value(&webpage))
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Library of Congress media {video_id} has no title"),
                )
            })?;
        let subtitles = json_string(&data, "ccUrl")
            .map(|cc_url| serde_json::json!({"en": [{"url": cc_url, "ext": "ttml"}]}))
            .unwrap_or_else(|| serde_json::json!({}));

        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some(
            "thumbnail",
            html_meta_value(&webpage, "og:image")
                .or_else(|| html_meta_value(&webpage, "twitter:image")),
        );
        info.insert_if_some("duration", json_f64(&data, "duration"));
        info.insert_if_some("view_count", json_i64(&data, "viewCount"));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", subtitles);
        Ok(ExtractorResult::single(info))
    }
}
