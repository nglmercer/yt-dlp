/// Native AltCensored page extractor. The archive.org item is resolved by the
/// native transparent URL-result path in the CLI, so no Python compatibility
/// layer is needed for the upstream media.
pub struct AltCensoredExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl AltCensoredExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for AltCensoredExtractor {
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
                    "AltCensored URL has no video ID",
                )
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let mut info = InfoDict::new();
        info.insert("_type", serde_json::json!("url_transparent"));
        info.insert(
            "url",
            serde_json::json!(format!("https://archive.org/details/youtube-{video_id}")),
        );
        info.insert("ie_key", serde_json::json!("ArchiveOrg"));
        info.insert_if_some("view_count", altcensored_view_count(&webpage));
        info.insert_if_some(
            "categories",
            altcensored_category(&webpage).map(|category| vec![category]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native AltCensored channel playlist extractor. Native playlist results are
/// materialized eagerly because the Rust result contract is an owned vector;
/// each entry remains a URL result for the native AltCensored extractor.
pub struct AltCensoredChannelExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl AltCensoredChannelExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for AltCensoredChannelExtractor {
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
        let channel_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "AltCensored channel URL has no channel ID",
                )
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let page_count = altcensored_page_count(&webpage);
        let title = html_meta_value(&webpage, "altcen_title")
            .map(|value| html_text_fragment(&value))
            .filter(|value| !value.is_empty());
        let mut entries = Vec::new();
        for page_number in 1..=page_count {
            let page_url = format!(
                "https://altcensored.com/channel/{channel_id}/page/{page_number}"
            );
            let response = context.get(&page_url)?;
            let page = String::from_utf8_lossy(response.body());
            for entry_url in altcensored_video_urls(&page) {
                if entries
                    .iter()
                    .any(|entry: &InfoDict| entry.get_str("url") == Some(entry_url.as_str()))
                {
                    continue;
                }
                let mut entry = native_url_result(&entry_url);
                entry.insert("ie_key", serde_json::json!("AltCensored"));
                entries.push(entry);
            }
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(channel_id));
        info.insert_if_some("title", title);
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

fn altcensored_category(html: &str) -> Option<String> {
    let matcher = Regex::new(
        r##"(?is)<a\b[^>]*\bhref\s*=\s*["']/category/\d+["'][^>]*>(.*?)</a>"##,
    )
    .ok()?;
    matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| html_text_fragment(value.as_str()))
        .filter(|value| !value.is_empty())
}

fn altcensored_view_count(html: &str) -> Option<i64> {
    let matcher = Regex::new(r##"(?is)YouTube\s+Views:\s*(?:&nbsp;|\s)*([\d,]+)"##).ok()?;
    matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .and_then(|value| value.as_str().replace(',', "").parse().ok())
}

fn altcensored_page_count(html: &str) -> usize {
    let Ok(matcher) = Regex::new(
        r##"(?is)<a\b[^>]*\bhref\s*=\s*["']/channel/[\w-]+/page/(\d+)["'][^>]*>\s*(\d+)\s*</a>"##,
    ) else {
        return 1;
    };
    matcher
        .captures_iter(html)
        .flatten()
        .filter_map(|captures| {
            let path_page = captures.get(1)?.as_str();
            let label_page = captures.get(2)?.as_str();
            (path_page == label_page).then(|| path_page.parse::<usize>().ok())?
        })
        .max()
        .unwrap_or(1)
}

fn altcensored_video_urls(html: &str) -> Vec<String> {
    let Ok(matcher) = Regex::new(
        r##"(?is)<a\b[^>]*\bhref\s*=\s*["'](/watch\?v=[^"']+)["']"##,
    ) else {
        return Vec::new();
    };
    let mut urls = Vec::new();
    for captures in matcher.captures_iter(html).flatten() {
        let Some(path) = captures.get(1).map(|value| value.as_str()) else {
            continue;
        };
        let path = unescape_html_attribute(path);
        let absolute = resolve_url("https://www.altcensored.com", &path);
        if !urls.contains(&absolute) {
            urls.push(absolute);
        }
    }
    urls
}
