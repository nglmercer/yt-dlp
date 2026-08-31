impl InfoExtractor for AudiodraftExtractor {
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
                "Audiodraft URL did not match its native pattern",
            )
        })?;
        let entry_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Audiodraft URL has no ID")
            })?;
        let mut request =
            Request::new("https://www.audiodraft.com/scripts/general/player/getPlayerInfoNew.php");
        request.set_method("POST").map_err(map_request_error)?;
        request.headers_mut().set(
            "Content-Type",
            "application/x-www-form-urlencoded; charset=UTF-8",
        );
        request
            .headers_mut()
            .set("X-Requested-With", "XMLHttpRequest");
        request.set_data(Some(format!("id=player_entry_{entry_id}").into_bytes()));
        let response = context.request(&request)?;
        let data: serde_json::Value = serde_json::from_slice(response.body()).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Audiodraft response: {error}"),
            )
        })?;
        let media_url = json_string(&data, "path").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Audiodraft response has no media path",
            )
        })?;
        let mut info = InfoDict::new();
        info.insert(
            "id",
            serde_json::json!(
                json_value_string(data.get("entry_id")).unwrap_or_else(|| entry_id.to_owned())
            ),
        );
        info.insert_if_some("title", json_string(&data, "entry_title"));
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!("mp3"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "mp3",
                "ext": "mp3",
                "vcodec": "none",
                "acodec": "mp3",
            }]),
        );
        info.insert_if_some("uploader", json_string(&data, "designer_name"));
        info.insert_if_some("uploader_id", json_value_string(data.get("designer_id")));
        info.insert_if_some("webpage_url", json_string(&data, "entry_url"));
        info.insert_if_some("like_count", json_i64(&data, "entry_likes"));
        info.insert_if_some("average_rating", json_i64(&data, "entry_rating"));
        Ok(ExtractorResult::single(info))
    }
}
