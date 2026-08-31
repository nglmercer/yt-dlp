/// Native ReverbNation song API/audio extractor.
pub struct ReverbNationExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl ReverbNationExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for ReverbNationExtractor {
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
        let song_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "ReverbNation URL has no song ID",
                )
            })?;
        let data = context.get_json(&format!("https://api.reverbnation.com/song/{song_id}"))?;
        let media_url = json_string(&data, "url")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("ReverbNation song {song_id} has no audio URL"),
                )
            })?;
        let title = json_string(&data, "name").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("ReverbNation song {song_id} has no title"),
            )
        })?;
        let mut thumbnails = Vec::new();
        for (preference, key) in [(0, "thumbnail"), (1, "image")] {
            if let Some(thumbnail) = json_string(&data, key).filter(|value| !value.is_empty()) {
                thumbnails.push(serde_json::json!({
                    "url": thumbnail,
                    "preference": preference,
                }));
            }
        }
        let artist = data.get("artist");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(song_id));
        info.insert("title", serde_json::json!(title));
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!("mp3"));
        info.insert("vcodec", serde_json::json!("none"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "mp3",
                "ext": "mp3",
                "vcodec": "none",
                "acodec": "mp3",
            }]),
        );
        info.insert_if_some(
            "uploader",
            artist.and_then(|artist| json_string(artist, "name")),
        );
        info.insert_if_some(
            "uploader_id",
            artist.and_then(|artist| json_value_string(artist.get("id"))),
        );
        if !thumbnails.is_empty() {
            info.insert("thumbnails", serde_json::Value::Array(thumbnails));
        }
        Ok(ExtractorResult::single(info))
    }
}
