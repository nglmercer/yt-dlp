/// Native Metacritic trailer XML/page extractor.
pub struct MetacriticExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MetacriticExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MetacriticExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Metacritic URL has no ID")
            })?;
        let page = metacritic_page(context, url)?;
        let xml_url = format!("http://www.metacritic.com/video_data?video={video_id}");
        let xml_response = context.get(&xml_url)?;
        let clips = metacritic_parse_xml(xml_response.body())?;
        let clip = clips
            .into_iter()
            .find(|clip| clip.id.as_deref() == Some(video_id.as_str()))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Metacritic XML has no clip {video_id}"),
                )
            })?;
        let mut formats = Vec::new();
        for file in clip.files {
            let (Some(media_url), Some(rate)) = (file.url, file.rate) else {
                continue;
            };
            let Ok(tbr) = rate.parse::<i64>() else {
                continue;
            };
            formats.push(serde_json::json!({
                "url": media_url,
                "ext": "mp4",
                "format_id": rate,
                "tbr": tbr,
            }));
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Metacritic clip {video_id} has no playable files"),
            ));
        }
        let title = clip.title.ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Metacritic clip {video_id} has no title"),
            )
        })?;
        let duration = clip
            .duration
            .as_deref()
            .and_then(|value| value.parse::<i64>().ok())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Metacritic clip {video_id} has no duration"),
                )
            })?;
        let description = metacritic_description(&page)?;
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("description", serde_json::json!(description));
        info.insert("duration", serde_json::json!(duration));
        info.insert_if_some("url", first.get("url").and_then(serde_json::Value::as_str));
        info.insert_if_some("ext", first.get("ext").and_then(serde_json::Value::as_str));
        Ok(ExtractorResult::single(info))
    }
}
