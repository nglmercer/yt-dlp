/// Native Egghead course playlist extractor.
pub struct EggheadCourseExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl EggheadCourseExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for EggheadCourseExtractor {
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
        let playlist_slug = egghead_match_id(&self.matcher, url, "Egghead course")?;
        let lessons_url = format!(
            "https://app.egghead.io/api/v1/series/{playlist_slug}/lessons"
        );
        let lessons_data = context.get_json(&lessons_url)?;
        let lessons = lessons_data.as_array().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Egghead course {playlist_slug} lessons are not an array"),
            )
        })?;
        let mut entries = Vec::new();
        for lesson in lessons {
            let Some(lesson_url) = json_string(lesson, "http_url")
                .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            else {
                continue;
            };
            let mut entry = native_url_result(lesson_url);
            entry.insert("ie_key", serde_json::json!("egghead:lesson"));
            if let Some(lesson_id) = json_value_string(lesson.get("id")) {
                entry.insert("id", serde_json::json!(lesson_id));
            }
            entries.push(entry);
        }
        let course_url = format!("https://app.egghead.io/api/v1/series/{playlist_slug}");
        let course = context.get_json(&course_url)?;
        let course_id = json_value_string(course.get("id")).unwrap_or(playlist_slug.clone());
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(course_id));
        info.insert_if_some("title", json_string(&course, "title").map(str::to_owned));
        info.insert_if_some(
            "description",
            json_string(&course, "description").map(str::to_owned),
        );
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

/// Native Egghead lesson API extractor.
pub struct EggheadLessonExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl EggheadLessonExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for EggheadLessonExtractor {
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
        let display_id = egghead_match_id(&self.matcher, url, "Egghead lesson")?;
        let lesson_url = format!("https://app.egghead.io/api/v1/lessons/{display_id}");
        let lesson = context.get_json(&lesson_url)?;
        let lesson_id = json_value_string(lesson.get("id")).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Egghead lesson {display_id} has no numeric ID"),
            )
        })?;
        let title = json_string(&lesson, "title")
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Egghead lesson {display_id} has no title"),
                )
            })?;
        let mut formats = Vec::new();
        if let Some(media_urls) = lesson.get("media_urls").and_then(serde_json::Value::as_object)
        {
            for media_url in media_urls.values().filter_map(serde_json::Value::as_str) {
                if !media_url.starts_with("http://") && !media_url.starts_with("https://") {
                    continue;
                }
                let extension = yt_dlp_core::determine_ext(Some(media_url), "mp4");
                let (format_id, protocol, ext) = match extension.to_ascii_lowercase().as_str() {
                    "m3u8" => ("hls".to_owned(), "m3u8_native".to_owned(), "mp4".to_owned()),
                    "mpd" => (
                        "dash".to_owned(),
                        "http_dash_segments".to_owned(),
                        "mp4".to_owned(),
                    ),
                    _ => ("http".to_owned(), "http".to_owned(), extension),
                };
                formats.push(serde_json::json!({
                    "url": media_url,
                    "format_id": format_id,
                    "protocol": protocol,
                    "ext": ext,
                }));
            }
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Egghead lesson {display_id} has no playable media URLs"),
            ));
        }
        let first_format = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let tags = lesson
            .get("tag_list")
            .and_then(serde_json::Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .filter(|values| !values.is_empty());
        let published = json_string(&lesson, "published_at").map(str::to_owned);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(lesson_id));
        info.insert("display_id", serde_json::json!(display_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", json_string(&lesson, "summary").map(str::to_owned));
        info.insert_if_some(
            "thumbnail",
            json_string(&lesson, "thumb_nail").map(str::to_owned),
        );
        info.insert_if_some("timestamp", published.and_then(parse_timestamp));
        info.insert_if_some("duration", json_i64(&lesson, "duration"));
        info.insert_if_some("view_count", json_i64(&lesson, "plays_count"));
        info.insert_if_some("tags", tags.map(|values| serde_json::json!(values)));
        info.insert_if_some(
            "series",
            lesson
                .get("series")
                .and_then(|series| json_string(series, "title"))
                .map(str::to_owned),
        );
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

fn egghead_match_id(
    matcher: &Regex,
    url: &str,
    label: &str,
) -> Result<String, ExtractorError> {
    matcher
        .captures(url)
        .ok()
        .flatten()
        .and_then(|captures| captures.name("id"))
        .map(|value| value.as_str().to_owned())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                format!("{label} URL has no ID"),
            )
        })
}
