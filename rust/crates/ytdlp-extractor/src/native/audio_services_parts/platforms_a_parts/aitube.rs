impl InfoExtractor for AitubeExtractor {
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
                format!("invalid Aitube URL: {error}"),
            )
        })?;
        let video_id = parsed
            .query_pairs()
            .find(|(name, _)| name == "id")
            .map(|(_, value)| value.into_owned())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Aitube URL has no id query")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let next_data = html_script_json(&html, "__NEXT_DATA__")?;
        let video_info = next_data
            .get("props")
            .and_then(|props| props.get("pageProps"))
            .and_then(|page_props| page_props.get("videoInfo"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Aitube page has no videoInfo data",
                )
            })?;
        let hls_url = format!(
            "https://api-http.aitube.kz/kz.aitudala.aitube.staticaccess/video/{video_id}/video"
        );
        let fallback_title = html_meta_value(&html, "og:title");
        let title = json_string(video_info, "title")
            .or(fallback_title.as_deref())
            .unwrap_or(&video_id)
            .to_owned();
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("url", serde_json::json!(hls_url.clone()));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": hls_url,
                "format_id": "hls",
                "ext": "mp4",
                "protocol": "m3u8_native",
            }]),
        );
        info.insert_if_some("description", json_string(video_info, "description"));
        for (source, target) in [
            ("viewCount", "view_count"),
            ("likeCount", "like_count"),
            ("commentCount", "comment_count"),
            ("channelSubscriberCount", "channel_follower_count"),
        ] {
            if let Some(value) = video_info.get(source) {
                info.insert(target, value.clone());
            }
        }
        for (source, target) in [
            ("channelTitle", "channel"),
            ("channelId", "channel_id"),
            ("coverUrl", "thumbnail"),
        ] {
            info.insert_if_some(target, json_string(video_info, source));
        }
        Ok(ExtractorResult::single(info))
    }
}
