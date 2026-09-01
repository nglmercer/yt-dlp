/// Native Epicon player and TV-show playlist extractors.
pub struct EpiconExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl EpiconExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for EpiconExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Epicon URL has no video ID")
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let content_id = epicon_capture(
            &webpage,
            r#"(?is)class\s*=\s*["']mylist-icon\s+iconclick["'][^>]*\bid\s*=\s*["'](\d+)"#,
        )
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Epicon video {video_id} has no player content ID"),
            )
        })?;
        let mut request = Request::new("https://www.epicon.in/ajaxplayer/");
        request.set_method("POST").map_err(map_request_error)?;
        request
            .headers_mut()
            .set("Content-Type", "application/x-www-form-urlencoded; charset=UTF-8");
        let mut form = url::form_urlencoded::Serializer::new(String::new());
        form.append_pair("cid", &content_id);
        form.append_pair("action", "st");
        form.append_pair("type", "video");
        request.set_data(Some(form.finish().into_bytes()));
        let response = context.request(&request)?;
        let data = serde_json::from_slice::<serde_json::Value>(response.body()).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Epicon player JSON for {video_id}: {error}"),
            )
        })?;
        if json_bool(&data, "success") != Some(true) {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                json_string(&data, "message")
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("Epicon player rejected video {video_id}")),
            ));
        }
        let media_url = data
            .get("url")
            .and_then(|value| json_string(value, "video_url"))
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Epicon video {video_id} has no HLS URL"),
                )
            })?;
        let format = serde_json::json!({
            "url": media_url,
            "format_id": "hls",
            "protocol": "m3u8_native",
            "ext": "mp4",
        });
        let title = epicon_capture(&webpage, r#"(?is)setplaytitle\s*=\s*"([^"]+)"#)
            .map(|value| unescape_html_attribute(&value))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Epicon video {video_id} has no player title"),
                )
            })?;
        let mut subtitles = serde_json::Map::new();
        if let Some(subtitle_entries) = data.get("subtitles").and_then(serde_json::Value::as_array)
        {
            for subtitle in subtitle_entries {
                let Some(sub_url) = json_string(subtitle, "file")
                    .map(|value| proto_relative_url(value, "https:"))
                    .filter(|value| {
                        value.starts_with("http://") || value.starts_with("https://")
                    })
                else {
                    continue;
                };
                let language = json_string(subtitle, "lang")
                    .filter(|value| !value.is_empty())
                    .unwrap_or("English");
                subtitles
                    .entry(language.to_owned())
                    .or_insert_with(|| serde_json::json!([]))
                    .as_array_mut()
                    .expect("Epicon subtitle entry is always initialized as an array")
                    .push(serde_json::json!({"url": sub_url}));
            }
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", html_meta_value(&webpage, "og:description"));
        info.insert_if_some("thumbnail", html_meta_value(&webpage, "og:image"));
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", serde_json::json!([format]));
        info.insert("subtitles", serde_json::Value::Object(subtitles));
        Ok(ExtractorResult::single(info))
    }
}

/// Native Epicon TV-show playlist extractor.
pub struct EpiconSeriesExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl EpiconSeriesExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for EpiconSeriesExtractor {
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
        let playlist_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Epicon series has no ID")
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let pattern = format!(
            r#"(?is)ct-tray-url\s*=\s*["'](tv-shows/{}/[^"']+)"#,
            regex::escape(&playlist_id)
        );
        let matcher = Regex::new(&pattern).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Epicon series entry matcher: {error}"),
            )
        })?;
        let mut entries = Vec::new();
        for captures in matcher.captures_iter(&webpage).flatten() {
            let Some(path) = captures.get(1).map(|value| value.as_str()) else {
                continue;
            };
            let target_url = format!("https://www.epicon.in/{}", unescape_html_attribute(path));
            if entries.iter().any(|entry: &InfoDict| {
                entry.get_str("url") == Some(target_url.as_str())
            }) {
                continue;
            }
            let mut entry = native_url_result(&target_url);
            entry.insert("ie_key", serde_json::json!("Epicon"));
            entries.push(entry);
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(playlist_id));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

fn epicon_capture(html: &str, pattern: &str) -> Option<String> {
    Regex::new(pattern)
        .ok()
        .and_then(|matcher| matcher.captures(html).ok().flatten())
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
}
