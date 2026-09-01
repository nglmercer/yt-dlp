/// Native HSE page-state extractors.
pub struct HseShowExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl HseShowExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for HseShowExtractor {
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
        let video_id = hse_match_id(&self.matcher, url, "HSE show")?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let redux = hse_redux_data(&webpage).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("HSE show {video_id} has no Redux page state"),
            )
        })?;
        let page = redux.get("tvShowPage").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("HSE show {video_id} has no show page state"),
            )
        })?;
        let show = page.get("tvShow").unwrap_or(&serde_json::Value::Null);
        let video = page.get("tvShowVideo").unwrap_or(&serde_json::Value::Null);
        let (formats, subtitles) =
            hse_formats_and_subtitles(video.get("sources"), &video_id)?;
        let title = json_string(show, "title")
            .filter(|value| !value.is_empty())
            .unwrap_or(&video_id);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", subtitles);
        info.insert_if_some(
            "timestamp",
            hse_show_timestamp(json_string(show, "date"), json_string(show, "hour")),
        );
        info.insert_if_some("thumbnail", json_string(video, "poster"));
        info.insert_if_some(
            "channel",
            json_string(show, "actionFieldText").and_then(|value| {
                hse_capture(value, r#"(?i)tvShow\s*\|\s*([A-Z0-9]+)_"#)
            }),
        );
        info.insert_if_some("uploader", json_string(show, "presenter"));
        info.insert("webpage_url", serde_json::json!(url));
        hse_insert_first_format(&mut info);
        Ok(ExtractorResult::single(info))
    }
}

/// Native HSE product-page video extractor.
pub struct HseProductExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl HseProductExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for HseProductExtractor {
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
        let video_id = hse_match_id(&self.matcher, url, "HSE product")?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let redux = hse_redux_data(&webpage).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("HSE product {video_id} has no Redux page state"),
            )
        })?;
        let video = redux
            .get("productContent")
            .and_then(|content| content.get("productContent"))
            .and_then(|content| content.get("videos"))
            .and_then(serde_json::Value::as_array)
            .and_then(|videos| videos.first())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("HSE product {video_id} has no product video"),
                )
            })?;
        let (formats, subtitles) =
            hse_formats_and_subtitles(video.get("sources"), &video_id)?;
        let title = redux
            .get("productDetail")
            .and_then(|detail| detail.get("product"))
            .and_then(|product| product.get("name"))
            .and_then(|name| json_string(name, "short"))
            .filter(|value| !value.is_empty())
            .unwrap_or(&video_id);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", subtitles);
        info.insert_if_some("thumbnail", json_string(video, "poster"));
        info.insert_if_some(
            "uploader",
            redux
                .get("productDetail")
                .and_then(|detail| detail.get("product"))
                .and_then(|product| product.get("brand"))
                .and_then(|brand| json_string(brand, "brandName")),
        );
        info.insert("webpage_url", serde_json::json!(url));
        hse_insert_first_format(&mut info);
        Ok(ExtractorResult::single(info))
    }
}

fn hse_formats_and_subtitles(
    sources: Option<&serde_json::Value>,
    video_id: &str,
) -> Result<(Vec<serde_json::Value>, serde_json::Value), ExtractorError> {
    let sources = sources.and_then(serde_json::Value::as_array).ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("HSE video {video_id} has no source list"),
        )
    })?;
    let mut formats = Vec::new();
    let mut subtitles = serde_json::Map::new();
    let mut unsupported_source = false;
    for (index, source) in sources.iter().enumerate() {
        let Some(media_url) = json_string(source, "url")
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
        else {
            continue;
        };
        let mimetype = json_string(source, "mimetype").unwrap_or_default();
        if !mimetype.eq_ignore_ascii_case("application/x-mpegURL")
            && yt_dlp_core::determine_ext(Some(media_url), "unknown") != "m3u8"
        {
            unsupported_source = true;
            continue;
        }
        formats.push(serde_json::json!({
            "url": media_url,
            "format_id": format!("hls-{index}"),
            "protocol": "m3u8_native",
            "ext": "mp4",
        }));
        hse_merge_subtitles(&mut subtitles, source);
    }
    if formats.is_empty() {
        let kind = if unsupported_source {
            ExtractorErrorKind::Unsupported
        } else {
            ExtractorErrorKind::Extraction
        };
        let message = if unsupported_source {
            format!("TODO: HSE video {video_id} has a non-HLS/DRM source that is not implemented in Rust")
        } else {
            format!("HSE video {video_id} has no playable source")
        };
        return Err(ExtractorError::new(kind, message));
    }
    Ok((formats, serde_json::Value::Object(subtitles)))
}

fn hse_merge_subtitles(subtitles: &mut serde_json::Map<String, serde_json::Value>, source: &serde_json::Value) {
    let Some(entries) = source
        .get("subtitles")
        .or_else(|| source.get("captions"))
        .and_then(serde_json::Value::as_object)
    else {
        return;
    };
    for (language, values) in entries {
        let Some(values) = values.as_array() else {
            continue;
        };
        let mut language_entries = Vec::new();
        for value in values {
            let url = value
                .as_str()
                .or_else(|| json_string(value, "url"))
                .filter(|value| value.starts_with("http://") || value.starts_with("https://"));
            if let Some(url) = url {
                language_entries.push(serde_json::json!({"url": url}));
            }
        }
        if !language_entries.is_empty() {
            subtitles.insert(language.clone(), serde_json::Value::Array(language_entries));
        }
    }
}

fn hse_insert_first_format(info: &mut InfoDict) {
    let (format_url, format_ext) = info
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .and_then(|formats| formats.first())
        .map(|format| (format.get("url").cloned(), format.get("ext").cloned()))
        .unwrap_or((None, None));
    info.insert_if_some("url", format_url);
    info.insert_if_some("ext", format_ext);
}

fn hse_redux_data(html: &str) -> Option<serde_json::Value> {
    json_object_after_marker(html, "window.__REDUX_DATA__")
}

fn hse_match_id(
    matcher: &Regex,
    url: &str,
    label: &str,
) -> Result<String, ExtractorError> {
    matcher
        .captures(url)
        .ok()
        .flatten()
        .and_then(|captures| captures.name("id"))
        .map(|value| value.as_str().to_owned())
        .ok_or_else(|| ExtractorError::new(ExtractorErrorKind::InvalidUrl, format!("{label} URL has no ID")))
}

fn hse_show_timestamp(date: Option<&str>, hour: Option<&str>) -> Option<i64> {
    let date = date?;
    let hour = hour.unwrap_or("00");
    parse_timestamp(format!("{date}T{hour}:00:00Z"))
}

fn hse_capture(value: &str, pattern: &str) -> Option<String> {
    Regex::new(pattern)
        .ok()
        .and_then(|matcher| matcher.captures(value).ok().flatten())
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
}
