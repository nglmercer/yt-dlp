/// Native IlPost podcast episode extractor.
pub struct IlPostExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl IlPostExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for IlPostExtractor {
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
        let display_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "IlPost URL has no episode ID")
            })?;
        let page_response = context.get(url)?;
        let webpage = String::from_utf8_lossy(page_response.body());
        let endpoint_metadata = json_object_after_marker(&webpage, "var ilpostpodcast").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("IlPost episode {display_id} has no podcast metadata"),
            )
        })?;
        let episode_id = json_value_string(endpoint_metadata.get("post_id")).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("IlPost episode {display_id} has no post ID"),
            )
        })?;
        let podcast_id = json_value_string(endpoint_metadata.get("podcast_id")).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("IlPost episode {display_id} has no podcast ID"),
            )
        })?;
        let ajax_url = json_string(&endpoint_metadata, "ajax_url")
            .map(|value| resolve_url(url, value))
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("IlPost episode {display_id} has no podcast API URL"),
                )
            })?;
        let mut form = url::form_urlencoded::Serializer::new(String::new());
        form.append_pair("action", "checkpodcast");
        if let Some(cookie) = json_string(&endpoint_metadata, "cookie") {
            form.append_pair("cookie", cookie);
        }
        form.append_pair("post_id", &episode_id);
        form.append_pair("podcast_id", &podcast_id);
        let mut request = Request::new(ajax_url);
        request.set_method("POST").map_err(map_request_error)?;
        request.set_data(Some(form.finish().into_bytes()));
        let response = context.request(&request)?;
        let podcast_metadata = serde_json::from_slice::<serde_json::Value>(response.body()).map_err(
            |error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid IlPost podcast JSON for {episode_id}: {error}"),
                )
            },
        )?;
        let episode = podcast_metadata
            .get("data")
            .and_then(|data| data.get("postcastList"))
            .and_then(serde_json::Value::as_array)
            .and_then(|episodes| {
                episodes.iter().find(|episode| {
                    json_value_string(episode.get("id")).as_deref() == Some(episode_id.as_str())
                })
            })
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("IlPost episode {episode_id} could not be extracted"),
                )
            })?;
        let media_url = json_string(episode, "podcast_raw_url")
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            .map(str::to_owned)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("IlPost episode {episode_id} has no podcast URL"),
                )
            })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(episode_id));
        info.insert("display_id", serde_json::json!(display_id));
        info.insert("series_id", serde_json::json!(podcast_id));
        info.insert("vcodec", serde_json::json!("none"));
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!("mp3"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "audio",
                "ext": "mp3",
                "vcodec": "none",
            }]),
        );
        info.insert_if_some("title", json_string(episode, "title"));
        info.insert_if_some("description", json_string(episode, "description"));
        info.insert_if_some("thumbnail", json_string(episode, "image"));
        info.insert_if_some("timestamp", json_i64(episode, "timestamp"));
        info.insert_if_some(
            "duration",
            json_f64(episode, "milliseconds").map(|value| value / 1000.0),
        );
        info.insert_if_some(
            "availability",
            json_bool(episode, "free")
                .map(|free| if free { "public" } else { "subscriber_only" }),
        );
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}
