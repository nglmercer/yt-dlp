pub struct LecturioExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

pub struct LecturioCourseExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

pub struct LecturioDeCourseExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl LecturioExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl LecturioCourseExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl LecturioDeCourseExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for LecturioExtractor {
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
        let captures = lecturio_capture_id(&self.matcher, url)?;
        let nt = captures
            .name("nt")
            .or_else(|| captures.name("nt_de"))
            .map(|value| value.as_str().to_owned());
        let mut lecture_id = captures.name("id").map(|value| value.as_str().to_owned());
        let display_id = nt.clone().or_else(|| lecture_id.clone()).ok_or_else(|| {
            ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Lecturio URL has no lecture ID")
        })?;
        let api_path = lecture_id
            .as_ref()
            .map_or_else(|| format!("lecture/{}.json", nt.as_deref().unwrap_or_default()), |id| {
                format!("lectures/{id}")
            });
        let video = lecturio_get_json(context, &api_path, &display_id)?;
        if lecture_id.is_none() {
            let product_id = json_string(&video, "productId")
                .or_else(|| json_string(&video, "uid"))
                .and_then(|value| value.split_once('_').map(|(_, id)| id.to_owned()));
            lecture_id = product_id;
        }
        let lecture_id = lecture_id.unwrap_or(display_id);
        Ok(ExtractorResult::single(lecturio_video_info(
            &video,
            &lecture_id,
        )?))
    }
}

impl InfoExtractor for LecturioCourseExtractor {
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
        let captures = lecturio_capture_id(&self.matcher, url)?;
        let nt = captures.name("nt").map(|value| value.as_str().to_owned());
        let course_id = captures.name("id").map(|value| value.as_str().to_owned());
        let display_id = nt.clone().or_else(|| course_id.clone()).ok_or_else(|| {
            ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Lecturio course has no ID")
        })?;
        let api_path = course_id.as_ref().map_or_else(
            || format!("course/content/{}.json", nt.as_deref().unwrap_or_default()),
            |id| format!("courses/{id}"),
        );
        let course = lecturio_get_json(context, &api_path, &display_id)?;
        let mut entries = Vec::new();
        if let Some(lectures) = course.get("lectures").and_then(serde_json::Value::as_array) {
            for lecture in lectures {
                let lecture_id = json_value_string(lecture.get("id"));
                let lecture_url = json_string(lecture, "url")
                    .map(|value| resolve_url(url, value))
                    .or_else(|| {
                        lecture_id.as_ref().map(|id| {
                            format!(
                                "https://app.lecturio.com/#/lecture/c/{}/{id}",
                                course_id.as_deref().unwrap_or_default()
                            )
                        })
                    });
                let Some(lecture_url) = lecture_url else {
                    continue;
                };
                let mut entry = native_url_result(&lecture_url);
                entry.insert("ie_key", serde_json::json!("Lecturio"));
                entry.insert_if_some("id", lecture_id);
                entries.push(entry);
            }
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(display_id));
        info.insert_if_some("title", json_string(&course, "title"));
        info.insert_if_some(
            "description",
            json_string(&course, "description").map(html_text_fragment),
        );
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

impl InfoExtractor for LecturioDeCourseExtractor {
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
        let display_id = lecturio_capture_id(&self.matcher, url)?
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Lecturio German course has no ID")
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let lecture_matcher = Regex::new(
            r#"(?is)<td[^>]+\bdata-lecture-id\s*=\s*[\"'](\d+)[\"'][^>]*>.*?\bhref\s*=\s*[\"']([^\"']+\.vortrag)\b[^>]*>"#,
        )
        .map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Lecturio German course matcher: {error}"),
            )
        })?;
        let mut entries = Vec::new();
        for captures in lecture_matcher.captures_iter(&webpage).flatten() {
            let Some(lecture_id) = captures.get(1).map(|value| value.as_str().to_owned()) else {
                continue;
            };
            let Some(lecture_url) = captures
                .get(2)
                .map(|value| resolve_url(url, value.as_str()))
            else {
                continue;
            };
            let mut entry = native_url_result(&lecture_url);
            entry.insert("ie_key", serde_json::json!("Lecturio"));
            entry.insert("id", serde_json::json!(lecture_id));
            entries.push(entry);
        }
        let title = Regex::new(r#"(?is)<h1[^>]*>([^<]+)"#)
            .ok()
            .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
            .and_then(|captures| captures.get(1).map(|value| html_text_fragment(value.as_str())));
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(display_id));
        info.insert_if_some("title", title);
        Ok(ExtractorResult::Playlist { info, entries })
    }
}
