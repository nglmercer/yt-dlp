/// Native MatchiTV Next.js/HLS extractor.
pub struct MatchiTvExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MatchiTvExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MatchiTvExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "MatchiTV URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let loaded_media = html_script_json(&html, "__NEXT_DATA__")
            .ok()
            .and_then(|data| data.get("props").cloned())
            .and_then(|props| props.get("pageProps").cloned())
            .and_then(|page_props| page_props.get("loadedMedia").cloned())
            .unwrap_or(serde_json::Value::Null);
        let court = json_string(&loaded_media, "courtDescription");
        let start = json_string(&loaded_media, "startDateTime");
        let title = match (court, start) {
            (Some(court), Some(start)) => format!("{court} {start}"),
            (Some(court), None) => court.to_owned(),
            (None, Some(start)) => start.to_owned(),
            (None, None) => video_id.clone(),
        };
        let media_url = format!(
            "https://streams.padelgo.tv/v2/streams/m3u8/{video_id}/anonymous/playlist.m3u8"
        );
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert(
            "thumbnail",
            serde_json::json!(format!("https://thumbnails.padelgo.tv/{video_id}.jpg")),
        );
        info.insert_if_some("upload_date", start.and_then(date_digits));
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "hls",
                "protocol": "m3u8_native",
                "ext": "mp4",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}
