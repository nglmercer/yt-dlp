/// Native RFI Français Facile audio page extractor.
pub struct FrancaisFacileExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FrancaisFacileExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FrancaisFacileExtractor {
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
        let encoded_display_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Français Facile URL has no article ID",
                )
            })?;
        let display_id = percent_decode(&encoded_display_id);
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let media_data = francais_facile_media_json(&webpage).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Français Facile article {display_id} has no audio data"),
            )
        })?;
        let media_id = json_value_string(media_data.get("mediaId")).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Français Facile article {display_id} has no media ID"),
            )
        })?;
        let source = media_data
            .get("sources")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .find_map(|source| {
                let url = json_string(source, "url").filter(|value| {
                    value.starts_with("http://") || value.starts_with("https://")
                })?;
                Some((url.to_owned(), json_f64(source, "duration")))
            })
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Français Facile audio {media_id} has no source URL"),
                )
            })?;
        let json_ld = html_json_ld(&webpage).unwrap_or(serde_json::Value::Null);
        let title = json_string(&media_data, "title")
            .map(str::to_owned)
            .or_else(|| html_title_value(&webpage))
            .unwrap_or_else(|| display_id.clone());
        let duration = source.1.or_else(|| json_ld_duration(&json_ld));
        let extension = yt_dlp_core::determine_ext(Some(&source.0), "mp3");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(media_id));
        info.insert("display_id", serde_json::json!(display_id));
        info.insert("title", serde_json::json!(title));
        info.insert("url", serde_json::json!(source.0.clone()));
        info.insert("ext", serde_json::json!(extension.clone()));
        info.insert("vcodec", serde_json::json!("none"));
        info.insert_if_some("duration", duration);
        info.insert_if_some(
            "description",
            json_string(&json_ld, "description")
                .map(str::to_owned)
                .or_else(|| html_meta_value(&webpage, "description")),
        );
        let published = json_string(&json_ld, "datePublished")
            .map(str::to_owned)
            .or_else(|| json_string(&json_ld, "uploadDate").map(str::to_owned));
        info.insert_if_some("timestamp", published.clone().and_then(parse_timestamp));
        info.insert_if_some("upload_date", published.as_deref().and_then(date_digits));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": source.0,
                "format_id": "audio",
                "ext": extension,
                "vcodec": "none",
            }]),
        );
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}

fn francais_facile_media_json(html: &str) -> Option<serde_json::Value> {
    let patterns = [
        r#"(?is)<script\b[^>]*\bdata-media-id\s*=\s*["'][^"']+["'][^>]*\btype\s*=\s*["']application/json["'][^>]*>(.*?)</script>"#,
        r#"(?is)<script\b[^>]*\btype\s*=\s*["']application/json["'][^>]*\bdata-media-id\s*=\s*["'][^"']+["'][^>]*>(.*?)</script>"#,
    ];
    patterns.iter().find_map(|pattern| {
        Regex::new(pattern)
            .ok()
            .and_then(|matcher| matcher.captures(html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .and_then(|value| serde_json::from_str(value.as_str().trim()).ok())
    })
}

fn json_ld_duration(value: &serde_json::Value) -> Option<f64> {
    json_f64(value, "duration").or_else(|| {
        json_string(value, "duration").and_then(yt_dlp_core::parse_duration)
    })
}
