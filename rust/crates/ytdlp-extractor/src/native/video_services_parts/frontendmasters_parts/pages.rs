/// Native Frontend Masters course lesson-page extractor.
pub struct FrontendMastersLessonExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FrontendMastersLessonExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FrontendMastersLessonExtractor {
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
                "Frontend Masters lesson URL did not match its native pattern",
            )
        })?;
        let course_name = captures
            .name("course_name")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Frontend Masters lesson URL has no course name",
                )
            })?;
        let lesson_name = captures
            .name("lesson_name")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Frontend Masters lesson URL has no lesson name",
                )
            })?;
        let course = frontendmasters_api(
            context,
            &format!("/courses/{course_name}"),
            url,
        )?;
        let chapters = frontendmasters_chapters(&course);
        let lesson = course
            .get("lessonData")
            .and_then(serde_json::Value::as_object)
            .and_then(|lessons| {
                lessons
                    .iter()
                    .find(|(_, lesson)| json_string(lesson, "slug") == Some(lesson_name))
            })
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!(
                        "Frontend Masters course {course_name} has no lesson {lesson_name}"
                    ),
                )
            })?;
        let lesson_id = json_string(lesson.1, "hash")
            .or_else(|| json_string(lesson.1, "statsId"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Frontend Masters lesson {lesson_name} has no lesson ID"),
                )
            })?;
        Ok(ExtractorResult::single(frontendmasters_lesson_entry(
            &chapters,
            lesson_id,
            lesson.1,
        )))
    }
}

/// Native Frontend Masters course playlist extractor.
pub struct FrontendMastersCourseExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FrontendMastersCourseExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FrontendMastersCourseExtractor {
    fn descriptor(&self) -> &ExtractorDescriptor {
        &self.descriptor
    }

    fn suitable(&self, url: &str) -> bool {
        self.matcher.is_match(url).unwrap_or(false) && !frontendmasters_lesson_url(url)
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
        let course_name = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Frontend Masters course URL has no course name",
                )
            })?;
        let course = frontendmasters_api(
            context,
            &format!("/courses/{course_name}"),
            url,
        )?;
        let chapters = frontendmasters_chapters(&course);
        let mut lessons = course
            .get("lessonData")
            .and_then(serde_json::Value::as_object)
            .into_iter()
            .flatten()
            .filter_map(|(key, lesson)| {
                let index = json_i64(lesson, "index")?;
                let lesson_id = json_string(lesson, "hash")
                    .or_else(|| json_string(lesson, "statsId"))
                    .filter(|value| !value.is_empty())
                    .unwrap_or(key);
                let slug = json_string(lesson, "slug").filter(|value| !value.is_empty())?;
                Some((index, lesson_id.to_owned(), slug.to_owned(), lesson.clone()))
            })
            .collect::<Vec<_>>();
        lessons.sort_by_key(|(index, _, _, _)| *index);
        let entries = lessons
            .iter()
            .map(|(_, lesson_id, _, lesson)| {
                frontendmasters_lesson_entry(&chapters, lesson_id, lesson)
            })
            .collect::<Vec<_>>();
        if entries.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Frontend Masters course {course_name} has no lessons"),
            ));
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(course_name));
        info.insert_if_some("title", json_string(&course, "title"));
        info.insert_if_some("description", json_string(&course, "description"));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}
