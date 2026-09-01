/// Native KompasVideo/Jixie API extractor.
pub struct KompasVideoExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl KompasVideoExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for KompasVideoExtractor {
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
                "KompasVideo URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "KompasVideo URL has no video ID",
                )
            })?;
        let display_id = captures
            .name("slug")
            .map(|value| value.as_str().to_owned())
            .unwrap_or_else(|| video_id.clone());
        let webpage = String::from_utf8_lossy(context.get(url)?.body()).into_owned();

        let mut api_request = Request::new("https://apidam.jixie.io/api/public/stream");
        api_request.update_query(&[
            ("metadata".to_owned(), "full".to_owned()),
            ("video_id".to_owned(), video_id.clone()),
        ]);
        let api_response = context.request(&api_request)?;
        let response: serde_json::Value =
            serde_json::from_slice(api_response.body()).map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid Jixie API JSON for {display_id}: {error}"),
                )
            })?;
        let data = response.get("data").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Jixie API response for {video_id} has no data"),
            )
        })?;
        let drm = json_bool(data, "drm").unwrap_or(false);
        let streams = data
            .get("streams")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Jixie video {video_id} has no streams"),
                )
            })?;
        let formats = streams
            .iter()
            .filter_map(|stream| kompas_stream_format(stream, drm))
            .collect::<Vec<_>>();
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Jixie video {video_id} has no playable streams"),
            ));
        }
        let metadata = data.get("metadata").unwrap_or(&serde_json::Value::Null);
        let title = json_string(data, "title")
            .map(str::to_owned)
            .or_else(|| html_meta_value(&webpage, "og:title"))
            .or_else(|| html_meta_value(&webpage, "twitter:title"))
            .unwrap_or_else(|| display_id.clone());
        let description = kompas_description(metadata.get("description"))
            .or_else(|| html_meta_value(&webpage, "description").map(|value| html_text_fragment(&value)))
            .or_else(|| html_meta_value(&webpage, "og:description").map(|value| html_text_fragment(&value)));
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("display_id", serde_json::json!(display_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", description);
        info.insert_if_some("thumbnails", kompas_thumbnails(metadata.get("thumbnails")));
        info.insert_if_some("duration", json_f64(metadata, "duration"));
        info.insert_if_some("tags", kompas_text_list(metadata.get("keywords")));
        info.insert_if_some("categories", kompas_text_list(metadata.get("categories")));
        info.insert_if_some("uploader_id", json_value_string(data.get("owner_id")));
        info.insert(
            "url",
            first
                .get("url")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
        info.insert(
            "ext",
            first
                .get("ext")
                .cloned()
                .unwrap_or_else(|| serde_json::json!("mp4")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", serde_json::json!({}));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}
