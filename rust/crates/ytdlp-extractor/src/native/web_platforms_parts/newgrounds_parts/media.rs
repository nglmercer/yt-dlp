/// Native Newgrounds media extractor. Newgrounds exposes either a direct
/// embedController URL or a JSON source list for the media page; both paths
/// are handled through the native request stack.
pub struct NewgroundsExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl NewgroundsExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for NewgroundsExtractor {
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
                "Newgrounds URL did not match its native pattern",
            )
        })?;
        let media_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Newgrounds URL has no ID")
            })?;
        let webpage_response = context.get(url)?;
        let webpage = String::from_utf8_lossy(webpage_response.body());

        let direct_media_url =
            Regex::new(r#"(?is)embedController\(\s*\[\s*\{\s*"url"\s*:\s*("(?:\\.|[^"\\])*")"#)
                .ok()
                .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
                .and_then(|captures| captures.get(1).map(|value| value.as_str()))
                .and_then(decode_json_string);

        let mut formats = Vec::new();
        let mut uploader = None;
        if let Some(media_url) = direct_media_url {
            formats.push(serde_json::json!({
                "url": proto_relative_url(&media_url, "https:"),
                "format_id": "source",
                "quality": 1,
                "ext": yt_dlp_core::determine_ext(Some(&media_url), "mp4"),
            }));
        } else {
            let json_video = native_get_json_with_headers(
                context,
                &format!("https://www.newgrounds.com/portal/video/{media_id}"),
                &[
                    ("Accept", "application/json"),
                    ("Referer", url),
                    ("X-Requested-With", "XMLHttpRequest"),
                ],
            )?;
            uploader = json_string(&json_video, "author").map(str::to_owned);
            if let Some(sources) = json_video
                .get("sources")
                .and_then(serde_json::Value::as_object)
            {
                for (format_id, source_list) in sources {
                    let quality = format_id
                        .trim_end_matches(|character: char| character == 'p' || character == 'P')
                        .parse::<i64>()
                        .ok();
                    for media_url in json_media_urls(source_list) {
                        formats.push(serde_json::json!({
                            "url": proto_relative_url(&media_url, "https:"),
                            "format_id": format_id,
                            "quality": quality,
                            "ext": yt_dlp_core::determine_ext(Some(&media_url), "mp4"),
                        }));
                    }
                }
            }
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Newgrounds media {media_id} has no playable formats"),
            ));
        }
        if uploader.is_none() {
            uploader = Regex::new(r#"(?is)<h4[^>]*>(.*?)</h4>.*?<em>\s*(?:Author|Artist)\s*</em>"#)
                .ok()
                .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
                .and_then(|captures| {
                    captures
                        .get(1)
                        .map(|value| html_text_fragment(value.as_str()))
                })
                .filter(|value| !value.is_empty());
        }
        if uploader.is_none() {
            uploader = Regex::new(r#"(?is)(?:Author|Writer)\s*<a[^>]*>(.*?)</a>"#)
                .ok()
                .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
                .and_then(|captures| {
                    captures
                        .get(1)
                        .map(|value| html_text_fragment(value.as_str()))
                })
                .filter(|value| !value.is_empty());
        }

        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(media_id));
        info.insert(
            "title",
            serde_json::json!(html_title_value(&webpage).unwrap_or_else(|| media_id.to_owned())),
        );
        info.insert(
            "url",
            first.get("url").cloned().unwrap_or(serde_json::Value::Null),
        );
        info.insert(
            "ext",
            first
                .get("ext")
                .cloned()
                .unwrap_or_else(|| serde_json::json!("mp4")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some("uploader", uploader);
        info.insert_if_some(
            "timestamp",
            html_attribute_value(&webpage, "itemprop", "uploadDate")
                .or_else(|| html_attribute_value(&webpage, "itemprop", "datePublished"))
                .and_then(parse_timestamp),
        );
        info.insert_if_some(
            "duration",
            html_json_number(&webpage, "duration").and_then(|value| value.parse::<f64>().ok()),
        );
        info.insert_if_some("thumbnail", html_meta_value(&webpage, "og:image"));
        let description = html_element_by_id(&webpage, "author_comments")
            .map(|value| html_text_fragment(&value))
            .filter(|value| !value.is_empty())
            .or_else(|| html_meta_value(&webpage, "og:description"));
        info.insert_if_some("description", description);
        let age_limit = Regex::new(r#"(?is)<h2\s+class=["']rated-([etma])["']"#)
            .ok()
            .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
            .and_then(|captures| captures.get(1))
            .and_then(|value| match value.as_str() {
                "e" => Some(0),
                "t" => Some(13),
                "m" => Some(17),
                "a" => Some(18),
                _ => None,
            });
        info.insert_if_some("age_limit", age_limit);
        info.insert_if_some(
            "view_count",
            Regex::new(r#"(?is)<dt>\s*(?:Views|Listens)\s*</dt>\s*<dd>\s*([\d\.,]+)\s*</dd>"#)
                .ok()
                .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
                .and_then(|captures| captures.get(1))
                .and_then(|value| {
                    value
                        .as_str()
                        .replace(',', "")
                        .replace('.', "")
                        .parse::<i64>()
                        .ok()
                }),
        );
        Ok(ExtractorResult::single(info))
    }
}
