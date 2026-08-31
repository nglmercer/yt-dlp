/// Native Art19 podcast episode extractor. Episode and RSS metadata are read
/// through the Rust request context; the direct MP3 URL remains available even
/// when the optional metadata endpoints omit a media record.
pub struct Art19Extractor {
    descriptor: ExtractorDescriptor,
    matchers: Vec<Regex>,
}

impl Art19Extractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        let mut matchers = Vec::new();
        for pattern in &descriptor.valid_urls {
            matchers.push(compile_source_pattern(pattern).map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid Art19 URL pattern: {error}"),
                )
            })?);
        }
        Ok(Self {
            descriptor,
            matchers,
        })
    }

    fn episode_id(&self, url: &str) -> Result<String, ExtractorError> {
        self.matchers
            .iter()
            .find_map(|matcher| {
                matcher
                    .captures(url)
                    .ok()
                    .flatten()
                    .and_then(|captures| captures.name("id"))
                    .map(|value| value.as_str().to_owned())
            })
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Art19 URL has no episode ID")
            })
    }
}

impl InfoExtractor for Art19Extractor {
    fn descriptor(&self) -> &ExtractorDescriptor {
        &self.descriptor
    }

    fn suitable(&self, url: &str) -> bool {
        self.matchers
            .iter()
            .any(|matcher| matcher.is_match(url).unwrap_or(false))
    }

    fn is_native(&self) -> bool {
        true
    }

    fn native_matcher_count(&self) -> usize {
        self.matchers.len()
    }

    fn extract_with_context(
        &self,
        url: &str,
        context: &ExtractionContext,
    ) -> Result<ExtractorResult, ExtractorError> {
        let episode_id = self.episode_id(url)?;
        let player_metadata = context
            .get_json(&format!("https://art19.com/episodes/{episode_id}"))
            .ok();
        let rss_metadata = context.get_json(&format!(
            "https://rss.art19.com/episodes/{episode_id}.json"
        ))?;
        let episode = player_metadata.as_ref().and_then(|value| value.get("episode"));
        let content = rss_metadata.get("content").unwrap_or(&rss_metadata);
        let direct_url = content
            .get("media")
            .and_then(|media| media.get("mp3"))
            .and_then(|media| json_string(media, "url"))
            .map(str::to_owned)
            .unwrap_or_else(|| format!("https://rss.art19.com/episodes/{episode_id}.mp3"));

        let mut formats = vec![serde_json::json!({
            "format_id": "direct",
            "url": direct_url,
            "vcodec": "none",
            "acodec": "mp3",
        })];
        if let Some(media) = content.get("media").and_then(serde_json::Value::as_object) {
            for (format_id, format_data) in media {
                if format_id == "waveform_bin" {
                    continue;
                }
                let Some(format_url) = json_string(format_data, "url") else {
                    continue;
                };
                formats.push(serde_json::json!({
                    "format_id": format_id,
                    "url": format_url,
                    "vcodec": "none",
                    "acodec": format_id,
                    "quality": if format_id == "ogg" { -2 } else { -1 },
                }));
            }
        }

        let title = json_string(content, "episode_title")
            .or_else(|| episode.and_then(|episode| json_string(episode, "title")))
            .map(str::to_owned)
            .unwrap_or_else(|| episode_id.clone());
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(episode_id));
        info.insert("title", serde_json::json!(title));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some(
            "description",
            json_string(content, "episode_description_plain")
                .or_else(|| episode.and_then(|episode| json_string(episode, "description_plain"))),
        );
        info.insert_if_some(
            "episode_id",
            json_string(content, "episode_id")
                .or_else(|| episode.and_then(|episode| json_string(episode, "id"))),
        );
        info.insert_if_some(
            "episode_number",
            json_i64(content, "episode_number")
                .or_else(|| episode.and_then(|episode| json_i64(episode, "episode_number"))),
        );
        info.insert_if_some(
            "series",
            json_string(content, "series_title")
                .or_else(|| episode.and_then(|episode| json_string(episode, "series"))),
        );
        info.insert_if_some("series_id", json_string(content, "series_id"));
        info.insert_if_some("season", json_string(content, "season_title"));
        info.insert_if_some("season_id", json_string(content, "season_id"));
        info.insert_if_some("season_number", json_i64(content, "season_number"));
        info.insert_if_some("thumbnail", json_string(content, "cover_image"));
        info.insert_if_some("duration", json_f64(content, "duration"));
        info.insert_if_some(
            "timestamp",
            episode
                .and_then(|episode| json_string(episode, "created_at"))
                .map(str::to_owned)
                .and_then(parse_timestamp),
        );
        info.insert_if_some(
            "release_timestamp",
            episode
                .and_then(|episode| json_string(episode, "released_at"))
                .map(str::to_owned)
                .and_then(parse_timestamp),
        );
        info.insert_if_some(
            "modified_timestamp",
            episode
                .and_then(|episode| json_string(episode, "updated_at"))
                .map(str::to_owned)
                .and_then(parse_timestamp),
        );
        Ok(ExtractorResult::single(info))
    }
}
