/// Native Kenh14 video page and media-metadata extractor.
pub struct Kenh14VideoExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl Kenh14VideoExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for Kenh14VideoExtractor {
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
        let video_id = kenh14_match_id(&self.matcher, url, "video")?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let stream_tag = kenh14_stream_tag(&webpage).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Kenh14 video {video_id} has no VideoStream element"),
            )
        })?;
        let raw_media_url = kenh14_attribute(&stream_tag, "data-vid").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Kenh14 video {video_id} has no direct media URL"),
            )
        })?;
        let media_url = kenh14_https_url(&raw_media_url);
        let mut formats = vec![kenh14_media_format(&media_url, "http")];
        let metadata_url = format!(
            "https://api.kinghub.vn/video/api/v1/detailVideoByGet?FileName={}",
            raw_media_url
                .strip_prefix("kenh14cdn.com/")
                .unwrap_or(raw_media_url.trim_start_matches("https://"))
        );
        let metadata = context.get_json(&metadata_url).ok();
        let media_data = context
            .get_json(&format!("https://{raw_media_url}.json"))
            .ok();
        if let Some(hls_url) = media_data
            .as_ref()
            .and_then(|data| json_string(data, "hls"))
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
        {
            formats.push(kenh14_media_format(hls_url, "hls"));
        }
        if let Some(dash_url) = media_data
            .as_ref()
            .and_then(|data| json_string(data, "mpd"))
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
        {
            formats.push(kenh14_media_format(dash_url, "dash"));
        }
        let metadata = metadata.unwrap_or(serde_json::Value::Null);
        let title = json_string(&metadata, "title")
            .map(str::to_owned)
            .or_else(|| html_meta_value(&webpage, "og:title"))
            .or_else(|| {
                html_element_by_class(&webpage, "vdbw-title")
                    .map(|value| html_text_fragment(&value))
            })
            .unwrap_or_else(|| video_id.clone());
        let description = json_string(&metadata, "description")
            .map(str::to_owned)
            .or_else(|| html_meta_value(&webpage, "og:description"))
            .or_else(|| {
                html_element_by_class(&webpage, "vdbw-sapo")
                    .map(|value| html_text_fragment(&value))
            });
        let timestamp = json_string(&metadata, "uploadtime").and_then(kenh14_timestamp);
        let thumbnail =
            html_meta_value(&webpage, "og:image").or_else(|| kenh14_attribute(&stream_tag, "data-thumb"));
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", description);
        info.insert_if_some("thumbnail", thumbnail);
        info.insert_if_some("duration", json_f64(&metadata, "duration"));
        info.insert_if_some(
            "uploader",
            json_string(&metadata, "author")
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        );
        info.insert_if_some("timestamp", timestamp);
        info.insert_if_some(
            "upload_date",
            json_string(&metadata, "uploadtime").and_then(|value| date_digits(value)),
        );
        info.insert_if_some("view_count", json_i64(&metadata, "views"));
        info.insert_if_some("tags", kenh14_tags(&webpage));
        info.insert("url", first.get("url").cloned().unwrap_or_default());
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

fn kenh14_match_id(
    matcher: &Regex,
    url: &str,
    kind: &str,
) -> Result<String, ExtractorError> {
    matcher
        .captures(url)
        .ok()
        .flatten()
        .and_then(|captures| captures.name("id"))
        .map(|value| value.as_str().to_owned())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                format!("Kenh14 {kind} URL has no ID"),
            )
        })
}

fn kenh14_stream_tag(html: &str) -> Option<String> {
    let matcher =
        Regex::new(r#"(?is)<[^>]*\btype\s*=\s*["']VideoStream["'][^>]*>"#).ok()?;
    matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(0).map(|value| value.as_str().to_owned()))
}

fn kenh14_attribute(tag: &str, name: &str) -> Option<String> {
    let pattern = format!(
        r#"(?is)\b{}\s*=\s*["']([^"']+)"#,
        regex::escape(name)
    );
    Regex::new(&pattern)
        .ok()
        .and_then(|matcher| matcher.captures(tag).ok().flatten())
        .and_then(|captures| {
            captures
                .get(1)
                .map(|value| unescape_html_attribute(value.as_str()))
        })
}

fn kenh14_https_url(value: &str) -> String {
    if value.starts_with("http://") || value.starts_with("https://") {
        value.to_owned()
    } else {
        format!("https://{value}")
    }
}

fn kenh14_media_format(media_url: &str, format_id: &str) -> serde_json::Value {
    let extension = yt_dlp_core::determine_ext(Some(media_url), "mp4");
    let (extension, protocol) = match extension.as_str() {
        "m3u8" => ("mp4", "m3u8_native"),
        "mpd" => ("mp4", "http_dash_segments"),
        extension => (extension, "http"),
    };
    serde_json::json!({
        "url": media_url,
        "format_id": format_id,
        "ext": extension,
        "protocol": protocol,
    })
}

fn kenh14_timestamp(value: &str) -> Option<i64> {
    parse_timestamp(value.replace(' ', "T"))
        .or_else(|| parse_timestamp(format!("{}Z", value.replace(' ', "T"))))
}

fn kenh14_tags(html: &str) -> Option<Vec<String>> {
    let keywords = html_meta_value(html, "keywords")?;
    let tags = keywords
        .split(';')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    Some(tags)
}
