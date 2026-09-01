fn lnk_thumbnail(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }
    Some(if value.starts_with("http://") || value.starts_with("https://") {
        value.to_owned()
    } else {
        format!("https://lnk.lt/all-images/{}", value.trim_start_matches('/'))
    })
}

fn lnk_duration(value: Option<&serde_json::Value>) -> Option<f64> {
    value.and_then(|value| {
        value
            .as_f64()
            .or_else(|| value.as_i64().map(|value| value as f64))
            .or_else(|| value.as_str().and_then(|value| value.parse::<f64>().ok()))
    })
}

fn lnk_subtitles(video_info: &serde_json::Value) -> serde_json::Value {
    let mut subtitles = serde_json::Map::new();
    if let Some(url) = json_string(video_info, "subtitleUrl")
        .filter(|url| url.starts_with("http://") || url.starts_with("https://"))
    {
        subtitles.insert("lt".to_owned(), serde_json::json!([{"url": url}]));
    }
    serde_json::Value::Object(subtitles)
}

fn lnk_add_hls_format(
    formats: &mut Vec<serde_json::Value>,
    media_url: Option<&str>,
    format_id: &str,
) {
    let Some(media_url) = media_url
        .map(str::trim)
        .filter(|url| url.starts_with("http://") || url.starts_with("https://"))
    else {
        return;
    };
    if formats
        .iter()
        .any(|format| format.get("url").and_then(serde_json::Value::as_str) == Some(media_url))
    {
        return;
    }
    formats.push(serde_json::json!({
        "url": media_url,
        "format_id": format_id,
        "ext": "mp4",
        "protocol": "m3u8_native",
    }));
}

/// Native LNK.lt video configuration extractor.
pub struct LnkExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl LnkExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for LnkExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "LNK URL has no video ID")
            })?;
        let video_info = lnk_video_info(context, &video_id)?;
        let mut formats = Vec::new();
        lnk_add_hls_format(
            &mut formats,
            json_string(&video_info, "videoUrl"),
            "hls",
        );
        if !json_bool(&video_info, "drm").unwrap_or(false) {
            lnk_add_hls_format(
                &mut formats,
                json_string(&video_info, "videoFairplayUrl"),
                "fairplay",
            );
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("LNK video {video_id} has no playable HLS URL"),
            ));
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(&video_info, "title"));
        info.insert_if_some("description", json_string(&video_info, "description"));
        info.insert_if_some("view_count", json_i64(&video_info, "viewsCount"));
        info.insert_if_some("duration", lnk_duration(video_info.get("duration")));
        info.insert_if_some(
            "upload_date",
            json_string(&video_info, "airDate").and_then(date_digits),
        );
        info.insert_if_some(
            "thumbnail",
            lnk_thumbnail(json_string(&video_info, "posterImage")),
        );
        info.insert_if_some(
            "episode_number",
            json_i64(&video_info, "episodeNumber"),
        );
        info.insert_if_some("series", json_string(&video_info, "programTitle"));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", lnk_subtitles(&video_info));
        Ok(ExtractorResult::single(info))
    }
}
