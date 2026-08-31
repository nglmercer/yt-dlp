/// Native Rumble page wrapper. Canonical pages embed the same u3 player
/// endpoint used by RumbleEmbedExtractor; page-level counters and description
/// are merged after extracting that native media record.
pub struct RumbleExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl RumbleExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for RumbleExtractor {
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
        let page_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Rumble page has no ID")
            })?;
        let webpage_response = context.get(url)?;
        let webpage = String::from_utf8_lossy(webpage_response.body());
        let embed_id = Regex::new(
            r#"(?is)(?:rumble\.com/embed/|["']embedUrl["']\s*:\s*["'](?:https?:)?//rumble\.com/embed/|<iframe[^>]+\bsrc=["'](?:https?:)?//rumble\.com/embed/|Rumble\(\s*["']play["']\s*,\s*\{[^}]*["']?video["']?\s*:\s*["'])([0-9a-z]+)"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!("TODO: Rumble page {page_id} has no native embed URL"),
            )
        })?;
        let embed_extractor = RumbleEmbedExtractor::new(ExtractorDescriptor::new(
            "RumbleEmbedIE",
            "RumbleEmbed",
            r"https?://(?:www\.)?rumble\.com/embed/(?:[0-9a-z]+\.)?(?P<id>[0-9a-z]+)",
            true,
        ))?;
        let mut info = match embed_extractor
            .extract_with_context(&format!("https://rumble.com/embed/{embed_id}"), context)?
        {
            ExtractorResult::Single(info) => info,
            ExtractorResult::Redirect { .. } | ExtractorResult::Playlist { .. } => {
                return Err(ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Rumble embed unexpectedly returned a non-media result",
                ));
            }
        };
        info.insert_if_some(
            "release_timestamp",
            Regex::new(
                r#"(?is)(?:Livestream begins|Streamed on):\s*<time[^>]*datetime=["']([^"']+)"#,
            )
            .ok()
            .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| value.as_str().to_owned())
            .and_then(parse_timestamp),
        );
        info.insert_if_some(
            "view_count",
            Regex::new(r#"(?is)"userInteractionCount"\s*:\s*(\d+)"#)
                .ok()
                .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
                .and_then(|captures| captures.get(1))
                .and_then(|value| value.as_str().parse::<i64>().ok()),
        );
        info.insert_if_some(
            "like_count",
            Regex::new(r#"(?is)<span[^>]*data-js=["']rumbles_up_votes["'][^>]*>\s*([^<]+)"#)
                .ok()
                .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
                .and_then(|captures| captures.get(1))
                .and_then(|value| parse_compact_count(value.as_str())),
        );
        info.insert_if_some(
            "dislike_count",
            Regex::new(r#"(?is)<span[^>]*data-js=["']rumbles_down_votes["'][^>]*>\s*([^<]+)"#)
                .ok()
                .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
                .and_then(|captures| captures.get(1))
                .and_then(|value| parse_compact_count(value.as_str())),
        );
        info.insert_if_some(
            "description",
            html_element_by_class(&webpage, "media-description")
                .map(|value| html_text_fragment(&value))
                .filter(|value| !value.is_empty()),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native Rumble channel/user listing extractor. Rumble exposes the same
/// video-card markup on both channel and user pages; pagination ends with an
/// empty page or a native HTTP 404, matching the source extractor's behavior.
pub struct RumbleChannelExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl RumbleChannelExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for RumbleChannelExtractor {
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
                "Rumble channel URL did not match its native pattern",
            )
        })?;
        let base_url = captures
            .name("url")
            .map(|value| value.as_str().to_owned())
            .unwrap_or_else(|| url.to_owned());
        let playlist_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Rumble channel has no ID")
            })?;
        let video_extractor = RumbleExtractor::new(ExtractorDescriptor::new(
            "RumbleIE",
            "Rumble",
            r"https?://(?:www\.)?rumble\.com/(?P<id>v[\w.-]+)[^/]*$",
            true,
        ))?;
        let mut entries = Vec::new();
        let mut seen_links = Vec::new();
        for page in 1..=10_000usize {
            let page_url = format!("{base_url}?page={page}");
            let response = match context.get(&page_url) {
                Ok(response) => response,
                Err(error) if error.message.contains("HTTP 404") => break,
                Err(error) => return Err(error),
            };
            let html = String::from_utf8_lossy(response.body());
            let links = rumble_channel_video_links(&html);
            if links.is_empty() {
                break;
            }
            for link in links {
                if seen_links.contains(&link) {
                    continue;
                }
                seen_links.push(link.clone());
                let entry = video_extractor
                    .extract_with_context(&link, context)
                    .map_err(|error| {
                        ExtractorError::new(
                            ExtractorErrorKind::Extraction,
                            format!("Rumble channel entry {link}: {error}"),
                        )
                    })?;
                match entry {
                    ExtractorResult::Single(info) => entries.push(info),
                    ExtractorResult::Redirect { .. } | ExtractorResult::Playlist { .. } => {
                        return Err(ExtractorError::new(
                            ExtractorErrorKind::Extraction,
                            format!("Rumble channel entry {link} returned a non-media result"),
                        ));
                    }
                }
            }
        }
        Ok(ExtractorResult::Playlist {
            info: {
                let mut info = InfoDict::new();
                info.insert("id", serde_json::json!(playlist_id));
                info
            },
            entries,
        })
    }
}

fn rumble_channel_video_links(html: &str) -> Vec<String> {
    let Ok(anchor_matcher) = Regex::new(r"(?is)<a\b([^>]+)>") else {
        return Vec::new();
    };
    let Ok(href_matcher) = Regex::new(r#"(?is)\bhref\s*=\s*[\"']([^\"']+)"#) else {
        return Vec::new();
    };
    let mut links = Vec::new();
    for captures in anchor_matcher.captures_iter(html).flatten() {
        let Some(attributes) = captures.get(1).map(|value| value.as_str()) else {
            continue;
        };
        let class_attributes = attributes.to_ascii_lowercase();
        if !class_attributes.contains("videostream__link")
            && !class_attributes.contains("video-item--a")
        {
            continue;
        }
        let Some(href) = href_matcher
            .captures(attributes)
            .ok()
            .flatten()
            .and_then(|value| {
                value
                    .get(1)
                    .map(|value| unescape_html_attribute(value.as_str()))
            })
        else {
            continue;
        };
        let Some(link) = url::Url::parse("https://rumble.com/")
            .ok()
            .and_then(|base| base.join(&href).ok())
            .map(|value| value.to_string())
        else {
            continue;
        };
        if !links.contains(&link) {
            links.push(link);
        }
    }
    links
}
