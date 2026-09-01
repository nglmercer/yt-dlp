/// Native FranceTV API/HLS extractor.
pub struct FranceTvExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FranceTvExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FranceTvExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "FranceTV URL has no ID")
            })?;
        let mut videos = Vec::new();
        let mut title = None;
        let mut subtitle = None;
        let mut image = None;
        let mut duration = None;
        let mut timestamp = None;
        let mut season_number = None;
        let mut episode_number = None;
        let mut is_live = None;
        let mut drm_only = false;

        for (device_type, browser) in [("desktop", "chrome"), ("mobile", "safari")] {
            let Some(data) = francetv_fetch_json(context, &video_id, device_type, browser)? else {
                continue;
            };
            if let Some(video) = data.get("video").filter(|value| value.is_object()) {
                videos.push(video.clone());
                if duration.is_none() {
                    duration = json_f64(video, "duration");
                }
                if is_live.is_none() {
                    is_live = json_bool(video, "is_live");
                }
            } else if let Some(code) = json_i64(&data, "code") {
                match code {
                    2009 => {
                        return Err(ExtractorError::new(
                            ExtractorErrorKind::Unsupported,
                            format!(
                                "TODO: FranceTV video {video_id} is geo-restricted to France"
                            ),
                        ));
                    }
                    2015 | 2017 | 2019 => {
                        drm_only = true;
                        continue;
                    }
                    _ => continue,
                }
            }
            if let Some(meta) = data.get("meta").filter(|value| value.is_object()) {
                let (
                    response_title,
                    response_subtitle,
                    response_image,
                    response_timestamp,
                    response_season,
                    response_episode,
                ) = francetv_json_meta(meta);
                if title.is_none() {
                    title = response_title;
                }
                if subtitle.is_none() {
                    subtitle = response_subtitle;
                }
                if image.is_none() {
                    image = response_image;
                }
                if timestamp.is_none() {
                    timestamp = response_timestamp;
                }
                if season_number.is_none() {
                    season_number = response_season;
                }
                if episode_number.is_none() {
                    episode_number = response_episode;
                }
            }
        }

        let mut formats = Vec::new();
        for video in videos {
            if let Some(format) = francetv_format(context, &video, &video_id)? {
                formats.push(format);
            }
        }
        if formats.is_empty() {
            if drm_only {
                return Err(ExtractorError::new(
                    ExtractorErrorKind::Unsupported,
                    format!(
                        "TODO: FranceTV video {video_id} is DRM-only or requires an \
                         authenticated playback workflow"
                    ),
                ));
            }
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("FranceTV video {video_id} has no playable formats"),
            ));
        }
        let first_format = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", francetv_join_title(title.clone(), subtitle.clone()));
        info.insert_if_some("thumbnail", image);
        info.insert_if_some("duration", duration);
        info.insert_if_some("timestamp", timestamp);
        info.insert_if_some("is_live", is_live);
        info.insert_if_some(
            "episode",
            episode_number
                .is_some()
                .then(|| subtitle.clone())
                .flatten(),
        );
        info.insert_if_some(
            "series",
            episode_number.is_some().then(|| title.clone()).flatten(),
        );
        info.insert_if_some("episode_number", episode_number);
        info.insert_if_some("season_number", season_number);
        info.insert(
            "url",
            first_format
                .get("url")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
        info.insert(
            "ext",
            first_format
                .get("ext")
                .cloned()
                .unwrap_or_else(|| serde_json::json!("mp4")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}
