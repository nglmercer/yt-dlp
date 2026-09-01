/// Native Culture Unplugged movie JSON/direct-media extractor.
pub struct CultureUnpluggedExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl CultureUnpluggedExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for CultureUnpluggedExtractor {
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
                "Culture Unplugged URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Culture Unplugged URL has no movie ID",
                )
            })?;
        let display_id = captures
            .name("display_id")
            .map(|value| value.as_str().to_owned())
            .unwrap_or_else(|| video_id.clone());
        let movie_data =
            context.get_json(&format!("http://www.cultureunplugged.com/movie-data/cu-{video_id}.json"))?;
        let media_url = json_string(&movie_data, "url")
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            .map(str::to_owned)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Culture Unplugged movie {video_id} has no media URL"),
                )
            })?;
        let title = json_string(&movie_data, "title")
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Culture Unplugged movie {video_id} has no title"),
                )
            })?;
        let extension = yt_dlp_core::determine_ext(Some(&media_url), "mp4");
        let protocol = if extension.eq_ignore_ascii_case("m3u8") {
            "m3u8_native"
        } else {
            "http"
        };
        let formats = serde_json::json!([{
            "url": media_url,
            "format_id": protocol,
            "protocol": protocol,
            "ext": extension,
        }]);
        let thumbnails = ["small", "large"]
            .into_iter()
            .enumerate()
            .filter_map(|(preference, size)| {
                let thumbnail_url = json_string(&movie_data, &format!("{size}_thumb"))
                    .filter(|value| {
                        value.starts_with("http://") || value.starts_with("https://")
                    })?;
                Some(serde_json::json!({
                    "url": thumbnail_url,
                    "id": size,
                    "preference": preference as i64,
                }))
            })
            .collect::<Vec<_>>();
        let mut output = InfoDict::new();
        output.insert("id", serde_json::json!(video_id));
        output.insert("display_id", serde_json::json!(display_id));
        output.insert("url", serde_json::json!(media_url));
        output.insert("ext", serde_json::json!(extension));
        output.insert("title", serde_json::json!(title));
        output.insert_if_some("description", json_string(&movie_data, "synopsis"));
        output.insert_if_some("creator", json_string(&movie_data, "producer"));
        output.insert_if_some("duration", json_i64(&movie_data, "duration"));
        output.insert_if_some("view_count", json_i64(&movie_data, "views"));
        output.insert("thumbnails", serde_json::Value::Array(thumbnails));
        output.insert("formats", formats);
        output.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(output))
    }
}
