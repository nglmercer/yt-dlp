/// Native 2M.ma replay/article API extractors.
pub struct DeuxMExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl DeuxMExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for DeuxMExtractor {
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
                "2M.ma URL did not match its native pattern",
            )
        })?;
        let page_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "2M.ma URL has no page ID")
            })?;
        let video = if self.descriptor.key == "DeuxMIE" {
            let data = context.get_json(&format!("https://2m.ma/api/watchDetail/{page_id}"))?;
            data.get("response")
                .and_then(|response| response.get("News"))
                .cloned()
                .ok_or_else(|| {
                    ExtractorError::new(
                        ExtractorErrorKind::Extraction,
                        format!("2M.ma replay {page_id} has no News object"),
                    )
                })?
        } else {
            let language = captures
                .name("lang")
                .map(|value| value.as_str().to_owned())
                .ok_or_else(|| {
                    ExtractorError::new(
                        ExtractorErrorKind::InvalidUrl,
                        "2M.ma news URL has no language",
                    )
                })?;
            let mut query = url::form_urlencoded::Serializer::new(String::new());
            query.append_pair("lang", &language);
            query.append_pair("url", &format!("/news/{page_id}"));
            let data = context.get_json(&format!(
                "https://2m.ma/api/articlesByUrl?{}",
                query.finish()
            ))?;
            data.get("response")
                .and_then(|response| response.get("article"))
                .and_then(serde_json::Value::as_array)
                .and_then(|articles| articles.first())
                .cloned()
                .ok_or_else(|| {
                    ExtractorError::new(
                        ExtractorErrorKind::Extraction,
                        format!("2M.ma news {page_id} has no article object"),
                    )
                })?
        };
        let media_url = if self.descriptor.key == "DeuxMIE" {
            json_string(&video, "url").map(str::to_owned)
        } else {
            video
                .get("image")
                .and_then(serde_json::Value::as_array)
                .and_then(|images| images.first())
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        }
        .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("2M.ma page {page_id} has no media URL"),
            )
        })?;
        let extension = yt_dlp_core::determine_ext(Some(&media_url), "mp4");
        let mut info = InfoDict::new();
        info.insert(
            "id",
            if self.descriptor.key == "DeuxMIE" {
                serde_json::json!(page_id)
            } else {
                video
                    .get("id")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!(page_id))
            },
        );
        info.insert_if_some(
            "title",
            json_string(&video, if self.descriptor.key == "DeuxMIE" {
                "titre"
            } else {
                "title"
            }),
        );
        info.insert_if_some(
            "description",
            json_string(
                &video,
                if self.descriptor.key == "DeuxMIE" {
                    "description"
                } else {
                    "content"
                },
            ),
        );
        info.insert_if_some(
            "thumbnail",
            json_string(
                &video,
                if self.descriptor.key == "DeuxMIE" {
                    "image"
                } else {
                    "cover"
                },
            )
            .filter(|value| value.starts_with("http://") || value.starts_with("https://")),
        );
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!(extension));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "http",
                "protocol": "http",
                "ext": extension,
            }]),
        );
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}
