/// Native Premiership Rugby article/JWPlatform HLS extractor.
pub struct PremiershipRugbyExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl PremiershipRugbyExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for PremiershipRugbyExtractor {
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
                    "Premiership Rugby URL has no article slug",
                )
            })?;
        let data_url = format!(
            "https://article-cms-api.incrowdsports.com/v2/articles/slug/{display_id}?clientId=PRL"
        );
        let response = context.get_json(&data_url)?;
        let article = response
            .get("data")
            .and_then(|data| data.get("article"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Premiership Rugby article {display_id} has no article object"),
                )
            })?;
        let hero = article.get("heroMedia").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Premiership Rugby article {display_id} has no hero media"),
            )
        })?;
        let content = hero.get("content").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Premiership Rugby article {display_id} has no media content"),
            )
        })?;
        let media_url = json_string(content, "videoLink").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Premiership Rugby article {display_id} has no video link"),
            )
        })?;
        let video_id = json_string(content, "sourceSystemId").unwrap_or(&display_id);
        let duration = content
            .get("metadata")
            .and_then(|metadata| json_f64(metadata, "msDuration"))
            .map(|milliseconds| milliseconds / 1000.0);
        let categories = article
            .get("categories")
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                serde_json::Value::Array(
                    items
                        .iter()
                        .filter_map(|item| json_string(item, "text").map(str::to_owned))
                        .map(serde_json::Value::String)
                        .collect(),
                )
            });
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("display_id", serde_json::json!(display_id));
        info.insert_if_some("title", json_string(hero, "title"));
        info.insert_if_some("thumbnail", json_string(content, "videoThumbnail"));
        info.insert_if_some("duration", duration);
        info.insert_if_some("tags", article.get("tags").cloned());
        info.insert_if_some("categories", categories);
        info.insert_if_some(
            "subtitles",
            content
                .get("subtitles")
                .cloned()
                .or_else(|| content.get("captions").cloned()),
        );
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "hls",
                "protocol": "m3u8_native",
                "ext": "mp4",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}
