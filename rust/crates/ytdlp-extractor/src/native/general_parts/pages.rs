/// Native BehindKink HTML5 video extractor.
pub struct BehindKinkExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl BehindKinkExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for BehindKinkExtractor {
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
                "BehindKink URL did not match its native pattern",
            )
        })?;
        let display_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "BehindKink URL has no ID")
            })?;
        let upload_date = ["year", "month", "day"]
            .iter()
            .map(|name| {
                captures
                    .name(name)
                    .map(|value| value.as_str())
                    .unwrap_or_default()
            })
            .collect::<String>();
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let video_url = Regex::new(r#"(?is)<source\b[^>]*\bsrc\s*=\s*["']([^"']+)"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| resolve_url(url, &unescape_html_attribute(value.as_str())))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("BehindKink page {display_id} has no video source"),
                )
            })?;
        let media_id = video_url
            .split('?')
            .next()
            .and_then(|value| value.rsplit('/').next())
            .unwrap_or(&display_id)
            .split('_')
            .next()
            .unwrap_or(&display_id)
            .trim_end_matches(".mp4")
            .trim_end_matches(".mov")
            .to_owned();
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(media_id));
        info.insert("display_id", serde_json::json!(display_id));
        info.insert("url", serde_json::json!(video_url.clone()));
        info.insert(
            "ext",
            serde_json::json!(yt_dlp_core::determine_ext(Some(&video_url), "mp4")),
        );
        info.insert(
            "title",
            serde_json::json!(
                html_meta_value(&html, "og:title")
                    .or_else(|| html_title_value(&html))
                    .unwrap_or_else(|| media_id.clone())
            ),
        );
        info.insert_if_some("thumbnail", html_meta_value(&html, "og:image"));
        info.insert_if_some("description", html_meta_value(&html, "og:description"));
        info.insert("upload_date", serde_json::json!(upload_date));
        info.insert("age_limit", serde_json::json!(18));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": video_url,
                "format_id": "source",
                "ext": yt_dlp_core::determine_ext(
                    info.get_str("url"),
                    "mp4",
                ),
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native Historic Films page extractor. The page supplies the tape ID and
/// descriptive metadata while the media URL follows a stable service path.
pub struct HistoricFilmsExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl HistoricFilmsExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for HistoricFilmsExtractor {
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
                    "Historic Films URL has no ID",
                )
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let tape_id = Regex::new(
            r#"(?is)(?:class\s*=\s*["']tapeId["'][^>]*>|["']tapeId["']\s*:\s*["'])([^<"']+)"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&html).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().trim().to_owned())
        .map(|value| {
            value
                .rsplit_once(':')
                .filter(|(_, suffix)| !suffix.contains('/'))
                .map_or(value.clone(), |(_, suffix)| suffix.to_owned())
        })
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Historic Films page {video_id} has no tape ID"),
            )
        })?;
        let media_url = format!("http://www.historicfilms.com/video/{tape_id}_{video_id}_web.mov");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!("mov"));
        info.insert_if_some(
            "title",
            html_meta_value(&html, "og:title").or_else(|| html_title_value(&html)),
        );
        info.insert_if_some("description", html_meta_value(&html, "og:description"));
        info.insert_if_some(
            "thumbnail",
            html_meta_value(&html, "thumbnailUrl").or_else(|| html_meta_value(&html, "og:image")),
        );
        info.insert_if_some(
            "duration",
            html_meta_value(&html, "duration")
                .and_then(|value| yt_dlp_core::parse_duration(&value)),
        );
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "source",
                "ext": "mov",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native OnePlace podcast episode extractor.
pub struct OnePlacePodcastExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl OnePlacePodcastExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for OnePlacePodcastExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "OnePlace URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let media_url = [
            r#"(?is)\bmp3-url\s*=\s*"([^"]+)"#,
            r#"(?is)<div[^>]+\bid\s*=\s*"player"[^>]+\bdata-media-url\s*=\s*"([^"]+)"#,
        ]
        .iter()
        .find_map(|pattern| {
            Regex::new(pattern)
                .ok()
                .and_then(|matcher| matcher.captures(&html).ok().flatten())
                .and_then(|captures| captures.get(1))
                .map(|value| resolve_url(url, &unescape_html_attribute(value.as_str())))
        })
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("OnePlace episode {video_id} has no audio URL"),
            )
        })?;
        let title = Regex::new(
            r#"(?is)<div[^>]*\bclass\s*=\s*"[^"]*\bdetails\b[^"]*"[^>]*>.*?<h2\b[^>]*>(.*?)</h2>"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&html).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| html_text_fragment(value.as_str()))
        .filter(|value| !value.is_empty())
        .or_else(|| html_meta_value(&html, "og:title"))
        .or_else(|| html_title_value(&html));
        let description =
            Regex::new(r#"(?is)<div[^>]*\bclass\s*=\s*"[^"]*\bepDesc\b[^"]*"[^>]*>(.*?)</div>"#)
                .ok()
                .and_then(|matcher| matcher.captures(&html).ok().flatten())
                .and_then(|captures| captures.get(1))
                .map(|value| html_text_fragment(value.as_str()))
                .filter(|value| !value.is_empty());
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!("mp3"));
        info.insert("vcodec", serde_json::json!("none"));
        info.insert_if_some("title", title);
        info.insert_if_some("description", description);
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "audio",
                "ext": "mp3",
                "vcodec": "none",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}
