pub struct LitvExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl LitvExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for LitvExtractor {
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
            ExtractorError::new(ExtractorErrorKind::InvalidUrl, "LiTV URL has no match")
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "LiTV URL has no ID")
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let next_data = html_script_json(&webpage, "__NEXT_DATA__")?;
        let vod_data = next_data
            .get("props")
            .and_then(|props| props.get("pageProps"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("LiTV page {video_id} has no pageProps state"),
                )
            })?;
        let null = serde_json::Value::Null;
        let program_info = vod_data
            .get("programInformation")
            .filter(|value| value.is_object())
            .unwrap_or(&null);
        if !litv_force_no_playlist(url) {
            if let Some(playlist_data) = vod_data.get("seriesTree") {
                let entries = litv_playlist_entries(playlist_data);
                let playlist_id = json_string(playlist_data, "content_id")
                    .unwrap_or(&video_id)
                    .to_owned();
                let mut info = InfoDict::new();
                info.insert("id", serde_json::json!(playlist_id));
                info.insert_if_some("title", json_string(playlist_data, "title"));
                return Ok(ExtractorResult::Playlist { info, entries });
            }
        }
        let (asset_id, media_type) = if let Some(asset_id) = program_info
            .get("assets")
            .and_then(serde_json::Value::as_array)
            .and_then(|assets| assets.first())
            .and_then(|asset| json_string(asset, "asset_id"))
        {
            (asset_id.to_owned(), "vod".to_owned())
        } else {
            (
                json_string(program_info, "content_id")
                    .unwrap_or(&video_id)
                    .to_owned(),
                json_string(program_info, "content_type")
                    .unwrap_or("live")
                    .to_owned(),
            )
        };
        let video_data = litv_playback_json(context, &video_id, &asset_id, &media_type)?;
        let formats = litv_formats(&video_data, &video_id)?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some("title", litv_program_title(program_info));
        info.insert_if_some("description", json_string(program_info, "description"));
        info.insert_if_some(
            "thumbnail",
            json_string(program_info, "picture")
                .map(|value| resolve_url("https://p-cdnstatic.svc.litv.tv/", value)),
        );
        if let Some(genres) = program_info
            .get("genres")
            .and_then(serde_json::Value::as_array)
        {
            let categories = genres
                .iter()
                .filter_map(|genre| json_string(genre, "name"))
                .map(str::to_owned)
                .collect::<Vec<_>>();
            if !categories.is_empty() {
                info.insert("categories", serde_json::json!(categories));
            }
        }
        info.insert_if_some("episode_number", json_i64(program_info, "episode"));
        Ok(ExtractorResult::single(info))
    }
}
