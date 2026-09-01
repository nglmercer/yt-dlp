/// Native Gab status API extractor.
pub struct GabExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl GabExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for GabExtractor {
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
        let post_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Gab post has no ID")
            })?;
        let data = context.get_json(&format!("https://gab.com/api/v1/statuses/{post_id}"))?;
        let account = data.get("account").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Gab post {post_id} has no account"),
            )
        })?;
        let author = gab_value_string(account.get("username"));
        let author_id = gab_value_string(account.get("id"));
        let author_url = gab_url(account.get("url"));
        let title = format!(
            "{} on Gab",
            gab_value_string(account.get("display_name"))
                .unwrap_or_else(|| author.clone().unwrap_or_else(|| "Gab".to_owned()))
        );
        let attachments = data
            .get("media_attachments")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Gab post {post_id} has no media attachments"),
                )
            })?;
        let mut entries = Vec::new();
        for (index, media) in attachments.iter().enumerate() {
            if !matches!(
                json_string(media, "type"),
                Some("video") | Some("gifv")
            ) {
                continue;
            }
            let media_meta = media.get("meta").unwrap_or(&serde_json::Value::Null);
            let formats = gab_formats(media, media_meta);
            if formats.is_empty() {
                continue;
            }
            let first_format = formats.first().cloned().unwrap_or(serde_json::Value::Null);
            let mut info = InfoDict::new();
            info.insert("id", serde_json::json!(format!("{post_id}-{index}")));
            info.insert("title", serde_json::json!(title));
            info.insert_if_some(
                "timestamp",
                json_string(&data, "created_at")
                    .map(str::to_owned)
                    .and_then(parse_timestamp),
            );
            info.insert(
                "formats",
                serde_json::Value::Array(formats),
            );
            info.insert_if_some("url", first_format.get("url"));
            info.insert_if_some("ext", first_format.get("ext"));
            info.insert_if_some(
                "description",
                json_string(&data, "content").map(html_text_fragment),
            );
            info.insert_if_some(
                "duration",
                json_f64(media_meta, "duration").or_else(|| {
                    json_string(media_meta, "length")
                        .and_then(yt_dlp_core::parse_duration)
                }),
            );
            info.insert_if_some("like_count", json_i64(&data, "favourites_count"));
            info.insert_if_some("comment_count", json_i64(&data, "replies_count"));
            info.insert_if_some("repost_count", json_i64(&data, "reblogs_count"));
            info.insert_if_some("uploader", author.clone());
            info.insert_if_some("uploader_id", author_id.clone());
            info.insert_if_some("uploader_url", author_url.clone());
            entries.push(info);
        }
        if entries.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Gab post {post_id} has no video attachments"),
            ));
        }
        if entries.len() == 1 {
            return Ok(ExtractorResult::single(entries.remove(0)));
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(post_id));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

fn gab_formats(media: &serde_json::Value, media_meta: &serde_json::Value) -> Vec<serde_json::Value> {
    let variants = [
        ("original", "url"),
        ("playable", "source_mp4"),
    ];
    variants
        .into_iter()
        .filter_map(|(metadata_key, url_key)| {
            let media_url = json_string(media, url_key)
                .filter(|value| value.starts_with("http://") || value.starts_with("https://"))?;
            let format_metadata = media_meta.get(metadata_key).unwrap_or(&serde_json::Value::Null);
            let extension = yt_dlp_core::determine_ext(Some(media_url), "mp4");
            let mut format = serde_json::Map::new();
            format.insert("url".to_owned(), serde_json::json!(media_url));
            format.insert("format_id".to_owned(), serde_json::json!(metadata_key));
            format.insert("ext".to_owned(), serde_json::json!(extension));
            format.insert("protocol".to_owned(), serde_json::json!("http"));
            if let Some(width) = json_i64(format_metadata, "width") {
                format.insert("width".to_owned(), serde_json::json!(width));
            }
            if let Some(height) = json_i64(format_metadata, "height") {
                format.insert("height".to_owned(), serde_json::json!(height));
            }
            if let Some(bitrate) = json_i64(format_metadata, "bitrate") {
                format.insert("tbr".to_owned(), serde_json::json!(bitrate * 1000));
            }
            if let Some(fps) = json_f64(format_metadata, "fps") {
                format.insert("fps".to_owned(), serde_json::json!(fps));
            }
            if let Some(codec) = json_string(format_metadata, "audio_encode") {
                format.insert("acodec".to_owned(), serde_json::json!(codec));
            }
            Some(serde_json::Value::Object(format))
        })
        .collect()
}

fn gab_value_string(value: Option<&serde_json::Value>) -> Option<String> {
    value.map(|value| {
        value
            .as_str()
            .map(str::to_owned)
            .or_else(|| value.as_i64().map(|value| value.to_string()))
            .or_else(|| value.as_u64().map(|value| value.to_string()))
            .unwrap_or_else(|| value.to_string())
    })
}

fn gab_url(value: Option<&serde_json::Value>) -> Option<String> {
    let value = gab_value_string(value)?;
    (value.starts_with("http://") || value.starts_with("https://")).then_some(value)
}
