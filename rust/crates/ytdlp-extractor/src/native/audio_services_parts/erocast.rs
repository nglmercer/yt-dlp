/// Native Erocast track page/player extractor.
pub struct ErocastExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl ErocastExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for ErocastExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Erocast URL has no track ID")
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let assignment = Regex::new(&format!(
            r"(?is)var\s+song_data_{}\s*=",
            regex::escape(&video_id)
        ))
        .ok()
        .and_then(|matcher| matcher.find(&webpage).ok().flatten())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Erocast track {video_id} has no player data"),
            )
        })?;
        let data = json_object_after_marker(&webpage[assignment.end()..], "").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Erocast track {video_id} has invalid player data"),
            )
        })?;
        let media_url = json_string(&data, "file_url")
            .or_else(|| json_string(&data, "stream_url"))
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            .map(str::to_owned)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Erocast track {video_id} has no playable stream URL"),
                )
            })?;
        let extension = yt_dlp_core::determine_ext(Some(&media_url), "m4a");
        let is_hls = extension.eq_ignore_ascii_case("m3u8");
        let media_format = serde_json::json!({
            "url": media_url,
            "format_id": "hls",
            "protocol": if is_hls { "m3u8_native" } else { "http" },
            "ext": if is_hls { "m4a" } else { extension.as_str() },
            "vcodec": "none",
        });
        let user = data.get("user").unwrap_or(&serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(&data, "title"));
        info.insert_if_some("description", json_string(&data, "description"));
        info.insert("age_limit", serde_json::json!(18));
        info.insert_if_some(
            "release_timestamp",
            json_string(&data, "created_at")
                .map(str::to_owned)
                .and_then(parse_timestamp),
        );
        info.insert_if_some(
            "modified_timestamp",
            json_string(&data, "updated_at")
                .map(str::to_owned)
                .and_then(parse_timestamp),
        );
        info.insert_if_some("uploader", json_string(user, "name"));
        info.insert_if_some("uploader_id", json_value_string(user.get("id")));
        info.insert_if_some(
            "uploader_url",
            json_string(user, "permalink_url")
                .filter(|value| value.starts_with("http://") || value.starts_with("https://")),
        );
        info.insert_if_some(
            "thumbnail",
            json_string(&data, "artwork_url")
                .filter(|value| value.starts_with("http://") || value.starts_with("https://")),
        );
        info.insert_if_some("duration", json_i64(&data, "duration"));
        info.insert_if_some("view_count", json_i64(&data, "plays"));
        info.insert_if_some("comment_count", json_i64(&data, "comment_count"));
        info.insert_if_some(
            "webpage_url",
            json_string(&data, "permalink_url")
                .filter(|value| value.starts_with("http://") || value.starts_with("https://")),
        );
        info.insert(
            "url",
            media_format
                .get("url")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
        info.insert(
            "ext",
            media_format
                .get("ext")
                .cloned()
                .unwrap_or_else(|| serde_json::json!("m4a")),
        );
        info.insert("formats", serde_json::json!([media_format]));
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}
