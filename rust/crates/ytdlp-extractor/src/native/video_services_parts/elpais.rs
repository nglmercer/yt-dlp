/// Native El País video page extractor.
pub struct ElPaisExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl ElPaisExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for ElPaisExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "El País URL has no video ID")
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let prefix = elpais_capture(
            &webpage,
            r#"(?is)\bvar\s+url_cache\s*=\s*"([^"]+)"\s*;"#,
        )
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("El País page {video_id} has no media URL prefix"),
            )
        })?;
        let video_suffix = if let Some(multimedia_id) =
            elpais_capture(&webpage, r#"(?is)\bid_multimedia\s*=\s*'([^']+)'"#)
        {
            let endpoint = format!("http://elpais.com/vdpep/1/?pepid={multimedia_id}");
            let response = context.get(&endpoint)?;
            let body = String::from_utf8_lossy(response.body());
            let data = elpais_parse_jsonp(&body).ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid El País media JSON for {video_id}"),
                )
            })?;
            data.get("mp4")
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| {
                    ExtractorError::new(
                        ExtractorErrorKind::Extraction,
                        format!("El País media record {multimedia_id} has no MP4 path"),
                    )
                })?
        } else {
            elpais_capture(
                &webpage,
                r#"(?is)\b(?:URLMediaFile|urlVideo_\d+)\s*=\s*url_cache\s*\+\s*'([^']+)'"#,
            )
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("El País page {video_id} has no video URL suffix"),
                )
            })?
        };
        let video_url = format!("{prefix}{video_suffix}");
        let thumbnail = elpais_capture(
            &webpage,
            r#"(?is)\b(?:URLMediaStill|urlFotogramaFijo_\d+)\s*=\s*url_cache\s*\+\s*'([^']+)'"#,
        )
        .map(|suffix| format!("{prefix}{suffix}"))
        .or_else(|| html_meta_value(&webpage, "og:image"));
        let title = elpais_capture(&webpage, r#"(?is)\btituloVideo\s*=\s*'([^']+)'"#)
            .map(|value| unescape_html_attribute(&value))
            .or_else(|| {
                elpais_capture(
                    &webpage,
                    r#"(?is)<h2\b[^>]*\bclass\s*=\s*["'][^"']*\bentry-header\b[^"']*\bentry-title\b[^"']*["'][^>]*>(.*?)</h2>"#,
                )
                .map(|value| html_text_fragment(&value))
            })
            .or_else(|| {
                elpais_capture(
                    &webpage,
                    r#"(?is)<h1\b[^>]*\bclass\s*=\s*["']titulo["'][^>]*>([^<]+)"#,
                )
                .map(|value| html_text_fragment(&value))
            })
            .or_else(|| html_meta_value(&webpage, "og:title"));
        let description = html_meta_value(&webpage, "og:description")
            .map(|value| unescape_html_attribute(&value));
        let published = elpais_capture(
            &webpage,
            r#"(?is)<p\b[^>]*\bclass\s*=\s*["']date-header\s+date-int\s+updated["'][^>]*\btitle\s*=\s*["']([^"']+)"#,
        )
        .or_else(|| html_meta_value(&webpage, "datePublished"));

        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("url", serde_json::json!(video_url));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert_if_some("title", title);
        info.insert_if_some("description", description);
        info.insert_if_some("thumbnail", thumbnail);
        info.insert_if_some("upload_date", published.as_deref().and_then(date_digits));
        Ok(ExtractorResult::single(info))
    }
}

fn elpais_capture(html: &str, pattern: &str) -> Option<String> {
    Regex::new(pattern)
        .ok()
        .and_then(|matcher| matcher.captures(html).ok().flatten())
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
}

fn elpais_parse_jsonp(value: &str) -> Option<serde_json::Value> {
    let value = value.trim();
    if let Some(parsed) = parse_common_javascript_value(value) {
        return Some(parsed);
    }
    let open = value.find('(')?;
    let close = value.rfind(')')?;
    (close > open).then(|| parse_common_javascript_value(value[open + 1..close].trim()))?
}
