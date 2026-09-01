/// Native KiKA.de video extractor backed by the Next.js proxy API.
pub struct KikaExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl KikaExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for KikaExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "KiKA URL has no video ID")
            })?;
        let doc = context.get_json(&format!(
            "https://www.kika.de/_next-api/proxy/v1/videos/{video_id}"
        ))?;
        let assets_url = doc
            .get("assets")
            .and_then(|assets| json_string(assets, "url"))
            .and_then(kika_http_url)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("KiKA video {video_id} has no assets API URL"),
                )
            })?;
        let assets = context.get_json(&assets_url)?;
        let formats = kika_formats(&assets);
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("KiKA video {video_id} has no playable media assets"),
            ));
        }
        let subtitles = kika_subtitles(&doc, &assets);
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(&doc, "title"));
        info.insert_if_some("description", json_string(&doc, "description"));
        info.insert_if_some(
            "timestamp",
            json_string(&doc, "date").and_then(|value| parse_timestamp(value.to_owned())),
        );
        info.insert_if_some(
            "modified_timestamp",
            json_string(&doc, "modificationDate")
                .and_then(|value| parse_timestamp(value.to_owned())),
        );
        if let Some(seconds) = json_i64(&doc, "durationInSeconds") {
            info.insert("duration", serde_json::json!(seconds));
        } else {
            info.insert_if_some(
                "duration",
                json_string(&doc, "duration").and_then(yt_dlp_core::parse_duration),
            );
        }
        info.insert_if_some("episode_number", json_i64(&doc, "episodeNumber"));
        info.insert_if_some("season_number", json_i64(&doc, "season"));
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
        info.insert("subtitles", subtitles);
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

fn kika_formats(media_info: &serde_json::Value) -> Vec<serde_json::Value> {
    let mut formats = Vec::new();
    for media in media_info
        .get("assets")
        .into_iter()
        .flat_map(json_object_values)
    {
        let Some(stream_url) = json_string(media, "url")
            .and_then(kika_http_url)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let extension = yt_dlp_core::determine_ext(Some(&stream_url), "mp4").to_ascii_lowercase();
        if extension == "m3u8" {
            formats.push(serde_json::json!({
                "url": stream_url,
                "format_id": "hls",
                "ext": "mp4",
                "protocol": "m3u8_native",
            }));
            continue;
        }
        let mut format = serde_json::json!({
            "url": stream_url,
            "format_id": extension,
        });
        if let Some(object) = format.as_object_mut() {
            if let Some(width) = json_i64(media, "frameWidth") {
                object.insert("width".to_owned(), serde_json::json!(width));
            }
            if let Some(height) = json_i64(media, "frameHeight") {
                object.insert("height".to_owned(), serde_json::json!(height));
            }
            if let Some(filesize) = json_i64(media, "fileSize").filter(|value| *value != 0) {
                object.insert("filesize".to_owned(), serde_json::json!(filesize));
            }
            if let Some(abr) = json_i64(media, "bitrateAudio").filter(|value| *value != -1) {
                object.insert("abr".to_owned(), serde_json::json!(abr));
            }
            if let Some(vbr) = json_i64(media, "bitrateVideo").filter(|value| *value != -1) {
                object.insert("vbr".to_owned(), serde_json::json!(vbr));
            }
        }
        formats.push(format);
    }
    formats
}

fn kika_subtitles(doc: &serde_json::Value, assets: &serde_json::Value) -> serde_json::Value {
    if !json_bool(doc, "hasSubtitle").unwrap_or(false) {
        return serde_json::json!({});
    }
    let mut tracks = Vec::new();
    if let Some(url) = json_string(assets, "videoSubtitle").and_then(kika_http_url) {
        tracks.push(serde_json::json!({"url": url, "ext": "ttml"}));
    }
    if let Some(url) = json_string(assets, "webvttUrl").and_then(kika_http_url) {
        tracks.push(serde_json::json!({"url": url, "ext": "vtt"}));
    }
    if tracks.is_empty() {
        serde_json::json!({})
    } else {
        serde_json::json!({"de": tracks})
    }
}

fn kika_http_url(value: &str) -> Option<String> {
    let value = value.trim();
    (value.starts_with("http://") || value.starts_with("https://"))
        .then(|| value.to_owned())
}
