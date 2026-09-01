/// Native Huya VOD API/HLS extractor.
pub struct HuyaVideoExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl HuyaVideoExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for HuyaVideoExtractor {
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
            .ok_or_else(|| ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Huya VOD URL has no ID"))?;
        let mut request = Request::new("https://liveapi.huya.com/moment/getMomentContent");
        request.update_query(&[("videoId".to_owned(), video_id.clone())]);
        let response = context.request(&request)?;
        let root = serde_json::from_slice::<serde_json::Value>(response.body()).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Huya VOD JSON for {video_id}: {error}"),
            )
        })?;
        let moment = root
            .get("data")
            .and_then(|data| data.get("moment"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Huya VOD {video_id} has no moment data"),
                )
            })?;
        let video_info = moment
            .get("videoInfo")
            .unwrap_or(&serde_json::Value::Null);
        let definitions = video_info
            .get("definitions")
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Huya VOD {video_id} has no rendition definitions"),
                )
            })?;
        let mut formats = Vec::new();
        for (definition_id, definition) in definitions {
            let Some(media_url) = json_string(definition, "m3u8").filter(|value| {
                value.starts_with("http://") || value.starts_with("https://")
            }) else {
                continue;
            };
            let mut format = serde_json::json!({
                "url": media_url,
                "format_id": json_string(definition, "defName").unwrap_or(definition_id),
                "protocol": "m3u8_native",
                "ext": "mp4",
            });
            for (source_key, output_key) in [
                ("size", "filesize"),
                ("height", "height"),
                ("width", "width"),
                ("definition", "quality"),
            ] {
                if let Some(value) = json_i64(definition, source_key) {
                    format[output_key] = serde_json::json!(value);
                }
            }
            formats.push(format);
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Huya VOD {video_id} has no playable HLS definitions"),
            ));
        }
        let first_format = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert(
            "url",
            first_format
                .get("url")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
        info.insert("ext", serde_json::json!("mp4"));
        info.insert_if_some(
            "title",
            json_string(video_info, "videoTitle").map(str::to_owned),
        );
        info.insert_if_some(
            "categories",
            huya_string_list(video_info.get("category")),
        );
        info.insert_if_some(
            "tags",
            huya_string_list(video_info.get("tags")),
        );
        info.insert_if_some(
            "duration",
            json_value_string(video_info.get("videoDuration"))
                .and_then(|value| yt_dlp_core::parse_duration(&value)),
        );
        info.insert_if_some("uploader", json_string(video_info, "nickName"));
        info.insert_if_some(
            "uploader_id",
            json_value_string(video_info.get("uid")),
        );
        info.insert_if_some("view_count", json_i64(video_info, "videoPlayNum"));
        info.insert_if_some("comment_count", json_i64(moment, "commentCount"));
        info.insert_if_some("like_count", json_i64(moment, "favorCount"));
        info.insert_if_some(
            "description",
            json_string(moment, "content").map(html_text_fragment),
        );
        info.insert_if_some(
            "timestamp",
            json_i64(moment, "cTime"),
        );
        info.insert_if_some(
            "thumbnail",
            json_string(video_info, "videoBigCover")
                .or_else(|| json_string(video_info, "videoCover"))
                .map(huya_remove_query),
        );
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

fn huya_string_list(value: Option<&serde_json::Value>) -> Option<serde_json::Value> {
    let values = match value? {
        serde_json::Value::Array(values) => values
            .iter()
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect::<Vec<_>>(),
        serde_json::Value::String(value) => vec![value.to_owned()],
        _ => Vec::new(),
    };
    (!values.is_empty()).then(|| {
        serde_json::Value::Array(
            values
                .into_iter()
                .map(serde_json::Value::String)
                .collect(),
        )
    })
}

fn huya_remove_query(value: &str) -> String {
    let Ok(mut url) = url::Url::parse(value) else {
        return value.to_owned();
    };
    url.set_query(None);
    url.to_string()
}
