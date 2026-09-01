/// Native HuffPost segment API extractor.
pub struct HuffPostExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl HuffPostExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for HuffPostExtractor {
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
            .ok_or_else(|| ExtractorError::new(ExtractorErrorKind::InvalidUrl, "HuffPost URL has no ID"))?;
        let response = context.get(&format!(
            "http://embed.live.huffingtonpost.com/api/segments/{video_id}.json"
        ))?;
        let root: serde_json::Value = serde_json::from_slice(response.body()).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid HuffPost segment JSON: {error}"),
            )
        })?;
        let data = root.get("data").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("HuffPost segment {video_id} has no data"),
            )
        })?;
        let title = json_string(data, "title").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("HuffPost segment {video_id} has no title"),
            )
        })?;
        let mut formats = Vec::new();
        let mut has_hds = false;
        if let Some(sources) = data.get("sources") {
            for source_group in ["live", "live_again"] {
                let Some(group) = sources
                    .get(source_group)
                    .and_then(serde_json::Value::as_object)
                else {
                    continue;
                };
                for (key, value) in group {
                    let Some(media_url) = value.as_str().filter(|value| {
                        value.starts_with("http://") || value.starts_with("https://")
                    }) else {
                        continue;
                    };
                    let source_ext = yt_dlp_core::determine_ext(Some(media_url), "mp4");
                    if source_ext == "f4m" {
                        has_hds = true;
                        continue;
                    }
                    let format_id = if source_ext == "m3u8" {
                        "hls".to_owned()
                    } else {
                        key.replace('/', ".")
                    };
                    let mut format = serde_json::json!({
                        "url": media_url,
                        "format_id": format_id,
                        "ext": "mp4",
                    });
                    if source_ext == "m3u8" {
                        format["protocol"] = serde_json::json!("m3u8_native");
                    }
                    if key.starts_with("audio/") {
                        format["vcodec"] = serde_json::json!("none");
                    }
                    formats.push(format);
                }
            }
        }
        if formats.is_empty() && has_hds {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!("TODO: HuffPost segment {video_id} requires HDS/F4M parsing"),
            ));
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("HuffPost segment {video_id} has no playable sources"),
            ));
        }
        let thumbnails = huffpost_thumbnails(data.get("images"));
        let first_url = formats
            .first()
            .and_then(|format| format.get("url"))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("HuffPost segment {video_id} has no first source URL"),
                )
            })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some(
            "description",
            json_string(data, "description").map(str::to_owned),
        );
        info.insert_if_some(
            "duration",
            json_string(data, "running_time")
                .and_then(|value| yt_dlp_core::parse_duration(value.trim())),
        );
        let scheduled = json_string(data.get("schedule").unwrap_or(&serde_json::Value::Null), "starts_at")
            .or_else(|| json_string(data, "segment_start_date_time"));
        info.insert_if_some("timestamp", scheduled.and_then(|value| parse_timestamp(value.to_owned())));
        info.insert_if_some("upload_date", scheduled.and_then(|value| date_digits(value)));
        if let Some(thumbnail) = thumbnails.first() {
            info.insert_if_some(
                "thumbnail",
                thumbnail.get("url").and_then(serde_json::Value::as_str),
            );
        }
        info.insert("url", serde_json::json!(first_url));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("thumbnails", serde_json::Value::Array(thumbnails));
        info.insert("subtitles", serde_json::json!({}));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

fn huffpost_thumbnails(images: Option<&serde_json::Value>) -> Vec<serde_json::Value> {
    let matcher = Regex::new(r#"-([0-9]+x[0-9]+)\.[^./?]+(?:[?#]|$)"#).ok();
    images
        .and_then(serde_json::Value::as_object)
        .into_iter()
        .flat_map(serde_json::Map::values)
        .filter_map(|value| {
            let url = value.as_str()?;
            let resolution = matcher
                .as_ref()
                .and_then(|matcher| matcher.captures(url).ok().flatten())
                .and_then(|captures| captures.get(1))
                .map(|value| value.as_str().to_owned());
            let mut thumbnail = serde_json::json!({"url": url});
            if let Some(resolution) = resolution {
                thumbnail["resolution"] = serde_json::json!(resolution);
            }
            Some(thumbnail)
        })
        .collect()
}
