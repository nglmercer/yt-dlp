/// Native GoPro page metadata and media-variations extractor.
pub struct GoProExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl GoProExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for GoProExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "GoPro URL has no ID")
            })?;
        let page_response = context.get(url)?;
        let webpage = String::from_utf8_lossy(page_response.body());
        let metadata = json_object_after_marker(&webpage, "window.__reflectData").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("GoPro page {video_id} has no reflect metadata"),
            )
        })?;
        let video_info = metadata
            .get("collectionMedia")
            .and_then(serde_json::Value::as_array)
            .and_then(|media| media.first())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("GoPro page {video_id} has no collection media"),
                )
            })?;
        let media_id = json_string(video_info, "id").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("GoPro video {video_id} has no media API ID"),
            )
        })?;
        let media_response =
            context.get(&format!("https://api.gopro.com/media/{media_id}/download"))?;
        let media_data = serde_json::from_slice::<serde_json::Value>(media_response.body())
            .map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid GoPro media JSON for {video_id}: {error}"),
                )
            })?;
        let mut formats = Vec::new();
        if let Some(variations) = media_data
            .get("_embedded")
            .and_then(|embedded| embedded.get("variations"))
            .and_then(serde_json::Value::as_array)
        {
            for variation in variations {
                let Some(format_url) = json_string(variation, "url")
                    .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
                else {
                    continue;
                };
                let extension = json_string(variation, "type")
                    .map(str::to_owned)
                    .unwrap_or_else(|| {
                        yt_dlp_core::determine_ext(Some(format_url), "mp4")
                    });
                let mut format = serde_json::Map::new();
                format.insert("url".to_owned(), serde_json::json!(format_url));
                if let Some(quality) = json_string(variation, "quality") {
                    format.insert("format_id".to_owned(), serde_json::json!(quality));
                }
                if let Some(label) = json_string(variation, "label") {
                    format.insert("format_note".to_owned(), serde_json::json!(label));
                }
                format.insert("ext".to_owned(), serde_json::json!(extension));
                if let Some(width) = json_i64(variation, "width") {
                    format.insert("width".to_owned(), serde_json::json!(width));
                }
                if let Some(height) = json_i64(variation, "height") {
                    format.insert("height".to_owned(), serde_json::json!(height));
                }
                formats.push(serde_json::Value::Object(format));
            }
        }
        let title = metadata
            .get("collection")
            .and_then(|collection| json_string(collection, "title"))
            .map(str::to_owned)
            .or_else(|| html_meta_value(&webpage, "og:title"))
            .or_else(|| html_meta_value(&webpage, "twitter:title"))
            .or_else(|| gopro_html_title(&webpage))
            .map(|value| {
                value
                    .replace('\n', " ")
                    .strip_suffix(" | GoPro")
                    .unwrap_or(&value)
                    .to_owned()
            });
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", title);
        info.insert_if_some(
            "thumbnail",
            html_meta_value(&webpage, "og:image")
                .or_else(|| html_meta_value(&webpage, "twitter:image")),
        );
        info.insert_if_some(
            "timestamp",
            metadata
                .get("collection")
                .and_then(|collection| json_string(collection, "created_at"))
                .map(str::to_owned)
                .and_then(parse_timestamp),
        );
        info.insert_if_some(
            "uploader_id",
            metadata
                .get("account")
                .and_then(|account| json_string(account, "nickname")),
        );
        info.insert_if_some("duration", json_i64(video_info, "source_duration"));
        info.insert_if_some(
            "artist",
            json_string(video_info, "music_track_artist")
                .filter(|value| !value.is_empty()),
        );
        info.insert_if_some(
            "track",
            json_string(video_info, "music_track_name")
                .filter(|value| !value.is_empty()),
        );
        if let Some(first_format) = formats.first() {
            info.insert_if_some("url", first_format.get("url"));
            info.insert_if_some("ext", first_format.get("ext"));
        }
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}

fn gopro_html_title(html: &str) -> Option<String> {
    let matcher = Regex::new(r#"(?is)<title\b[^>]*>(.*?)</title>"#).ok()?;
    matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1).map(|value| html_text_fragment(value.as_str())))
        .filter(|value| !value.is_empty())
}
