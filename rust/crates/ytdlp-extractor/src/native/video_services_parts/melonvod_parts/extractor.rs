/// Native Melon VOD player/streaming API extractor.
pub struct MelonVodExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MelonVodExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MelonVodExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Melon VOD URL has no ID")
            })?;
        let play_info = melonvod_player_info(context, &video_id)?;
        let stream_info = melonvod_streaming_info(context, &video_id)?;
        let title = play_info
            .get("mvInfo")
            .and_then(|value| json_string(value, "MVTITLE"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Melon VOD {video_id} has no title"),
                )
            })?;
        let streaming = stream_info.get("streamingInfo").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Melon VOD {video_id} has no streamingInfo"),
            )
        })?;
        let media_url = json_string(streaming, "encUrl")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Melon VOD {video_id} has no HLS URL"),
                )
            })?
            .to_owned();
        let format = melonvod_hls_format(media_url.clone());
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", serde_json::Value::Array(vec![format]));
        info.insert_if_some("artist", melonvod_artist(&play_info));
        info.insert_if_some(
            "thumbnail",
            melonvod_thumbnail(
                json_string(&stream_info, "staticDomain"),
                json_string(streaming, "imgPath"),
            ),
        );
        info.insert_if_some("duration", json_i64(streaming, "playTime"));
        info.insert_if_some(
            "upload_date",
            melonvod_upload_date(json_string(streaming, "mvSvcOpenDt")),
        );
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}
