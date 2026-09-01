/// Native Gaskrank HTML5 video extractor.
pub struct GaskrankExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl GaskrankExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for GaskrankExtractor {
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
                "Gaskrank URL did not match its native pattern",
            )
        })?;
        let display_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Gaskrank URL has no ID")
            })?;
        let category = captures
            .name("categories")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Gaskrank URL has no category",
                )
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let title = html_meta_value(&webpage, "og:title")
            .or_else(|| html_meta_value(&webpage, "title"))
            .or_else(|| gaskrank_html_title(&webpage))
            .map(|value| html_text_fragment(&value))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Gaskrank video {display_id} has no title"),
                )
            })?;

        let video_matcher = Regex::new(
            r#"(?is)(?P<url>https?://movies\.gaskrank\.tv/(?P<id>[^-]*?)(?:-[^\.\s<>'"]*)?\.mp4)"#,
        )
        .map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Gaskrank media matcher: {error}"),
            )
        })?;
        let video_capture = video_matcher.captures(&webpage).ok().flatten();
        let video_id = video_capture
            .as_ref()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .unwrap_or_else(|| display_id.clone());
        let mut formats = html5_media_formats(url, &webpage);
        if formats.is_empty() {
            if let Some(media_url) = video_capture
                .as_ref()
                .and_then(|captures| captures.name("url"))
                .map(|value| value.as_str().to_owned())
            {
                formats.push(serde_json::json!({
                    "format_id": "http",
                    "url": media_url,
                    "ext": "mp4",
                    "protocol": "http",
                }));
            }
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Gaskrank video {display_id} has no HTML5 media source"),
            ));
        }
        for format in &mut formats {
            let mut headers = format
                .get("http_headers")
                .and_then(serde_json::Value::as_object)
                .cloned()
                .unwrap_or_default();
            headers.insert("Referer".to_owned(), serde_json::json!(url));
            format["http_headers"] = serde_json::Value::Object(headers);
        }
        let first_format = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("formats", serde_json::Value::Array(formats));
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
        info.insert("categories", serde_json::json!([category]));
        info.insert("display_id", serde_json::json!(display_id));
        if let Some((uploader, upload_date)) = gaskrank_uploader_and_date(&webpage) {
            info.insert("uploader_id", serde_json::json!(uploader));
            info.insert_if_some("upload_date", upload_date);
        }
        info.insert_if_some("uploader_url", gaskrank_uploader_url(&webpage));
        let tags = gaskrank_tags(&webpage);
        if !tags.is_empty() {
            info.insert("tags", serde_json::json!(tags));
        }
        info.insert_if_some("view_count", gaskrank_view_count(&webpage));
        info.insert_if_some("average_rating", gaskrank_average_rating(&webpage));
        Ok(ExtractorResult::single(info))
    }
}

fn gaskrank_html_title(html: &str) -> Option<String> {
    let matcher = Regex::new(r#"(?is)<title\b[^>]*>(.*?)</title>"#).ok()?;
    matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1).map(|value| html_text_fragment(value.as_str())))
}

fn gaskrank_uploader_and_date(html: &str) -> Option<(String, Option<String>)> {
    let matcher = Regex::new(
        r#"(?is)Video\s+von:\s*(?P<uploader>[^|]*?)\s*\|\s*vom:\s*(?P<date>[0-9]{2}\.[0-9]{2}\.[0-9]{4})"#,
    )
    .ok()?;
    let captures = matcher.captures(html).ok().flatten()?;
    let uploader = captures
        .name("uploader")
        .map(|value| html_text_fragment(value.as_str()))
        .filter(|value| !value.is_empty())?;
    let upload_date = captures.name("date").and_then(|value| {
        let mut parts = value.as_str().split('.');
        let day = parts.next()?;
        let month = parts.next()?;
        let year = parts.next()?;
        Some(format!("{year}{month}{day}"))
    });
    Some((uploader, upload_date))
}

fn gaskrank_uploader_url(html: &str) -> Option<String> {
    let matcher = Regex::new(r#"(?is)Homepage:\s*<[^>]*>(?P<url>[^<]*)<"#).ok()?;
    matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.name("url"))
        .map(|value| html_text_fragment(value.as_str()))
        .filter(|value| !value.is_empty())
}

fn gaskrank_tags(html: &str) -> Vec<String> {
    let Ok(matcher) = Regex::new(r#"(?is)/tv/tags/[^/]+/"\s*>(?P<tag>[^<]*?)<"#) else {
        return Vec::new();
    };
    matcher
        .captures_iter(html)
        .flatten()
        .filter_map(|captures| captures.name("tag"))
        .map(|value| html_text_fragment(value.as_str()))
        .filter(|value| !value.is_empty())
        .collect()
}

fn gaskrank_view_count(html: &str) -> Option<i64> {
    let matcher = Regex::new(
        r#"(?is)class\s*=\s*["'][^"']*gkRight[^"']*["'](?:[^>]*>\s*<[^>]*)*icon-eye-open(?:[^>]*>\s*<[^>]*)*>\s*(?P<count>[0-9.]*)"#,
    )
    .ok()?;
    let value = matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.name("count"))?
        .as_str()
        .replace('.', "");
    (!value.is_empty()).then(|| value.parse::<i64>().ok()).flatten()
}

fn gaskrank_average_rating(html: &str) -> Option<f64> {
    let matcher = Regex::new(
        r#"(?is)itemprop\s*=\s*["']ratingValue["'][^>]*>\s*(?P<rating>[0-9]+(?:,[0-9]+)?)"#,
    )
    .ok()?;
    let value = matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.name("rating"))?
        .as_str()
        .replace(',', ".");
    value.parse::<f64>().ok()
}
