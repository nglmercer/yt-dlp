/// Native eBay item-page video extractor.
///
/// eBay embeds a video object whose playlist map contains HLS and DASH
/// manifests. The manifests are passed directly to the native downloader,
/// which performs the actual playlist expansion.
pub struct EbayExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl EbayExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for EbayExtractor {
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
                "eBay URL did not match its native pattern",
            )
        })?;
        let item_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "eBay URL has no item ID")
            })?;
        let response = context.get(url)?;
        let html = String::from_utf8_lossy(response.body());
        let video = json_object_after_marker(&html, "\"video\":").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("eBay item {item_id} has no embedded video object"),
            )
        })?;
        let playlist_map = video
            .get("playlistMap")
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("eBay item {item_id} has no playlist map"),
                )
            })?;

        let mut formats = Vec::new();
        for (playlist_type, format_id, protocol) in [
            ("HLS", "hls", "m3u8_native"),
            ("DASH", "dash", "http_dash_segments"),
        ] {
            let Some(manifest_url) = playlist_map
                .get(playlist_type)
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            formats.push(serde_json::json!({
                "format_id": format_id,
                "url": manifest_url,
                "ext": "mp4",
                "protocol": protocol,
            }));
        }
        let first = formats.first().cloned().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("eBay item {item_id} has no playable HLS or DASH manifest"),
            )
        })?;
        let title = html_title_value(&html)
            .map(|title| title.strip_suffix(" | eBay").unwrap_or(&title).to_owned())
            .filter(|title| !title.is_empty())
            .unwrap_or_else(|| item_id.clone());

        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(item_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("url", first.get("url").and_then(serde_json::Value::as_str));
        info.insert_if_some("ext", first.get("ext").and_then(serde_json::Value::as_str));
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}
