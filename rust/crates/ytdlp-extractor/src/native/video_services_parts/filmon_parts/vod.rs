/// Native FilmOn VOD/movie API extractor.
pub struct FilmOnExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FilmOnExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FilmOnExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "FilmOn URL has no movie ID")
            })?;
        let response = context.get_json(&format!(
            "https://www.filmon.com/api/vod/movie?id={video_id}"
        ))?;
        let movie = response.get("response").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("FilmOn movie {video_id} API response has no response object"),
            )
        })?;
        let title = json_string(movie, "title")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("FilmOn movie {video_id} has no title"),
                )
            })?;
        let description = json_string(movie, "description")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned);
        if json_i64(movie, "type_id") == Some(1) {
            let entries = movie
                .get("episodes")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|episode| {
                    json_value_string(Some(episode)).map(|episode_id| {
                        native_url_result(&format!("filmon:{episode_id}"))
                    })
                })
                .collect::<Vec<_>>();
            if entries.is_empty() {
                return Err(ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("FilmOn series {video_id} has no episodes"),
                ));
            }
            let mut info = InfoDict::new();
            info.insert("id", serde_json::json!(video_id));
            info.insert("title", serde_json::json!(title));
            info.insert_if_some("description", description);
            return Ok(ExtractorResult::Playlist { info, entries });
        }
        let formats = filmon_formats_from_object(movie.get("streams"));
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("FilmOn movie {video_id} has no playable streams"),
            ));
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some("description", description);
        info.insert(
            "thumbnails",
            serde_json::Value::Array(filmon_vod_thumbnails(movie.get("poster"))),
        );
        Ok(ExtractorResult::single(info))
    }
}
