/// Native RadioZET podcast API extractor.
pub struct RadioZetPodcastExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl RadioZetPodcastExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for RadioZetPodcastExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "RadioZET URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let podcast_id = Regex::new(
            r#"(?is)<div\b[^>]*\bid\s*=\s*["']player["'][^>]*\bdata-id\s*=\s*["']([^"']+)"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&html).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().to_owned())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("RadioZET podcast {display_id} has no player ID"),
            )
        })?;
        let data_url = format!(
            "https://player.radiozet.pl/api/podcasts/getPodcast/(node)/{podcast_id}/(station)/radiozet"
        );
        let response = context.get_json(&data_url)?;
        let data = response
            .get("data")
            .and_then(serde_json::Value::as_array)
            .and_then(|items| items.first())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("RadioZET podcast {podcast_id} has no API record"),
                )
            })?;
        let stream_url = data
            .get("player")
            .and_then(|player| json_string(player, "stream"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("RadioZET podcast {podcast_id} has no audio stream"),
                )
            })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(podcast_id));
        info.insert("display_id", serde_json::json!(display_id));
        info.insert_if_some(
            "title",
            data.get("title")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        );
        info.insert_if_some(
            "description",
            data.get("program")
                .and_then(|program| json_string(program, "desc"))
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        );
        info.insert_if_some(
            "release_timestamp",
            data.get("published_date").and_then(|value| {
                value
                    .as_i64()
                    .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
            }),
        );
        info.insert_if_some(
            "thumbnail",
            data.get("program")
                .and_then(|program| program.get("image"))
                .and_then(|image| json_string(image, "original")),
        );
        info.insert_if_some(
            "duration",
            data.get("player")
                .and_then(|player| player.get("duration"))
                .cloned(),
        );
        info.insert_if_some(
            "series",
            data.get("program")
                .and_then(|program| json_string(program, "title"))
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        );
        info.insert_if_some(
            "creator",
            data.get("presenter")
                .and_then(serde_json::Value::as_array)
                .and_then(|presenters| presenters.first())
                .and_then(|presenter| json_string(presenter, "title"))
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        );
        let ext = yt_dlp_core::determine_ext(Some(stream_url), "mp3");
        info.insert("url", serde_json::json!(stream_url));
        info.insert("ext", serde_json::json!(ext));
        info.insert(
            "formats",
            serde_json::json!([{"url": stream_url, "format_id": "source", "ext": ext}]),
        );
        Ok(ExtractorResult::single(info))
    }
}
