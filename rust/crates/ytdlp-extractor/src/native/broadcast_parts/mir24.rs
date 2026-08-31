/// Native Mir24 article iframe/HLS extractor.
pub struct Mir24TvExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl Mir24TvExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for Mir24TvExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Mir24 URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let iframe_url =
            Regex::new(r#"(?is)<iframe\b[^>]+\bsrc\s*=\s*["'](https?://mir24\.tv/players/[^"']+)"#)
                .ok()
                .and_then(|matcher| matcher.captures(&html).ok().flatten())
                .and_then(|captures| captures.get(1))
                .map(|value| value.as_str().to_owned())
                .ok_or_else(|| {
                    ExtractorError::new(
                        ExtractorErrorKind::Extraction,
                        format!("Mir24 article {video_id} has no player iframe"),
                    )
                })?;
        let player = url::Url::parse(&iframe_url).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Mir24 player URL: {error}"),
            )
        })?;
        let media_url = player
            .query_pairs()
            .find_map(|(key, value)| {
                (key == "source").then(|| proto_relative_url(value.as_ref(), "https:"))
            })
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Mir24 player {video_id} has no source URL"),
                )
            })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert(
            "title",
            serde_json::json!(
                html_meta_value(&html, "og:title")
                    .or_else(|| html_title_value(&html))
                    .unwrap_or_else(|| video_id.clone())
            ),
        );
        info.insert_if_some("thumbnail", html_meta_value(&html, "og:image"));
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "hls",
                "ext": "mp4",
                "protocol": "m3u8_native",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}
