/// Native Hollywood Reporter video wrapper and category playlist extractors.
pub struct HollywoodReporterExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl HollywoodReporterExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for HollywoodReporterExtractor {
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
        let display_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Hollywood Reporter URL has no video ID",
                )
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let card = hollywood_reporter_opening_tag_by_class(&webpage, "vlanding-video-card__link")
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Hollywood Reporter video {display_id} has no video card"),
                )
            })?;
        let video_id = hollywood_reporter_attribute(&card, "data-video-showcase-trigger")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!(
                        "Hollywood Reporter video {display_id} has no showcase media ID"
                    ),
                )
            })?;
        let showcase_type = hollywood_reporter_attribute(&card, "data-video-showcase-type")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!(
                        "Hollywood Reporter video {display_id} has no showcase media type"
                    ),
                )
            })?;

        match showcase_type.to_ascii_lowercase().as_str() {
            "jwplayer" => Ok(ExtractorResult::Redirect {
                url: format!("jwplatform:{video_id}"),
                ie_key: Some("JWPlatform".to_owned()),
            }),
            "youtube" => Ok(ExtractorResult::Redirect {
                url: hollywood_reporter_youtube_url(&video_id),
                ie_key: Some("Youtube".to_owned()),
            }),
            other => Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!(
                    "TODO: Hollywood Reporter showcase type \"{other}\" is not implemented in Rust"
                ),
            )),
        }
    }
}

/// Native Hollywood Reporter category playlist extractor.
pub struct HollywoodReporterPlaylistExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl HollywoodReporterPlaylistExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for HollywoodReporterPlaylistExtractor {
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
        let captures = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Hollywood Reporter category URL is invalid",
                )
            })?;
        let slug = captures
            .name("slug")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Hollywood Reporter category URL has no slug",
                )
            })?;
        let playlist_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Hollywood Reporter category URL has no ID",
                )
            })?;

        const MAX_PAGES: usize = 10_000;
        let mut entries = Vec::new();
        for page in 1..=MAX_PAGES {
            let page_url = format!(
                "https://www.hollywoodreporter.com/vcategory/{slug}-{playlist_id}/page/{page}/"
            );
            let response = context.get(&page_url)?;
            let webpage = String::from_utf8_lossy(response.body());
            let section = html_element_by_class(&webpage, "video-playlist-river").unwrap_or_default();
            let page_entries = hollywood_reporter_playlist_entries(&page_url, &section);
            if page_entries.is_empty() {
                break;
            }
            entries.extend(page_entries);
        }

        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(playlist_id));
        info.insert("title", serde_json::json!(slug));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

fn hollywood_reporter_playlist_entries(page_url: &str, section: &str) -> Vec<InfoDict> {
    let Ok(anchor_matcher) = Regex::new(r#"(?is)<a\b([^>]*)>"#) else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    for captures in anchor_matcher.captures_iter(section).flatten() {
        let Some(attributes) = captures.get(1).map(|value| value.as_str()) else {
            continue;
        };
        let Some(classes) = hollywood_reporter_attribute(attributes, "class") else {
            continue;
        };
        if !classes
            .split_ascii_whitespace()
            .any(|class| class == "c-title__link")
        {
            continue;
        }
        let Some(href) = hollywood_reporter_attribute(attributes, "href")
            .filter(|href| !href.is_empty())
        else {
            continue;
        };
        let target = resolve_url(page_url, &href);
        let mut entry = native_url_result(&target);
        entry.insert("ie_key", serde_json::json!("HollywoodReporter"));
        entries.push(entry);
    }
    entries
}

fn hollywood_reporter_opening_tag_by_class(html: &str, class: &str) -> Option<String> {
    let matcher = Regex::new(r#"(?is)<[a-z0-9]+\b[^>]*>"#).ok()?;
    matcher.find_iter(html).find_map(|match_| {
        let match_ = match_.ok()?;
        let tag = match_.as_str();
        let classes = hollywood_reporter_attribute(tag, "class")?;
        classes
            .split_ascii_whitespace()
            .any(|value| value == class)
            .then(|| tag.to_owned())
    })
}

fn hollywood_reporter_attribute(html: &str, name: &str) -> Option<String> {
    let name = regex::escape(name);
    let patterns = [
        format!(r#"(?is)(?:^|\s){name}\s*=\s*"([^"]*)""#),
        format!(r#"(?is)(?:^|\s){name}\s*=\s*'([^']*)'"#),
    ];
    patterns.iter().find_map(|pattern| {
        Regex::new(pattern)
            .ok()
            .and_then(|matcher| matcher.captures(html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| unescape_html_attribute(value.as_str()))
    })
}

fn hollywood_reporter_youtube_url(video_id: &str) -> String {
    if video_id.starts_with("http://") || video_id.starts_with("https://") {
        video_id.to_owned()
    } else {
        format!("https://www.youtube.com/watch?v={video_id}")
    }
}
