/// Native DrTuber player-config and page-metadata extractor.
pub struct DrTuberExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl DrTuberExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for DrTuberExtractor {
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
                "DrTuber URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "DrTuber URL has no ID")
            })?;
        let display_id = captures
            .name("display_id")
            .map(|value| value.as_str().to_owned())
            .unwrap_or_else(|| video_id.clone());
        let page_url = format!("http://www.drtuber.com/video/{video_id}");
        let page_response = context.get(&page_url)?;
        let webpage = String::from_utf8_lossy(page_response.body());
        let mut config_url = Request::new("http://www.drtuber.com/player_config_json/");
        config_url.update_query(&[
            ("vid".to_owned(), video_id.clone()),
            ("embed".to_owned(), "0".to_owned()),
            ("aid".to_owned(), "0".to_owned()),
            ("domain_id".to_owned(), "0".to_owned()),
        ]);
        let config_response = context.request(&config_url)?;
        let config: serde_json::Value = serde_json::from_slice(config_response.body()).map_err(
            |error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid DrTuber player config for {video_id}: {error}"),
                )
            },
        )?;
        let files = config
            .get("files")
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("DrTuber video {video_id} has no player files"),
                )
            })?;
        let mut formats = Vec::new();
        for (format_id, value) in files {
            let Some(media_url) = value
                .as_str()
                .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            else {
                continue;
            };
            let extension = yt_dlp_core::determine_ext(Some(media_url), "mp4");
            let protocol = if extension.eq_ignore_ascii_case("m3u8") {
                "m3u8_native"
            } else {
                "http"
            };
            formats.push(serde_json::json!({
                "format_id": format_id,
                "quality": if format_id == "hq" { 2 } else { 1 },
                "url": media_url,
                "ext": if protocol == "m3u8" { "mp4" } else { extension.as_str() },
                "protocol": protocol,
            }));
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("DrTuber video {video_id} has no usable player files"),
            ));
        }
        let title = drtuber_title(&webpage).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("DrTuber video {video_id} has no title"),
            )
        })?;
        let first_format = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("display_id", serde_json::json!(display_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some(
            "thumbnail",
            drtuber_match_value(&webpage, r#"(?i)\bposter\s*=\s*["']([^"']+)"#),
        );
        info.insert_if_some(
            "duration",
            json_i64(&config, "duration")
                .map(|value| value as f64)
                .or_else(|| {
                    json_string(&config, "duration_format")
                        .and_then(|value| yt_dlp_core::parse_duration(value))
                }),
        );
        for (field, attribute) in [
            ("like_count", "rate_likes"),
            ("dislike_count", "rate_dislikes"),
            ("comment_count", "comments_count"),
        ] {
            info.insert_if_some(field, drtuber_count(&webpage, attribute));
        }
        info.insert_if_some("categories", drtuber_categories(&webpage));
        info.insert("age_limit", serde_json::json!(18));
        info.insert(
            "url",
            first_format
                .get("url")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
        info.insert(
            "ext",
            first_format
                .get("ext")
                .cloned()
                .unwrap_or_else(|| serde_json::json!("mp4")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}

fn drtuber_title(html: &str) -> Option<String> {
    let patterns = [
        r#"(?is)<h1[^>]+\bclass\s*=\s*["'][^"']*\btitle\b[^"']*["'][^>]*>([^<]+)"#,
        r#"(?is)<title>([^<]+?)\s*@\s+DrTuber"#,
        r#"(?is)\bclass\s*=\s*["']title_watch["'][^>]*>\s*<(?:p|h\d+)[^>]*>([^<]+)"#,
        r#"(?is)<p[^>]+\bclass\s*=\s*["']title_substrate["'][^>]*>([^<]+)</p>"#,
        r#"(?is)<title>([^<]+?)\s+-\s+\d+"#,
    ];
    patterns.iter().find_map(|pattern| {
        Regex::new(pattern)
            .ok()?
            .captures(html)
            .ok()
            .flatten()
            .and_then(|captures| captures.get(1))
            .map(|value| html_text_fragment(value.as_str()))
            .filter(|value| !value.is_empty())
    })
}

fn drtuber_match_value(html: &str, pattern: &str) -> Option<String> {
    Regex::new(pattern)
        .ok()?
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| unescape_html_attribute(value.as_str()))
        .filter(|value| !value.is_empty())
}

fn drtuber_count(html: &str, id: &str) -> Option<i64> {
    let pattern = format!(
        r#"(?is)<span[^>]*(?:class|id)\s*=\s*["']{}["'][^>]*>([\d,.]+)</span>"#,
        regex::escape(id)
    );
    drtuber_match_value(html, &pattern).and_then(|value| {
        let digits = value
            .chars()
            .filter(char::is_ascii_digit)
            .collect::<String>();
        digits.parse::<i64>().ok()
    })
}

fn drtuber_categories(html: &str) -> Option<Vec<String>> {
    let section = Regex::new(
        r#"(?is)<div[^>]+\bclass\s*=\s*["'][^"']*\bcategories_list\b[^"']*["'][^>]*>(.*?)</div>"#,
    )
    .ok()?
    .captures(html)
    .ok()
    .flatten()
    .and_then(|captures| captures.get(1))
    .map(|value| value.as_str().to_owned())?;
    let matcher = Regex::new(r#"(?is)<a[^>]+\btitle\s*=\s*["']([^"']+)["']"#).ok()?;
    let categories = matcher
        .captures_iter(&section)
        .flatten()
        .filter_map(|captures| captures.get(1))
        .map(|value| unescape_html_attribute(value.as_str()))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    (!categories.is_empty()).then_some(categories)
}
