/// Native DR Bonanza HTML5-player and current-asset extractor.
pub struct DrBonanzaExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl DrBonanzaExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for DrBonanzaExtractor {
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
        let captures = self.matcher.captures(url).ok().flatten().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "DR Bonanza URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "DR Bonanza URL has no ID")
            })?;
        let display_id = captures
            .name("display_id")
            .map(|value| value.as_str().to_owned())
            .unwrap_or_else(|| video_id.clone());
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let asset = json_object_after_marker(&webpage, "currentAsset").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("DR Bonanza video {video_id} has no currentAsset data"),
            )
        })?;
        let asset_title = json_string(&asset, "AssetTitle")
            .map(unescape_html_attribute)
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("DR Bonanza video {video_id} has no asset title"),
                )
            })?;
        let mut formats = html5_media_formats(url, &webpage);
        for format in &mut formats {
            if format.get("protocol").and_then(serde_json::Value::as_str) == Some("m3u8_native") {
                format["format_id"] = serde_json::json!("hls");
                format["ext"] = serde_json::json!("mp4");
            }
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("DR Bonanza video {video_id} has no HTML5 media sources"),
            ));
        }
        let first_format = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut output = InfoDict::new();
        output.insert(
            "id",
            json_string(&asset, "AssetId")
                .map(str::to_owned)
                .map_or_else(|| serde_json::json!(video_id), |value| serde_json::json!(value)),
        );
        output.insert("display_id", serde_json::json!(display_id));
        output.insert("title", serde_json::json!(asset_title));
        output.insert_if_some(
            "description",
            drbonanza_field(&webpage, "Programinfo")
                .map(|value| html_text_fragment(&value))
                .filter(|value| !value.is_empty()),
        );
        output.insert_if_some(
            "duration",
            drbonanza_field(&webpage, "Tid")
                .and_then(|value| yt_dlp_core::parse_duration(value.trim())),
        );
        output.insert_if_some(
            "thumbnail",
            json_string(&asset, "AssetImageUrl").map(str::to_owned),
        );
        output.insert(
            "url",
            first_format
                .get("url")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
        output.insert(
            "ext",
            first_format
                .get("ext")
                .cloned()
                .unwrap_or_else(|| serde_json::json!("mp4")),
        );
        output.insert("formats", serde_json::Value::Array(formats));
        output.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(output))
    }
}

fn drbonanza_field(html: &str, field: &str) -> Option<String> {
    let pattern = format!(
        r"(?is)<div[^>]+>\s*<p>{}:<p>\s*</div>\s*<div[^>]+>\s*<p>([^<]+)",
        regex::escape(field)
    );
    Regex::new(&pattern)
        .ok()?
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().trim().to_owned())
}
