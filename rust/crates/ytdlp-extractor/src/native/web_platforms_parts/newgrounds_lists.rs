/// Native Newgrounds collection/search extractor. Entries are materialized
/// through NewgroundsExtractor so playlist selection can operate entirely on
/// Rust InfoDict values.
pub struct NewgroundsPlaylistExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl NewgroundsPlaylistExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for NewgroundsPlaylistExtractor {
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
                "Newgrounds collection URL did not match its native pattern",
            )
        })?;
        let playlist_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Newgrounds collection URL has no ID",
                )
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let links = newgrounds_media_links(&html);
        let entries = extract_newgrounds_entries(context, &links)?;
        if entries.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Newgrounds collection {playlist_id} has no media entries"),
            ));
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(playlist_id));
        info.insert_if_some("title", html_title_value(&html));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

/// Native Newgrounds user listing extractor. The JSON page endpoint is
/// paginated, so pages are fetched until the service returns an empty page.
pub struct NewgroundsUserExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl NewgroundsUserExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for NewgroundsUserExtractor {
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
                "Newgrounds user URL did not match its native pattern",
            )
        })?;
        let user_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Newgrounds user URL has no ID",
                )
            })?;
        let mut links = Vec::new();
        for page in 1..=1_000 {
            let page_url = if url.contains('?') {
                format!("{url}&page={page}")
            } else {
                format!("{url}?page={page}")
            };
            let response = native_get_json_with_headers(
                context,
                &page_url,
                &[
                    ("Accept", "application/json, text/javascript, */*; q=0.01"),
                    ("X-Requested-With", "XMLHttpRequest"),
                ],
            )?;
            let Some(items) = response.get("items").and_then(serde_json::Value::as_array) else {
                break;
            };
            if items.is_empty() {
                break;
            }
            for item in items {
                for fragment in json_text_values(item) {
                    for link in newgrounds_media_links(fragment) {
                        if !links.contains(&link) {
                            links.push(link);
                        }
                    }
                }
            }
        }
        let entries = extract_newgrounds_entries(context, &links)?;
        if entries.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Newgrounds user {user_id} has no media entries"),
            ));
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(user_id));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

fn newgrounds_media_extractor() -> Result<NewgroundsExtractor, ExtractorError> {
    NewgroundsExtractor::new(ExtractorDescriptor::new(
        "NewgroundsIE",
        "Newgrounds",
        r"https?://(?:www\.)?newgrounds\.com/(?:audio/listen|portal/view)/(?P<id>\d+)(?:/format/flash)?",
        true,
    ))
}

fn newgrounds_media_links(value: &str) -> Vec<String> {
    let Ok(matcher) = Regex::new(
        r#"(?is)href\s*=\s*["'](?:https?://(?:www\.)?newgrounds\.com)?/?((?:portal/view|audio/listen)/(\d+))"#,
    ) else {
        return Vec::new();
    };
    let mut links = Vec::new();
    for captures in matcher.captures_iter(value).flatten() {
        let Some(path) = captures.get(1).map(|value| value.as_str()) else {
            continue;
        };
        let link = format!("https://www.newgrounds.com/{path}");
        if !links.contains(&link) {
            links.push(link);
        }
    }
    links
}

fn json_text_values(value: &serde_json::Value) -> Vec<&str> {
    match value {
        serde_json::Value::String(value) => vec![value.as_str()],
        serde_json::Value::Array(values) => values.iter().flat_map(json_text_values).collect(),
        serde_json::Value::Object(values) => values.values().flat_map(json_text_values).collect(),
        _ => Vec::new(),
    }
}

fn extract_newgrounds_entries(
    context: &ExtractionContext,
    links: &[String],
) -> Result<Vec<InfoDict>, ExtractorError> {
    let extractor = newgrounds_media_extractor()?;
    links
        .iter()
        .map(
            |link| match extractor.extract_with_context(link, context)? {
                ExtractorResult::Single(info) => Ok(info),
                ExtractorResult::Redirect { .. } => Err(ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Newgrounds media entry unexpectedly returned a redirect",
                )),
                ExtractorResult::Playlist { .. } => Err(ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Newgrounds media entry unexpectedly returned a playlist",
                )),
            },
        )
        .collect()
}
