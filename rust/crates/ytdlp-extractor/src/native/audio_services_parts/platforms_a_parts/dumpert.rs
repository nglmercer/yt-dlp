impl InfoExtractor for DumpertExtractor {
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
                "Dumpert URL did not match its native pattern",
            )
        })?;
        let normalized_id = captures
            .name("id")
            .map(|value| value.as_str().replace('_', "/"))
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Dumpert URL has no ID")
            })?;
        let api_id = normalized_id.replace('/', "_");
        let response = context.get_json(&format!(
            "http://api-live.dumpert.nl/mobile_api/json/info/{api_id}"
        ))?;
        let item = response
            .get("items")
            .and_then(serde_json::Value::as_array)
            .and_then(|items| items.first())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Dumpert API returned no item",
                )
            })?;
        let media = item
            .get("media")
            .and_then(serde_json::Value::as_array)
            .and_then(|media| {
                media.iter().find(|media| {
                    media.get("mediatype").and_then(serde_json::Value::as_str) == Some("VIDEO")
                })
            })
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Dumpert item has no VIDEO media",
                )
            })?;
        let formats = media
            .get("variants")
            .and_then(serde_json::Value::as_array)
            .map(|variants| {
                variants
                    .iter()
                    .filter_map(|variant| {
                        let url = variant.get("uri").and_then(serde_json::Value::as_str)?;
                        let version = variant
                            .get("version")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("source");
                        let detected_ext = yt_dlp_core::determine_ext(Some(url), "mp4");
                        let ext = if detected_ext == "m3u8" {
                            "mp4".to_owned()
                        } else {
                            detected_ext
                        };
                        Some(serde_json::json!({
                            "url": url,
                            "format_id": version,
                            "ext": ext,
                            "protocol": if url.split('?').next().is_some_and(|url| url.ends_with(".m3u8")) {
                                "m3u8_native"
                            } else {
                                "http"
                            },
                        }))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let first = formats.first().cloned().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Dumpert media has no playable variants",
            )
        })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(normalized_id));
        info.insert_if_some("title", json_string(item, "title"));
        info.insert_if_some("description", json_string(item, "description"));
        info.insert_if_some(
            "duration",
            media.get("duration").and_then(serde_json::Value::as_f64),
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
                .unwrap_or(serde_json::json!("mp4")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        if let Some(stills) = item.get("stills").and_then(serde_json::Value::as_object) {
            let thumbnails = stills
                .iter()
                .filter_map(|(id, value)| {
                    value
                        .as_str()
                        .map(|url| serde_json::json!({"id": id, "url": url}))
                })
                .collect::<Vec<_>>();
            if !thumbnails.is_empty() {
                info.insert("thumbnails", serde_json::Value::Array(thumbnails));
            }
        }
        if let Some(stats) = item.get("stats") {
            info.insert_if_some(
                "like_count",
                stats.get("kudos_total").and_then(|value| value.as_i64()),
            );
            info.insert_if_some(
                "view_count",
                stats.get("views_total").and_then(|value| value.as_i64()),
            );
        }
        Ok(ExtractorResult::single(info))
    }
}
