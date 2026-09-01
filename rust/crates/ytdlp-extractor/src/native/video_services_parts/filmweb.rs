/// Native Filmweb article-to-TwentyThreeVideo wrapper.
pub struct FilmwebExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FilmwebExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FilmwebExtractor {
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
                "Filmweb URL did not match its native pattern",
            )
        })?;
        let article_type = captures
            .name("type")
            .map(|value| value.as_str())
            .unwrap_or_default();
        let mut article_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Filmweb URL has no article ID")
            })?;
        if article_type == "filmnytt" {
            let response = context.get(url)?;
            let webpage = String::from_utf8_lossy(response.body());
            article_id = Regex::new(r#"(?is)\bdata-videoid\s*=\s*["'](\d+)"#)
                .ok()
                .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
                .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
                .ok_or_else(|| {
                    ExtractorError::new(
                        ExtractorErrorKind::Extraction,
                        format!("Filmweb article {article_id} has no video ID"),
                    )
                })?;
        }
        let mut request =
            Request::new("https://www.filmweb.no/template_v2/ajax/json_trailerEmbed.jsp");
        request.update_query(&[("articleId".to_owned(), article_id.clone())]);
        let response = context.request(&request)?;
        let data = serde_json::from_slice::<serde_json::Value>(response.body()).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Filmweb trailer JSON for {article_id}: {error}"),
            )
        })?;
        let embed_code = json_string(&data, "embedCode").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Filmweb trailer {article_id} has no embed code"),
            )
        })?;
        let iframe_url = Regex::new(r#"(?is)<iframe\b[^>]*\bsrc\s*=\s*["']([^"']+)"#)
            .ok()
            .and_then(|matcher| matcher.captures(embed_code).ok().flatten())
            .and_then(|captures| captures.get(1).map(|value| value.as_str()))
            .map(|value| proto_relative_url(value, "https:"))
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Filmweb trailer {article_id} has no iframe URL"),
                )
            })?;
        let mut info = native_url_result(&iframe_url);
        info.insert("_type", serde_json::json!("url_transparent"));
        info.insert("id", serde_json::json!(article_id));
        info.insert("ie_key", serde_json::json!("TwentyThreeVideo"));
        Ok(ExtractorResult::single(info))
    }
}
