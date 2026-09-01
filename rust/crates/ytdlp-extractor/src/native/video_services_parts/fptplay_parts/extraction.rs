/// Native FPT Play signed VOD extractor.
pub struct FptplayExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FptplayExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FptplayExtractor {
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
                "FPT Play URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "FPT Play URL has no video ID")
            })?;
        let slug_episode = captures
            .name("episode")
            .map(|value| value.as_str().to_owned());

        // The source extractor treats the page as non-fatal because the signed
        // API is the authoritative playback source. Preserve that behavior in
        // native Rust while still using any page metadata that is available.
        let webpage = context
            .get(url)
            .map(|response| String::from_utf8_lossy(response.body()).into_owned())
            .unwrap_or_default();
        let heading_title = fptplay_heading_title(&webpage);
        let title = heading_title
            .clone()
            .or_else(|| html_meta_value(&webpage, "og:title"))
            .or_else(|| html_meta_value(&webpage, "twitter:title"));
        let real_episode = if heading_title.is_none() {
            slug_episode
        } else {
            fptplay_active_episode(&webpage)
        };
        let title = fptplay_join_title(title, real_episode);
        let description = fptplay_description(&webpage);
        let episode = captures
            .name("episode")
            .and_then(|value| value.as_str().parse::<i64>().ok())
            .map_or(0, |value| value - 1);
        let api_url = fptplay_api_url(&video_id, episode);
        let response = context.get_json(&api_url)?;
        let stream_url = response
            .get("data")
            .and_then(|data| json_string(data, "url"))
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            .map(str::to_owned)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("FPT Play video {video_id} has no playable stream URL"),
                )
            })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", title);
        info.insert_if_some("description", description);
        info.insert("url", serde_json::json!(stream_url.clone()));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": stream_url,
                "format_id": "hls-0",
                "protocol": "m3u8_native",
                "ext": "mp4",
            }]),
        );
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}

fn fptplay_heading_title(html: &str) -> Option<String> {
    let matcher = Regex::new(r#"(?is)<h4\b[^>]*\bclass\s*=\s*["']([^"']*)["'][^>]*>(.*?)</h4\s*>"#).ok()?;
    matcher.captures_iter(html).flatten().find_map(|captures| {
        let classes = captures.get(1)?.as_str().split_whitespace().collect::<Vec<_>>();
        if !["mb-1", "text-2xl", "text-white"]
            .iter()
            .all(|class| classes.contains(class))
        {
            return None;
        }
        let value = fptplay_clean_html(captures.get(2)?.as_str());
        (!value.is_empty()).then_some(value)
    })
}

fn fptplay_active_episode(html: &str) -> Option<String> {
    let matcher = Regex::new(r#"(?is)<p\b([^>]*)>"#).ok()?;
    matcher.captures_iter(html).flatten().find_map(|captures| {
        let attributes = captures.get(1)?.as_str();
        let classes = fptplay_attribute(attributes, "class")?;
        if !classes.split_whitespace().any(|class| class == "epi-title")
            || !classes.split_whitespace().any(|class| class == "active")
        {
            return None;
        }
        fptplay_attribute(attributes, "title").filter(|value| !value.is_empty())
    })
}

fn fptplay_description(html: &str) -> Option<String> {
    let matcher = Regex::new(r#"(?is)<p\b([^>]*)>(.*?)</p\s*>"#).ok()?;
    matcher.captures_iter(html).flatten().find_map(|captures| {
        let attributes = captures.get(1)?.as_str();
        let classes = fptplay_attribute(attributes, "class")?;
        if !classes
            .split_whitespace()
            .any(|class| class == "overflow-hidden")
        {
            return None;
        }
        let value = fptplay_clean_html(captures.get(2)?.as_str());
        (!value.is_empty()).then_some(value)
    }).or_else(|| {
        html_meta_value(html, "og:description")
            .or_else(|| html_meta_value(html, "twitter:description"))
            .map(|value| fptplay_clean_html(&value))
            .filter(|value| !value.is_empty())
    })
}

fn fptplay_attribute(attributes: &str, name: &str) -> Option<String> {
    let pattern = format!(r#"(?is)\b{}\s*=\s*["']([^"']*)"#, regex::escape(name));
    Regex::new(&pattern)
        .ok()
        .and_then(|matcher| matcher.captures(attributes).ok().flatten())
        .and_then(|captures| captures.get(1).map(|value| unescape_html_attribute(value.as_str())))
}

fn fptplay_clean_html(value: &str) -> String {
    let value = unescape_html_attribute(&html_text_fragment(value));
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn fptplay_join_title(title: Option<String>, episode: Option<String>) -> Option<String> {
    match (
        title.filter(|value| !value.trim().is_empty()),
        episode.filter(|value| !value.is_empty()),
    ) {
        (Some(title), Some(episode)) => Some(format!("{title} - {episode}")),
        (Some(title), None) => Some(title),
        (None, Some(episode)) => Some(episode),
        (None, None) => None,
    }
}
