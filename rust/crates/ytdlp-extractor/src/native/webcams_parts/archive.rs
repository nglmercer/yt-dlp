/// Native Moving Image archive extractor. Archive pages expose one HLS
/// manifest and a small set of labelled metadata fields.
pub struct MovingImageExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MovingImageExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MovingImageExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Moving Image URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let media_url = Regex::new(r#"(?is)\bfile\s*:\s*"([^"]+)"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| resolve_url(url, value.as_str()))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Moving Image film {video_id} has no HLS URL"),
                )
            })?;
        let title = html_field_value(&html, "Title")
            .map(|value| value.trim_matches(['(', ')', '[', ']']).trim().to_owned())
            .filter(|value| !value.is_empty())
            .or_else(|| html_title_value(&html))
            .unwrap_or_else(|| video_id.clone());
        let description = html_field_value(&html, "Description");
        let duration = html_field_value(&html, "Running time").and_then(|value| {
            yt_dlp_core::parse_duration(value.trim_matches(['(', ')', '[', ']']))
        });
        let thumbnail = Regex::new(r#"(?is)\bimage\s*:\s*'([^']+)'"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| resolve_url(url, value.as_str()));
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", description);
        info.insert_if_some("duration", duration);
        info.insert_if_some("thumbnail", thumbnail);
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

/// Native Tweakers video API extractor.
pub struct TweakersExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl TweakersExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for TweakersExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Tweakers URL has no ID")
            })?;
        let data = context.get_json(&format!(
            "https://tweakers.net/video/s1playlist/{video_id}/1920/1080/playlist.json"
        ))?;
        let item = data
            .get("items")
            .and_then(serde_json::Value::as_array)
            .and_then(|items| items.first())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Tweakers video {video_id} has no API item"),
                )
            })?;
        let mut formats = Vec::new();
        if let Some(locations) = item
            .get("locations")
            .and_then(|value| value.get("progressive"))
            .and_then(serde_json::Value::as_array)
        {
            for location in locations {
                let format_id = json_string(location, "label");
                for source in location
                    .get("sources")
                    .and_then(serde_json::Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let Some(media_url) = json_string(source, "src") else {
                        continue;
                    };
                    let ext = json_string(source, "type")
                        .and_then(|value| mimetype_extension(value.split(';').next()))
                        .unwrap_or_else(|| yt_dlp_core::determine_ext(Some(media_url), "mp4"));
                    let mut format = serde_json::json!({
                        "url": media_url,
                        "format_id": format_id,
                        "ext": ext,
                    });
                    if let Some(value) = json_i64(location, "width") {
                        format["width"] = serde_json::json!(value);
                    }
                    if let Some(value) = json_i64(location, "height") {
                        format["height"] = serde_json::json!(value);
                    }
                    formats.push(format);
                }
            }
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Tweakers video {video_id} has no progressive formats"),
            ));
        }
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(item, "title"));
        info.insert_if_some("description", json_string(item, "description"));
        info.insert_if_some("thumbnail", json_string(item, "poster"));
        info.insert_if_some("duration", json_i64(item, "duration"));
        info.insert_if_some("uploader_id", json_string(item, "account"));
        info.insert(
            "url",
            first.get("url").cloned().unwrap_or(serde_json::Value::Null),
        );
        info.insert(
            "ext",
            first
                .get("ext")
                .cloned()
                .unwrap_or_else(|| serde_json::json!("mp4")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}

/// Native KrasView page extractor.
pub struct KrasViewExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl KrasViewExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for KrasViewExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "KrasView URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let flashvars = json_object_after_marker(&html, "video_Init(").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("KrasView video {video_id} has no player data"),
            )
        })?;
        let media_url = json_string(&flashvars, "url").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("KrasView video {video_id} has no media URL"),
            )
        })?;
        let ext = yt_dlp_core::determine_ext(Some(media_url), "mp4");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!(ext.clone()));
        info.insert_if_some(
            "title",
            html_meta_value(&html, "og:title").or_else(|| html_title_value(&html)),
        );
        info.insert_if_some("description", html_meta_value(&html, "og:description"));
        info.insert_if_some(
            "thumbnail",
            json_string(&flashvars, "image")
                .map(str::to_owned)
                .or_else(|| html_meta_value(&html, "og:image")),
        );
        info.insert_if_some("duration", json_i64(&flashvars, "duration"));
        info.insert_if_some(
            "width",
            html_meta_value(&html, "video:width").and_then(|value| value.parse::<i64>().ok()),
        );
        info.insert_if_some(
            "height",
            html_meta_value(&html, "video:height").and_then(|value| value.parse::<i64>().ok()),
        );
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "source",
                "ext": ext,
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}
