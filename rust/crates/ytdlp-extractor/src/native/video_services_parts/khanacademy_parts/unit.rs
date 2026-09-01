fn khan_collect_video_entries(value: &serde_json::Value, entries: &mut Vec<InfoDict>) {
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                khan_collect_video_entries(value, entries);
            }
        }
        serde_json::Value::Object(values) => {
            if values.get("contentKind").and_then(serde_json::Value::as_str) == Some("Video") {
                if let Some(canonical_url) = values
                    .get("canonicalUrl")
                    .and_then(serde_json::Value::as_str)
                    .filter(|url| url.starts_with('/'))
                {
                    let mut entry = native_url_result(&resolve_url(
                        "https://www.khanacademy.org",
                        canonical_url,
                    ));
                    entry.insert("ie_key", serde_json::json!("KhanAcademy"));
                    entry.insert_if_some("title", json_string(value, "translatedTitle"));
                    entries.push(entry);
                    return;
                }
            }
            for value in values.values() {
                khan_collect_video_entries(value, entries);
            }
        }
        _ => {}
    }
}

/// Native Khan Academy unit-to-video playlist extractor.
pub struct KhanAcademyUnitExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl KhanAcademyUnitExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for KhanAcademyUnitExtractor {
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
            .map(|value| value.as_str().trim_end_matches('/').to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Khan Academy unit URL has no display ID",
                )
            })?;
        let content = khan_content(context, &display_id)?;
        let course = content.get("course").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Khan Academy unit {display_id} has no course object"),
            )
        })?;
        let expected_relative_url = format!("/{display_id}");
        let selected_unit = course
            .get("unitChildren")
            .and_then(serde_json::Value::as_array)
            .and_then(|units| {
                units.iter().find(|unit| {
                    json_string(unit, "relativeUrl") == Some(expected_relative_url.as_str())
                })
            })
            .unwrap_or(course);
        let mut entries = Vec::new();
        khan_collect_video_entries(selected_unit, &mut entries);

        let mut info = InfoDict::new();
        info.insert("display_id", serde_json::json!(display_id));
        info.insert_if_some("id", json_string(selected_unit, "id"));
        info.insert_if_some("title", json_string(selected_unit, "translatedTitle"));
        info.insert_if_some(
            "description",
            json_string(selected_unit, "translatedDescription"),
        );
        if let Some(slug) = json_string(selected_unit, "slug") {
            info.insert(
                "_old_archive_ids",
                serde_json::json!([format!("khanacademy:unit {slug}")]),
            );
        }
        Ok(ExtractorResult::Playlist { info, entries })
    }
}
