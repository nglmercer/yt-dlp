/// Native Murrtube video-page extractor.
pub struct MurrtubeExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MurrtubeExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MurrtubeExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Murrtube URL has no ID")
            })?;
        if url.starts_with("murrtube:") {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                "TODO: Murrtube prefix URLs require the native age/session flow",
            ));
        }
        murrtube_initialize(context)?;
        let page = murrtube_page(context, url)?;
        let playlist_url = murrtube_playlist_url(url, &page, &video_id)?;
        let stream_id = Regex::new(r#"(?i)/([\da-f]+)/index\.m3u8"#)
            .ok()
            .and_then(|matcher| matcher.captures(&playlist_url).ok().flatten())
            .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
            .unwrap_or(video_id.clone());
        let format = serde_json::json!({
            "url": playlist_url,
            "format_id": "hls",
            "ext": "mp4",
            "protocol": "m3u8_native",
        });
        let page_title = html_meta_value(&page, "og:title")
            .or_else(|| html_title_value(&page))
            .unwrap_or_else(|| stream_id.clone());
        let title = page_title
            .strip_suffix(" - Murrtube")
            .unwrap_or(&page_title)
            .to_owned();
        let first_url = format
            .get("url")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(stream_id));
        info.insert("title", serde_json::json!(title));
        info.insert("age_limit", serde_json::json!(18));
        info.insert("formats", serde_json::Value::Array(vec![format]));
        info.insert("url", serde_json::json!(first_url));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert_if_some("description", html_meta_value(&page, "og:description"));
        info.insert_if_some("thumbnail", murrtube_thumbnail(&page));
        info.insert_if_some("uploader", murrtube_uploader(&page));
        info.insert_if_some("view_count", murrtube_count(&page, "Views"));
        info.insert_if_some("like_count", murrtube_count(&page, "Likes"));
        info.insert_if_some("comment_count", murrtube_count(&page, "Comments"));
        Ok(ExtractorResult::single(info))
    }
}
