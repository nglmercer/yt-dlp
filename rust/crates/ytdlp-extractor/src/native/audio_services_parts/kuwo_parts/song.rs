/// Native Kuwo song extractor.
pub struct KuwoExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl KuwoExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for KuwoExtractor {
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
        let song_id = kuwo_match_id(&self.matcher, url, "song")?;
        let (webpage, response_url) = kuwo_page(context, url, "song detail")?;
        if !response_url.contains(&song_id)
            || webpage.contains("对不起，该歌曲由于版权问题已被下线，将返回网站首页")
        {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!(
                    "TODO: Kuwo song {song_id} is offline because of copyright restrictions"
                ),
            ));
        }
        let song_name = kuwo_song_name(&webpage).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Kuwo song {song_id} has no song name"),
            )
        })?;
        let formats = kuwo_formats(context, &song_id, false, false)?;
        let publish_date = if let Some(album_id) = kuwo_album_id(&webpage) {
            let album_url = format!("http://www.kuwo.cn/album/{album_id}/");
            kuwo_page(context, &album_url, "album detail")
                .ok()
                .and_then(|(album_page, _)| kuwo_publish_date(&album_page))
        } else {
            None
        };
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(song_id));
        info.insert("title", serde_json::json!(song_name));
        info.insert_if_some("creator", kuwo_singer_name(&webpage));
        info.insert_if_some("upload_date", publish_date);
        info.insert_if_some("description", kuwo_lyrics(&webpage));
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}

/// Native Kuwo MV extractor.
pub struct KuwoMvExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl KuwoMvExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for KuwoMvExtractor {
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
        let song_id = kuwo_match_id(&self.matcher, url, "MV")?;
        let (webpage, _) = kuwo_page(context, url, "MV detail")?;
        let matcher = Regex::new(
            r#"(?is)<h1[^>]+title\s*=\s*["']([^"']+)["'][^>]*>[^<]*<span[^>]+title\s*=\s*["']([^"']+)["']"#,
        )
        .map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Kuwo MV title matcher: {error}"),
            )
        })?;
        let captures = matcher.captures(&webpage).ok().flatten().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Kuwo MV {song_id} has no song or singer name"),
            )
        })?;
        let song_name = captures
            .get(1)
            .map(|value| html_text_fragment(value.as_str()))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Kuwo MV {song_id} has no song name"),
                )
            })?;
        let singer_name = captures
            .get(2)
            .map(|value| html_text_fragment(value.as_str()))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Kuwo MV {song_id} has no singer name"),
                )
            })?;
        let mut formats = kuwo_formats(context, &song_id, true, true)?;
        let mv_url = kuwo_text_request(
            context,
            &format!("http://www.kuwo.cn/yy/st/mvurl?rid=MUSIC_{song_id}"),
            "MV URL",
        )?;
        if let Some(mv_url) = kuwo_http_url(&mv_url) {
            formats.push(serde_json::json!({
                "url": mv_url,
                "format_id": "mv",
            }));
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(song_id));
        info.insert("title", serde_json::json!(song_name));
        info.insert("creator", serde_json::json!(singer_name));
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}
