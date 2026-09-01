/// Native ERR/Jupiter VOD-content API extractor.
pub struct ErrJupiterExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl ErrJupiterExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for ErrJupiterExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "ERR URL has no content ID")
            })?;
        let data = context.get_json(&format!(
            "https://services.err.ee/api/v2/vodContent/getContentPageData?contentId={video_id}"
        ))?;
        let content = data
            .get("data")
            .and_then(|data| data.get("mainContent"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("ERR content {video_id} has no main content"),
                )
            })?;
        let media = err_media_object(content.get("medias")).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("ERR content {video_id} has no media data"),
            )
        })?;
        if json_bool(
            media
                .get("restrictions")
                .unwrap_or(&serde_json::Value::Null),
            "drm",
        )
        .unwrap_or(false)
        {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!("TODO: ERR content {video_id} uses DRM-protected media"),
            ));
        }
        let source = media.get("src").unwrap_or(&serde_json::Value::Null);
        let mut formats = Vec::new();
        for key in ["hls", "hls2", "hlsNew"] {
            if let Some(media_url) = json_string(source, key)
                .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            {
                if !formats.iter().any(|format: &serde_json::Value| {
                    format.get("url").and_then(serde_json::Value::as_str) == Some(media_url)
                }) {
                    formats.push(serde_json::json!({
                        "url": media_url,
                        "format_id": "hls",
                        "protocol": "m3u8_native",
                        "ext": "mp4",
                    }));
                }
            }
        }
        for key in ["dash", "dashNew"] {
            if let Some(media_url) = json_string(source, key)
                .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            {
                if !formats.iter().any(|format: &serde_json::Value| {
                    format.get("url").and_then(serde_json::Value::as_str) == Some(media_url)
                }) {
                    formats.push(serde_json::json!({
                        "url": media_url,
                        "format_id": "dash",
                        "protocol": "http_dash_segments",
                        "ext": "mp4",
                    }));
                }
            }
        }
        if let Some(media_url) = json_string(source, "file")
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
        {
            formats.push(serde_json::json!({
                "url": media_url,
                "format_id": "http",
                "protocol": "http",
                "ext": yt_dlp_core::determine_ext(Some(media_url), "mp4"),
            }));
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("ERR content {video_id} has no playable media sources"),
            ));
        }
        let content_type = json_string(content, "type");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert(
            "title",
            serde_json::json!(
                json_string(content, "heading")
                    .filter(|value| !value.is_empty())
                    .unwrap_or(&video_id)
            ),
        );
        info.insert_if_some("alt_title", json_string(content, "subHeading"));
        info.insert_if_some(
            "description",
            ["lead", "body"].iter().find_map(|key| {
                json_string(content, key)
                    .map(html_text_fragment)
                    .filter(|value| !value.is_empty())
            }),
        );
        for (field, key) in [
            ("timestamp", "created"),
            ("modified_timestamp", "updated"),
            ("release_timestamp", "scheduleStart"),
        ] {
            info.insert_if_some(field, json_i64(content, key));
        }
        if info.get("release_timestamp").is_none() {
            info.insert_if_some("release_timestamp", json_i64(content, "publicStart"));
        }
        info.insert_if_some("release_year", json_i64(content, "year"));
        if content_type == Some("episode") {
            info.insert_if_some("series", json_string(content, "heading"));
            info.insert_if_some("series_id", json_string(content, "rootContentId"));
            info.insert_if_some("episode", json_string(content, "subHeading"));
            info.insert_if_some("season_number", json_i64(content, "season"));
            info.insert_if_some("episode_number", json_i64(content, "episode"));
            info.insert_if_some("episode_id", json_string(content, "id"));
            if let Some(season) = json_i64(content, "season") {
                info.insert("season", serde_json::json!(format!("Season {season}")));
            }
        }
        let first_format = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        info.insert(
            "url",
            first_format
                .get("url")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
        info.insert(
            "ext",
            first_format
                .get("ext")
                .cloned()
                .unwrap_or_else(|| serde_json::json!("mp4")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}

/// Native ERR Arhiiv API extractor.
pub struct ErrArhiivExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl ErrArhiivExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for ErrArhiivExtractor {
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
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "ERR Arhiiv URL has no content ID",
                )
            })?;
        let data = context.get_json(&format!(
            "https://arhiiv.err.ee/api/v1/content/video/{video_id}"
        ))?;
        let media_src = data
            .get("media")
            .and_then(|media| media.get("src"))
            .unwrap_or(&serde_json::Value::Null);
        let mut formats = Vec::new();
        for (key, format_id, protocol) in [
            ("hls", "hls", "m3u8_native"),
            ("dash", "dash", "http_dash_segments"),
        ] {
            let Some(media_url) = json_string(media_src, key)
                .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            else {
                continue;
            };
            if formats.iter().any(|format: &serde_json::Value| {
                format.get("url").and_then(serde_json::Value::as_str) == Some(media_url)
            }) {
                continue;
            }
            formats.push(serde_json::json!({
                "url": media_url,
                "format_id": format_id,
                "protocol": protocol,
                "ext": "mp4",
            }));
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("ERR Arhiiv content {video_id} has no playable media sources"),
            ));
        }
        let metadata = data.get("info").unwrap_or(&serde_json::Value::Null);
        let first_format = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(metadata, "title").filter(|value| !value.is_empty()));
        info.insert_if_some("series", json_string(metadata, "seriesTitle").filter(|value| !value.is_empty()));
        info.insert_if_some("series_id", json_string(metadata, "seriesId").filter(|value| !value.is_empty()));
        info.insert_if_some("episode_id", json_string(metadata, "episode").filter(|value| !value.is_empty()));
        info.insert_if_some("description", json_string(metadata, "synopsis").filter(|value| !value.is_empty()));
        for (field, key) in [
            ("timestamp", "uploadDate"),
            ("modified_timestamp", "dateModified"),
            ("release_timestamp", "date"),
        ] {
            info.insert_if_some(field, err_timestamp(metadata, key));
        }
        info.insert_if_some("release_year", json_i64(metadata, "year"));
        info.insert(
            "url",
            first_format
                .get("url")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
        info.insert(
            "ext",
            first_format
                .get("ext")
                .cloned()
                .unwrap_or_else(|| serde_json::json!("mp4")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}

fn err_timestamp(value: &serde_json::Value, key: &str) -> Option<i64> {
    json_i64(value, key).or_else(|| json_string(value, key).and_then(|value| parse_timestamp(value.to_owned())))
}

fn err_media_object(value: Option<&serde_json::Value>) -> Option<&serde_json::Value> {
    match value? {
        serde_json::Value::Object(_) => value,
        serde_json::Value::Array(values) => values.iter().find(|value| value.is_object()),
        _ => None,
    }
}
