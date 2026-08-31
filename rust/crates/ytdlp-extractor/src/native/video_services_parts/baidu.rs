/// Native Baidu Video API playlist extractor.
pub struct BaiduVideoExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl BaiduVideoExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for BaiduVideoExtractor {
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
                "Baidu Video URL did not match its native pattern",
            )
        })?;
        let category = captures
            .name("type")
            .map(|value| match value.as_str() {
                "show" => "tvshow",
                "tv" => "tvplay",
                value => value,
            })
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Baidu Video URL has no category",
                )
            })?;
        let playlist_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Baidu Video URL has no playlist ID",
                )
            })?;
        let detail = context.get_json(&format!(
            "http://app.video.baidu.com/xqinfo/?worktype=adnative{category}&id={playlist_id}"
        ))?;
        let title = json_string(&detail, "title")
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| playlist_id.clone());
        let description = json_string(&detail, "intro")
            .map(unescape_html_attribute)
            .filter(|value| !value.is_empty());
        let episodes = context.get_json(&format!(
            "http://app.video.baidu.com/xqsingle/?worktype=adnative{category}&id={playlist_id}"
        ))?;
        let videos = episodes
            .get("videos")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Baidu Video playlist {playlist_id} has no episode list"),
                )
            })?;
        let mut entries = Vec::new();
        for video in videos {
            let Some(entry_url) = json_string(video, "url").filter(|value| !value.is_empty())
            else {
                continue;
            };
            let mut entry = native_url_result(entry_url);
            entry.insert_if_some(
                "title",
                json_string(video, "title").map(unescape_html_attribute),
            );
            entries.push(entry);
        }
        if entries.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Baidu Video playlist {playlist_id} has no playable episodes"),
            ));
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(playlist_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", description);
        Ok(ExtractorResult::Playlist { info, entries })
    }
}
