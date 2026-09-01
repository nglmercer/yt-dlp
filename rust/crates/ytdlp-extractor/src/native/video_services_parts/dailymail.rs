/// Native Daily Mail page/player API rendition extractor.
pub struct DailyMailExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl DailyMailExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for DailyMailExtractor {
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
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Daily Mail URL has no video ID",
                )
            })?;
        let page_response = context.get(url)?;
        let webpage = String::from_utf8_lossy(page_response.body());
        let video_data = html_data_json_attribute(&webpage, "opts").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Daily Mail video {video_id} has no player data"),
            )
        })?;
        let title = json_string(&video_data, "title")
            .map(unescape_html_attribute)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Daily Mail video {video_id} has no title"),
                )
            })?;
        let source_url = video_data
            .get("plugins")
            .and_then(|plugins| plugins.get("sources"))
            .and_then(|sources| json_string(sources, "url"))
            .or_else(|| {
                video_data
                    .get("sources")
                    .and_then(|sources| json_string(sources, "url"))
            })
            .map(str::to_owned)
            .unwrap_or_else(|| {
                format!(
                    "http://www.dailymail.co.uk/api/player/{video_id}/video-sources.json"
                )
            });
        let source_response = context.get(&source_url)?;
        let source_data: serde_json::Value = serde_json::from_slice(source_response.body())
            .map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid Daily Mail source JSON for {video_id}: {error}"),
                )
            })?;
        let source_data = source_data
            .get("body")
            .filter(|body| !body.is_null())
            .unwrap_or(&source_data);
        let renditions = source_data
            .get("renditions")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Daily Mail video {video_id} has no renditions"),
                )
            })?;
        let mut formats = Vec::new();
        for rendition in renditions {
            let Some(media_url) = json_string(rendition, "url")
                .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            else {
                continue;
            };
            let container = json_string(rendition, "videoContainer");
            let is_hls = container == Some("M2TS");
            let protocol = if is_hls {
                "m3u8_native".to_owned()
            } else {
                dailymail_protocol(media_url)
            };
            let tbr = json_i64(rendition, "encodingRate").map(|value| value / 1000);
            let mut format = serde_json::Map::new();
            format.insert("url".to_owned(), serde_json::json!(media_url));
            format.insert(
                "format_id".to_owned(),
                serde_json::json!(match tbr {
                    Some(tbr) => format!("{}-{tbr}", if is_hls { "hls" } else { &protocol }),
                    None => if is_hls { "hls".to_owned() } else { protocol.clone() },
                }),
            );
            if let Some(width) = json_i64(rendition, "frameWidth") {
                format.insert("width".to_owned(), serde_json::json!(width));
            }
            if let Some(height) = json_i64(rendition, "frameHeight") {
                format.insert("height".to_owned(), serde_json::json!(height));
            }
            if let Some(tbr) = tbr {
                format.insert("tbr".to_owned(), serde_json::json!(tbr));
            }
            if let Some(vcodec) = json_string(rendition, "videoCodec") {
                format.insert("vcodec".to_owned(), serde_json::json!(vcodec));
            }
            if let Some(container) = container {
                format.insert("container".to_owned(), serde_json::json!(container));
            }
            format.insert("protocol".to_owned(), serde_json::json!(protocol));
            if is_hls {
                format.insert("ext".to_owned(), serde_json::json!("mp4"));
            }
            formats.push(serde_json::Value::Object(format));
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Daily Mail video {video_id} has no playable renditions"),
            ));
        }

        let first_url = formats
            .first()
            .and_then(|format| format.get("url"))
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let mut output = InfoDict::new();
        output.insert("id", serde_json::json!(video_id));
        output.insert("title", serde_json::json!(title));
        output.insert_if_some(
            "description",
            json_string(&video_data, "descr")
                .map(unescape_html_attribute)
                .filter(|value| !value.is_empty()),
        );
        output.insert_if_some(
            "thumbnail",
            json_string(&video_data, "poster")
                .or_else(|| json_string(&video_data, "thumbnail"))
                .map(unescape_html_attribute),
        );
        output.insert("url", first_url);
        output.insert("formats", serde_json::Value::Array(formats));
        output.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(output))
    }
}

fn dailymail_protocol(media_url: &str) -> String {
    if media_url.starts_with("rtmp") {
        return "rtmp".to_owned();
    }
    if yt_dlp_core::determine_ext(Some(media_url), "unknown").eq_ignore_ascii_case("m3u8") {
        return "m3u8_native".to_owned();
    }
    url::Url::parse(media_url)
        .ok()
        .map(|url| url.scheme().to_owned())
        .or_else(|| media_url.split_once("://").map(|(scheme, _)| scheme.to_owned()))
        .unwrap_or_else(|| "http".to_owned())
}
