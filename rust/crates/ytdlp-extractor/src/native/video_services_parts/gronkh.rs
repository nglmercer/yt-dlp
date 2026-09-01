/// Native Gronkh VOD metadata and HLS extractor.
pub struct GronkhExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl GronkhExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for GronkhExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Gronkh URL has no ID")
            })?;
        let info_data = context.get_json(&format!(
            "https://api.gronkh.tv/v1/video/info?episode={video_id}"
        ))?;
        let playlist_data = context.get_json(&format!(
            "https://api.gronkh.tv/v1/video/playlist?episode={video_id}"
        ))?;
        let playlist_url = json_string(&playlist_data, "playlist_url")
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Gronkh video {video_id} has no HLS playlist"),
                )
            })?;
        let formats = vec![serde_json::json!({
            "url": playlist_url,
            "format_id": "hls",
            "protocol": "m3u8_native",
            "ext": "mp4",
        })];
        let mut subtitles = serde_json::Map::new();
        if let Some(vtt_url) = json_string(&info_data, "vtt_url")
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
        {
            subtitles.insert(
                "en".to_owned(),
                serde_json::json!([{
                    "url": vtt_url,
                    "ext": "vtt",
                }]),
            );
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(&info_data, "title"));
        info.insert_if_some("view_count", json_i64(&info_data, "views"));
        info.insert_if_some("thumbnail", json_string(&info_data, "preview_url"));
        info.insert_if_some(
            "upload_date",
            json_string(&info_data, "created_at").and_then(gronkh_date),
        );
        info.insert_if_some("duration", json_f64(&info_data, "source_length"));
        info.insert_if_some("chapters", gronkh_chapters(&info_data));
        info.insert("url", serde_json::json!(playlist_url));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", serde_json::Value::Object(subtitles));
        Ok(ExtractorResult::single(info))
    }
}

fn gronkh_date(value: &str) -> Option<String> {
    let matcher = Regex::new(r#"(?P<year>[0-9]{4})-(?P<month>[0-9]{2})-(?P<day>[0-9]{2})"#).ok()?;
    let captures = matcher.captures(value).ok().flatten()?;
    Some(format!(
        "{}{}{}",
        captures.name("year")?.as_str(),
        captures.name("month")?.as_str(),
        captures.name("day")?.as_str()
    ))
}

fn gronkh_chapters(data: &serde_json::Value) -> Option<Vec<serde_json::Value>> {
    let chapters = data.get("chapters")?.as_array()?;
    let chapters = chapters
        .iter()
        .filter_map(|chapter| {
            let offset = json_f64(chapter, "offset")?;
            let title = json_string(chapter, "title")?;
            Some(serde_json::json!({
                "title": title,
                "start_time": offset,
            }))
        })
        .collect::<Vec<_>>();
    (!chapters.is_empty()).then_some(chapters)
}
