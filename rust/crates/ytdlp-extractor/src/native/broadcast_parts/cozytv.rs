/// Native CozyTV replay extractor.
pub struct CozyTvExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl CozyTvExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for CozyTvExtractor {
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
                "CozyTV URL did not match its native pattern",
            )
        })?;
        let uploader = captures
            .name("uploader")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "CozyTV URL has no uploader")
            })?;
        let date = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "CozyTV URL has no replay ID",
                )
            })?;
        let video_id = format!("{uploader}-{date}");
        let data = context.get_json(&format!(
            "https://api.cozy.tv/cache/{uploader}/replay/{date}"
        ))?;
        let media_url =
            format!("https://cozycdn.foxtrotstream.xyz/replays/{uploader}/{date}/index.m3u8");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(&data, "title"));
        info.insert(
            "uploader",
            serde_json::json!(json_string(&data, "user").unwrap_or(&uploader)),
        );
        info.insert(
            "upload_date",
            serde_json::json!(
                json_string(&data, "date")
                    .map(|value| {
                        value
                            .chars()
                            .filter(|character| character.is_ascii_digit())
                            .take(8)
                            .collect::<String>()
                    })
                    .filter(|value| value.len() == 8)
                    .unwrap_or_else(|| {
                        date.chars()
                            .filter(|character| character.is_ascii_digit())
                            .take(8)
                            .collect::<String>()
                    })
            ),
        );
        info.insert("was_live", serde_json::json!(true));
        info.insert_if_some("duration", json_i64(&data, "duration"));
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
