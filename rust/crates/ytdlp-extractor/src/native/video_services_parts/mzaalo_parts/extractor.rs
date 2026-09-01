/// Native Mzaalo movie/original/clip API extractor.
pub struct MzaaloExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MzaaloExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MzaaloExtractor {
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
                "Mzaalo URL did not match its native pattern",
            )
        })?;
        let media_type = captures
            .name("type")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Mzaalo URL has no type")
            })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Mzaalo URL has no ID")
            })?;
        let data = mzaalo_data(context, &media_type, &video_id)?;
        let language = json_string(&data, "language").map(str::to_ascii_lowercase);
        let media_url = mzaalo_http_url(data.get("streamURL")).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Mzaalo video {video_id} has no streamURL"),
            )
        })?;
        let format = mzaalo_hls_format(media_url.clone(), language.as_deref());
        let thumbnails = mzaalo_thumbnails(&data);
        let first_thumbnail = thumbnails
            .as_ref()
            .and_then(|thumbnails| thumbnails.first())
            .and_then(|thumbnail| thumbnail.get("url"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        let categories = match data.get("genre") {
            Some(serde_json::Value::Array(values)) => Some(
                values
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::to_owned)
                    .collect::<Vec<_>>(),
            ),
            Some(serde_json::Value::String(value)) => Some(vec![value.to_owned()]),
            _ => None,
        }
        .filter(|categories| !categories.is_empty());
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", serde_json::Value::Array(vec![format]));
        info.insert("subtitles", mzaalo_subtitles(&data));
        info.insert_if_some("language", language);
        info.insert_if_some("title", json_string(&data, "title"));
        info.insert_if_some("description", json_string(&data, "description"));
        info.insert_if_some("duration", mzaalo_duration(data.get("duration")));
        info.insert_if_some("age_limit", mzaalo_age_limit(data.get("maturity_rating")));
        info.insert_if_some("thumbnails", thumbnails);
        info.insert_if_some("thumbnail", first_thumbnail);
        info.insert_if_some("categories", categories);
        Ok(ExtractorResult::single(info))
    }
}
