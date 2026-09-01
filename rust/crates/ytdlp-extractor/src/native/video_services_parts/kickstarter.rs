/// Native Kickstarter project-page extractor.
pub struct KickstarterExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl KickstarterExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for KickstarterExtractor {
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
                    "Kickstarter URL has no project ID",
                )
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let title = kickstarter_title(&webpage).unwrap_or_else(|| video_id.clone());
        let description = html_meta_value(&webpage, "og:description")
            .map(|value| unescape_html_attribute(&value).trim().to_owned())
            .filter(|value| !value.is_empty());
        let thumbnail = html_meta_value(&webpage, "og:image")
            .and_then(|value| {
                kickstarter_valid_url(&value).or_else(|| Some(resolve_url(url, &value)))
            });
        let video_url = kickstarter_attribute(&webpage, "data-video-url")
            .and_then(|value| (!value.trim().is_empty()).then(|| resolve_url(url, &value)));

        let Some(video_url) = video_url else {
            let generic = GenericExtractor::new(ExtractorDescriptor::new(
                "GenericIE",
                "Generic",
                "",
                true,
            ));
            let fallback = generic.extract_with_context(url, context)?;
            if let ExtractorResult::Single(mut info) = fallback {
                if info.get_bool("direct") == Some(true) || info.contains_key("formats") {
                    info.insert("id", serde_json::json!(video_id));
                    info.insert("title", serde_json::json!(title));
                    info.insert_if_some("description", description);
                    info.insert_if_some("thumbnail", thumbnail);
                    return Ok(ExtractorResult::single(info));
                }
            }
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!(
                    "TODO: Kickstarter project {video_id} uses an embedded provider without a native extractor"
                ),
            ));
        };
        let extension = yt_dlp_core::determine_ext(Some(&video_url), "mp4").to_ascii_lowercase();
        let extension = if extension == "unknown" {
            "mp4".to_owned()
        } else {
            extension
        };
        let protocol = if extension == "m3u8" {
            "m3u8_native"
        } else if extension == "mpd" {
            "http_dash_segments"
        } else {
            "http"
        };
        let format_extension = matches!(extension.as_str(), "m3u8" | "mpd")
            .then_some("mp4")
            .unwrap_or(extension.as_str());
        let format_id = if matches!(extension.as_str(), "m3u8" | "mpd") {
            "hls"
        } else {
            extension.as_str()
        };
        let format = serde_json::json!({
            "url": video_url,
            "format_id": format_id,
            "ext": format_extension,
            "protocol": protocol,
        });
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", description);
        info.insert_if_some("thumbnail", thumbnail);
        info.insert(
            "url",
            format
                .get("url")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
        info.insert(
            "ext",
            format
                .get("ext")
                .cloned()
                .unwrap_or_else(|| serde_json::json!("mp4")),
        );
        info.insert("formats", serde_json::json!([format]));
        info.insert("subtitles", serde_json::json!({}));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

fn kickstarter_attribute(html: &str, name: &str) -> Option<String> {
    let pattern = format!(
        r#"(?is)\b{}\s*=\s*["']([^"']*)"#,
        regex::escape(name)
    );
    Regex::new(&pattern)
        .ok()
        .and_then(|matcher| matcher.captures(html).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| unescape_html_attribute(value.as_str()))
}

fn kickstarter_title(html: &str) -> Option<String> {
    let matcher = Regex::new(r#"(?is)<title\b[^>]*>(.*?)</title\s*>"#).ok()?;
    let raw_title = matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| html_text_fragment(value.as_str()))?;
    let title = raw_title
        .trim()
        .strip_suffix("&mdash; Kickstarter")
        .or_else(|| raw_title.trim().strip_suffix("— Kickstarter"))
        .unwrap_or(raw_title.trim())
        .trim();
    (!title.is_empty()).then(|| title.to_owned())
}

fn kickstarter_valid_url(value: &str) -> Option<String> {
    let value = value.trim();
    (value.starts_with("http://") || value.starts_with("https://"))
        .then(|| value.to_owned())
}
