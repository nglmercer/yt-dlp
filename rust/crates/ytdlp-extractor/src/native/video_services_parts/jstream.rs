/// Native JStream JSONP/HLS extractor.
pub struct JstreamExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl JstreamExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for JstreamExtractor {
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
                "JStream URL did not match its native pattern",
            )
        })?;
        let host = captures
            .name("host")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| ExtractorError::new(ExtractorErrorKind::InvalidUrl, "JStream URL has no host"))?;
        let publisher = captures
            .name("publisher")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "JStream URL has no publisher")
            })?;
        let media_id = captures
            .name("mid")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| ExtractorError::new(ExtractorErrorKind::InvalidUrl, "JStream URL has no media ID"))?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .unwrap_or_else(|| format!("{publisher}:{media_id}"));
        let metadata_url = format!(
            "https://{publisher}.eq.webcdn.stream.ne.jp/{host}/{publisher}/jmc_pub/eq_meta/v1/{media_id}.jsonp"
        );
        let response = context.get(&metadata_url)?;
        let json = json_object_after_marker(
            &String::from_utf8_lossy(response.body()),
            "metaDataResult",
        )
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("JStream media {video_id} has invalid JSONP metadata"),
            )
        })?;
        let movie = json.get("movie").unwrap_or(&json);
        let mut formats = Vec::new();
        for item in movie
            .get("movie_list_hls")
            .into_iter()
            .flat_map(json_object_values)
        {
            let Some(text) = json_string(item, "text").filter(|value| value.starts_with("auto"))
            else {
                continue;
            };
            let format_id = text
                .strip_prefix("auto")
                .unwrap_or(text)
                .strip_prefix('_')
                .filter(|value| !value.is_empty())
                .unwrap_or("hls");
            let Some(path) = json_string(item, "url") else {
                continue;
            };
            let media_url =
                format!("https://{publisher}.eq.webcdn.stream.ne.jp/{host}/{publisher}/jmc_pub/{path}");
            formats.push(serde_json::json!({
                "url": media_url,
                "format_id": format_id,
                "ext": "mp4",
                "protocol": "m3u8_native",
            }));
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("JStream media {video_id} has no auto HLS rendition"),
            ));
        }
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(movie, "title"));
        info.insert_if_some("duration", json_f64(movie, "duration"));
        info.insert_if_some("thumbnail", json_string(movie, "thumbnail_url"));
        info.insert("url", first.get("url").cloned().unwrap_or(serde_json::Value::Null));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", serde_json::json!({}));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}
