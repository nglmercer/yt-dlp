/// Native Monster Siren/Hypergryph song and album API extractor.
pub struct MonsterSirenHypergryphMusicExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MonsterSirenHypergryphMusicExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MonsterSirenHypergryphMusicExtractor {
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
        let audio_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Monster Siren URL has no ID")
            })?;
        let song = context.get_json(&format!(
            "https://monster-siren.hypergryph.com/api/song/{audio_id}"
        ))?;
        if json_i64(&song, "code") != Some(0) {
            let message = json_string(&song, "msg").unwrap_or("API returned an error response");
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!("TODO: Monster Siren API rejected song {audio_id}: {message}"),
            ));
        }
        let song_data = song.get("data").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Monster Siren song {audio_id} has no data"),
            )
        })?;
        let media_url = json_string(song_data, "sourceUrl")
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Monster Siren song {audio_id} has no source URL"),
                )
            })?;
        let album = json_value_string(song_data.get("albumCid"))
            .and_then(|album_id| {
                context
                    .get_json(&format!(
                        "https://monster-siren.hypergryph.com/api/album/{album_id}/detail"
                    ))
                    .ok()
            })
            .and_then(|album| album.get("data").cloned());
        let extension = yt_dlp_core::determine_ext(Some(media_url), "wav");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(audio_id));
        info.insert("vcodec", serde_json::json!("none"));
        info.insert_if_some("title", json_string(song_data, "name"));
        if let Some(artists) = song_data.get("artists").and_then(serde_json::Value::as_array) {
            let artists = artists
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_owned)
                .collect::<Vec<_>>();
            if !artists.is_empty() {
                info.insert("artists", serde_json::json!(artists));
            }
        }
        if let Some(lyric_url) = json_string(song_data, "lyricUrl") {
            info.insert(
                "subtitles",
                serde_json::json!({"en":[{"url":lyric_url}]}),
            );
        } else {
            info.insert("subtitles", serde_json::json!({}));
        }
        info.insert_if_some(
            "album",
            album.as_ref().and_then(|value| json_string(value, "name")),
        );
        info.insert_if_some(
            "description",
            album
                .as_ref()
                .and_then(|value| json_string(value, "intro"))
                .map(html_text_fragment),
        );
        info.insert_if_some(
            "thumbnail",
            album.as_ref().and_then(|value| json_string(value, "coverUrl")),
        );
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!(extension.clone()));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "source",
                "protocol": "http",
                "ext": extension,
                "vcodec": "none",
            }]),
        );
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}
