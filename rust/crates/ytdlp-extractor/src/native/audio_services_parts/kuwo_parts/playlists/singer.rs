/// Native Kuwo singer playlist extractor.
pub struct KuwoSingerExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl KuwoSingerExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for KuwoSingerExtractor {
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
        const PAGE_SIZE: i64 = 15;
        let singer_id = kuwo_match_id(&self.matcher, url, "singer")?;
        let (webpage, _) = kuwo_page(context, url, "singer detail")?;
        let singer_name = Regex::new(r#"(?is)<h1[^>]*>([^<]+)</h1>"#)
            .ok()
            .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| html_text_fragment(value.as_str()))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Kuwo singer {singer_id} has no singer name"),
                )
            })?;
        let artist_id = Regex::new(r#"data-artistid\s*=\s*["'](\d+)["']"#)
            .ok()
            .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Kuwo singer {singer_id} has no artist ID"),
                )
            })?;
        let page_count = Regex::new(r#"data-page\s*=\s*["'](\d+)["']"#)
            .ok()
            .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
            .and_then(|captures| captures.get(1))
            .and_then(|value| value.as_str().parse::<usize>().ok())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Kuwo singer {singer_id} has no page count"),
                )
            })?;
        let link_matcher = Regex::new(
            r#"(?is)<div[^>]+class\s*=\s*["']name["'][^>]*>\s*<a[^>]+href\s*=\s*["'](/yinyue/\d+)"#,
        )
        .map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Kuwo singer entry matcher: {error}"),
            )
        })?;
        let mut entries = Vec::new();
        for page_number in 0..page_count {
            let mut request = Request::new("http://www.kuwo.cn/artist/contentMusicsAjax");
            request.update_query(&[
                ("artistId".to_owned(), artist_id.clone()),
                ("pn".to_owned(), page_number.to_string()),
                ("rn".to_owned(), PAGE_SIZE.to_string()),
            ]);
            let page = kuwo_text_request(context, request.url(), "singer song list")?;
            for captures in link_matcher.captures_iter(&page).flatten() {
                let Some(path) = captures.get(1).map(|value| value.as_str()) else {
                    continue;
                };
                let song_url = kuwo_absolute_url(url, path);
                entries.push(kuwo_entry(&song_url));
            }
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(singer_id));
        info.insert("title", serde_json::json!(singer_name));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}
