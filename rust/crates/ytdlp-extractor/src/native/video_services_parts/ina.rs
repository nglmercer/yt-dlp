/// Native INA media extractor backed by the page asset-details JSON API.
pub struct InaExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl InaExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for InaExtractor {
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
        let display_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "INA URL has no ID")
            })?;
        let page_response = context.get(url)?;
        let webpage = String::from_utf8_lossy(page_response.body());
        let api_url = ina_asset_details_url(&webpage).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("INA page {display_id} has no asset-details URL"),
            )
        })?;
        let asset_id = Regex::new(r#"assets/([^?/]+)"#)
            .ok()
            .and_then(|matcher| matcher.captures(&api_url).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("INA asset API URL has no asset ID: {api_url}"),
                )
            })?;
        let json_url = api_url.replacen(&asset_id, &format!("{asset_id}.json"), 1);
        let api_response = context.get_json(&json_url)?;
        let media_url = json_string(&api_response, "resourceUrl")
            .filter(|value| !value.trim().is_empty())
            .map(str::to_owned)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("INA asset {asset_id} has no resource URL"),
                )
            })?;
        let media_type = json_string(&api_response, "type").unwrap_or("video");
        let (extension, format_fields) = match media_type {
            "video" => ("mp4", serde_json::json!({})),
            "audio" => (
                "mp3",
                serde_json::json!({
                    "vcodec": "none",
                }),
            ),
            other => {
                return Err(ExtractorError::new(
                    ExtractorErrorKind::Unsupported,
                    format!(
                        "TODO: INA asset {asset_id} returned unsupported media type {other:?}"
                    ),
                ));
            }
        };
        let mut format = serde_json::json!({
            "url": media_url,
            "format_id": "source",
            "ext": extension,
            "protocol": "http",
        });
        if let Some(fields) = format_fields.as_object() {
            for (key, value) in fields {
                format[key] = value.clone();
            }
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(asset_id));
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!(extension));
        info.insert("formats", serde_json::json!([format]));
        info.insert_if_some("title", json_string(&api_response, "title"));
        info.insert_if_some("description", json_string(&api_response, "description"));
        info.insert_if_some(
            "upload_date",
            json_string(&api_response, "dateOfBroadcast").and_then(ina_date_digits),
        );
        info.insert_if_some("duration", json_f64(&api_response, "duration"));
        info.insert_if_some("thumbnail", json_string(&api_response, "resourceThumbnail"));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

fn ina_asset_details_url(webpage: &str) -> Option<String> {
    let matcher = Regex::new(r#"(?is)\basset-details-url\s*=\s*["']([^"']+)"#).ok()?;
    matcher
        .captures(webpage)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| unescape_html_attribute(value.as_str()))
        .filter(|value| !value.trim().is_empty())
}

fn ina_date_digits(value: &str) -> Option<String> {
    let iso_matcher = Regex::new(r#"(?P<year>\d{4})[-/](?P<month>\d{2})[-/](?P<day>\d{2})"#).ok()?;
    if let Some(captures) = iso_matcher.captures(value).ok().flatten() {
        let year = captures.name("year")?.as_str();
        let month = captures.name("month")?.as_str();
        let day = captures.name("day")?.as_str();
        return Some(format!("{year}{month}{day}"));
    }
    date_digits(value)
}
