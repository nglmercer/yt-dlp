/// Native CTS News direct-feed extractor.
pub struct CtsNewsExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl CtsNewsExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for CtsNewsExtractor {
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
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "CTS News URL has no page ID",
                )
            })?;
        let page_response = context.get(url)?;
        let webpage = String::from_utf8_lossy(page_response.body());
        let news_id = html_named_input_value(&webpage, "get_id");
        let Some(news_id) = news_id else {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!(
                    "TODO: CTS News native extractor does not implement the embedded YouTube fallback for {page_id}"
                ),
            ));
        };
        let mut feed_request = Request::new("http://news.cts.com.tw/action/test_mp4feed.php");
        feed_request.update_query(&[("news_id".to_owned(), news_id.clone())]);
        let feed_response = context.request(&feed_request)?;
        let feed: serde_json::Value = serde_json::from_slice(feed_response.body()).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid CTS News feed JSON for {news_id}: {error}"),
            )
        })?;
        let media_url = json_string(&feed, "source_url")
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            .map(str::to_owned)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("CTS News feed {news_id} has no source URL"),
                )
            })?;
        let extension = yt_dlp_core::determine_ext(Some(&media_url), "mp4");
        let protocol = if extension.eq_ignore_ascii_case("m3u8") {
            "m3u8_native"
        } else {
            "http"
        };
        let datetime = Regex::new(
            r#"(?P<date>\d{4}/\d{2}/\d{2} \d{2}:\d{2})"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
        .and_then(|captures| captures.name("date"))
        .map(|value| value.as_str().to_owned());
        let timestamp = datetime
            .as_deref()
            .and_then(ctsnews_timestamp);
        let mut output = InfoDict::new();
        output.insert("id", serde_json::json!(news_id));
        output.insert("url", serde_json::json!(media_url));
        output.insert("ext", serde_json::json!(extension));
        output.insert_if_some(
            "title",
            html_meta_value(&webpage, "title").map(|value| unescape_html_attribute(&value)),
        );
        output.insert_if_some(
            "description",
            html_meta_value(&webpage, "description")
                .map(|value| unescape_html_attribute(&value)),
        );
        output.insert_if_some(
            "thumbnail",
            html_meta_value(&webpage, "image").map(|value| unescape_html_attribute(&value)),
        );
        output.insert_if_some("timestamp", timestamp);
        output.insert_if_some("upload_date", datetime.as_deref().and_then(date_digits));
        output.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": protocol,
                "protocol": protocol,
                "ext": extension,
            }]),
        );
        output.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(output))
    }
}

fn ctsnews_timestamp(value: &str) -> Option<i64> {
    let normalized = value.replace('/', "-").replace(' ', "T");
    let iso_value = format!("{normalized}:00Z");
    yt_dlp_core::parse_iso8601(&iso_value).map(|timestamp| timestamp - 8 * 3_600)
}
