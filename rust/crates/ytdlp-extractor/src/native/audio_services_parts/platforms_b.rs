impl InfoExtractor for AcastChannelExtractor {
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
                "Acast channel URL did not match its native pattern",
            )
        })?;
        let show_slug = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Acast show has no ID")
            })?;
        let show = context.get_json(&format!(
            "https://feeder.acast.com/api/v1/shows/{show_slug}"
        ))?;
        let mut info = InfoDict::new();
        info.insert(
            "id",
            serde_json::json!(json_string(&show, "id").unwrap_or(show_slug)),
        );
        info.insert_if_some("title", json_string(&show, "title"));
        info.insert_if_some("description", json_string(&show, "description"));
        let show_info = show
            .as_object()
            .map(|show| {
                serde_json::json!({
                    "creator": show.get("author"),
                    "series": show.get("title"),
                })
            })
            .unwrap_or(serde_json::Value::Null);
        let entries = show
            .get("episodes")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Acast show response has no episodes array",
                )
            })?
            .iter()
            .filter_map(|episode| {
                let media_url = json_string(episode, "url")?;
                let episode_id =
                    json_string(episode, "id").or_else(|| json_string(episode, "episodeUrl"))?;
                let title = json_string(episode, "title").unwrap_or(episode_id);
                let ext = yt_dlp_core::determine_ext(Some(media_url), "mp3");
                let mut entry = InfoDict::new();
                entry.insert("id", serde_json::json!(episode_id));
                entry.insert("title", serde_json::json!(title));
                entry.insert("url", serde_json::json!(media_url));
                entry.insert("ext", serde_json::json!(ext.clone()));
                entry.insert(
                    "formats",
                    serde_json::json!([{
                        "url": media_url,
                        "format_id": "audio",
                        "ext": ext,
                        "vcodec": "none",
                    }]),
                );
                entry.insert_if_some("description", json_string(episode, "description"));
                entry.insert_if_some("thumbnail", json_string(episode, "image"));
                if let Some(value) = episode.get("duration").and_then(|value| value.as_f64()) {
                    entry.insert("duration", serde_json::json!(value));
                }
                if let Some(value) = show_info.get("creator").and_then(|value| value.as_str()) {
                    entry.insert("creator", serde_json::json!(value));
                }
                if let Some(value) = show_info.get("series").and_then(|value| value.as_str()) {
                    entry.insert("series", serde_json::json!(value));
                }
                Some(entry)
            })
            .collect::<Vec<_>>();
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

impl InfoExtractor for AcastExtractor {
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
                "Acast URL did not match its native pattern",
            )
        })?;
        let channel = captures
            .name("channel")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Acast URL has no channel")
            })?;
        let display_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Acast URL has no episode ID",
                )
            })?;
        let mut api_request = Request::new(format!(
            "https://feeder.acast.com/api/v1/shows/{channel}/episodes/{display_id}"
        ));
        api_request.update_query(&[("showInfo".to_owned(), "true".to_owned())]);
        let episode = context.get_json(api_request.url())?;
        let episode_url = json_string(&episode, "url").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Acast episode has no media URL",
            )
        })?;
        let ext = yt_dlp_core::determine_ext(Some(episode_url), "mp3");
        let title = json_string(&episode, "title")
            .map(str::to_owned)
            .unwrap_or_else(|| display_id.to_owned());
        let mut info = InfoDict::new();
        info.insert(
            "id",
            serde_json::json!(json_string(&episode, "id").unwrap_or(display_id)),
        );
        info.insert("display_id", serde_json::json!(display_id));
        info.insert("title", serde_json::json!(title.clone()));
        info.insert("episode", serde_json::json!(title));
        info.insert("url", serde_json::json!(episode_url));
        info.insert("ext", serde_json::json!(ext.clone()));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": episode_url,
                "format_id": "audio",
                "ext": ext,
                "vcodec": "none",
            }]),
        );
        info.insert_if_some("description", json_string(&episode, "description"));
        info.insert_if_some("thumbnail", json_string(&episode, "image"));
        info.insert_if_some("duration", json_f64(&episode, "duration"));
        info.insert_if_some("filesize", json_f64(&episode, "contentLength"));
        if let Some(show) = episode.get("show") {
            info.insert_if_some("creator", json_string(show, "author"));
            info.insert_if_some("series", json_string(show, "title"));
        }
        for (source, target) in [("season", "season_number"), ("episode", "episode_number")] {
            if let Some(value) = episode.get(source).and_then(|value| value.as_i64()) {
                info.insert(target, serde_json::json!(value));
            }
        }
        Ok(ExtractorResult::single(info))
    }
}
