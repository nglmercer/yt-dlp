/// Native EbaumsWorld XML player extractor.
pub struct EbaumsWorldExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl EbaumsWorldExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for EbaumsWorldExtractor {
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
                "EbaumsWorld URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "EbaumsWorld URL has no ID")
            })?;
        let response = context.get(&format!(
            "http://www.ebaumsworld.com/video/player/{video_id}"
        ))?;
        let xml = String::from_utf8_lossy(response.body());
        let media_url = xml_element_text(&xml, "file").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("EbaumsWorld video {video_id} has no media URL"),
            )
        })?;
        let extension = yt_dlp_core::determine_ext(Some(&media_url), "mp4");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert(
            "title",
            serde_json::json!(xml_element_text(&xml, "title").unwrap_or_else(|| video_id.clone())),
        );
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!(extension));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "http",
                "ext": extension,
            }]),
        );
        info.insert_if_some("description", xml_element_text(&xml, "description"));
        info.insert_if_some("thumbnail", xml_element_text(&xml, "image"));
        info.insert_if_some("uploader", xml_element_text(&xml, "username"));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

/// Native Fuyin TV API extractor.
pub struct FuyinTvExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FuyinTvExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FuyinTvExtractor {
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
                "Fuyin TV URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Fuyin TV URL has no ID")
            })?;
        let api = native_get_json_with_headers(
            context,
            &format!("https://www.fuyin.tv/api/api/tv.movie/url?urlid={video_id}"),
            &[("Accept", "application/json")],
        )?;
        let data = api.get("data").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Fuyin TV API response has no data object",
            )
        })?;
        let media_url = json_string(data, "url").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Fuyin TV video {video_id} has no media URL"),
            )
        })?;
        let webpage = context
            .get(url)
            .ok()
            .map(|response| String::from_utf8_lossy(response.body()).into_owned())
            .unwrap_or_default();
        let extension = yt_dlp_core::determine_ext(Some(media_url), "mp4");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(data, "title"));
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!(extension));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "http",
                "ext": extension,
            }]),
        );
        info.insert_if_some("description", html_meta_value(&webpage, "description"));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

/// Native CAM4 live HLS extractor.
pub struct Cam4Extractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl Cam4Extractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for Cam4Extractor {
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
                "CAM4 URL did not match its native pattern",
            )
        })?;
        let channel_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "CAM4 URL has no ID")
            })?;
        let data = native_get_json_with_headers(
            context,
            &format!("https://www.cam4.com/rest/v1.0/profile/{channel_id}/streamInfo"),
            &[("Accept", "application/json")],
        )?;
        let playlist_url = json_string(&data, "cdnURL").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("CAM4 channel {channel_id} has no live stream URL"),
            )
        })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(channel_id));
        info.insert("title", serde_json::json!(channel_id));
        info.insert("url", serde_json::json!(playlist_url));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": playlist_url,
                "format_id": "hls",
                "ext": "mp4",
                "protocol": "m3u8_native",
            }]),
        );
        info.insert("is_live", serde_json::json!(true));
        info.insert("live_status", serde_json::json!("is_live"));
        info.insert("age_limit", serde_json::json!(18));
        info.insert(
            "thumbnail",
            serde_json::json!(format!(
                "https://snapshots.xcdnpro.com/thumbnails/{channel_id}"
            )),
        );
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

/// Native Kommunetv stream API extractor.
pub struct KommunetvExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl KommunetvExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for KommunetvExtractor {
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
                "Kommunetv URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Kommunetv URL has no ID")
            })?;
        let host = url::Url::parse(url)
            .ok()
            .and_then(|value| value.host_str().map(str::to_owned))
            .unwrap_or_else(|| "oslo.kommunetv.no".to_owned());
        let data = native_get_json_with_headers(
            context,
            &format!("https://{host}/api/streams?streamType=1&id={video_id}"),
            &[("Accept", "application/json")],
        )?;
        let title = data
            .get("stream")
            .and_then(|stream| json_string(stream, "title"))
            .unwrap_or(video_id.as_str());
        let playlist_url = data
            .get("playlist")
            .and_then(serde_json::Value::as_array)
            .and_then(|playlist| playlist.first())
            .and_then(|playlist| playlist.get("playlist"))
            .and_then(serde_json::Value::as_array)
            .and_then(|playlist| playlist.first())
            .and_then(|playlist| json_string(playlist, "file"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Kommunetv stream {video_id} has no playlist URL"),
                )
            })?;
        let mut parsed_playlist = url::Url::parse(playlist_url).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Kommunetv playlist URL: {error}"),
            )
        })?;
        parsed_playlist.set_query(None);
        parsed_playlist.set_fragment(None);
        let playlist_url = parsed_playlist.to_string();
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("url", serde_json::json!(playlist_url));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": playlist_url,
                "format_id": "hls",
                "ext": "mp4",
                "protocol": "m3u8_native",
            }]),
        );
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}
