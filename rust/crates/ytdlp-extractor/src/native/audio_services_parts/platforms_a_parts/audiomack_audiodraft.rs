impl InfoExtractor for AudiomackExtractor {
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
        let parsed = url::Url::parse(url).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                format!("invalid Audiomack URL: {error}"),
            )
        })?;
        let path = parsed.path().trim_matches('/');
        let song_tag = path
            .split_once("song/")
            .map(|(_, tag)| tag)
            .filter(|tag| !tag.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Audiomack URL has no song path",
                )
            })?;
        let mut request = Request::new(format!(
            "http://www.audiomack.com/api/music/url/song/{song_tag}"
        ));
        request.update_query(&[("extended".to_owned(), "1".to_owned())]);
        let response = context.get_json(request.url())?;
        let media_url = json_string(&response, "url")
            .filter(|url| !url.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Audiomack API returned no song URL",
                )
            })?;
        if media_url.contains("soundcloud.com/") {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                "TODO: native SoundCloud wrapper extraction is not implemented",
            ));
        }
        let ext = yt_dlp_core::determine_ext(Some(media_url), "mp3");
        let id = json_value_string(response.get("id")).unwrap_or_else(|| {
            media_url
                .rsplit('/')
                .next()
                .unwrap_or(song_tag)
                .split('?')
                .next()
                .unwrap_or(song_tag)
                .trim_end_matches(&format!(".{ext}"))
                .to_owned()
        });
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(id));
        info.insert_if_some("uploader", json_string(&response, "artist"));
        info.insert_if_some("title", json_string(&response, "title"));
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!(ext.clone()));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "audio",
                "ext": ext,
                "vcodec": "none",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}
