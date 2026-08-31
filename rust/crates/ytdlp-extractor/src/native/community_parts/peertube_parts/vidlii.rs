/// Native VidLii page extractor. Media URLs are embedded in the player
/// configuration and are checked with native HEAD requests before exposure.
pub struct VidLiiExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl VidLiiExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for VidLiiExtractor {
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
                "VidLii URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "VidLii URL has no ID")
            })?;
        let page_url = format!("https://www.vidlii.com/watch?v={video_id}");
        let webpage_response = context.get(&page_url)?;
        let webpage = String::from_utf8_lossy(webpage_response.body());
        let parsed_page = url::Url::parse(&page_url).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                format!("invalid VidLii page URL: {error}"),
            )
        })?;

        let source_matcher =
            Regex::new(r#"(?is)\bsrc\s*:\s*["']([^"']+)["']"#).map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid VidLii source matcher: {error}"),
                )
            })?;
        let height_matcher = Regex::new(r#"(?i)(\d+)\.mp4"#).ok();
        let mut formats = Vec::new();
        for captures in source_matcher.captures_iter(&webpage).flatten() {
            let Some(raw_url) = captures.get(1).map(|value| value.as_str()) else {
                continue;
            };
            let source_url = parsed_page
                .join(&proto_relative_url(raw_url, "https:"))
                .map(|value| value.to_string())
                .unwrap_or_else(|_| raw_url.to_owned());
            let height = height_matcher
                .as_ref()
                .and_then(|matcher| matcher.captures(&source_url).ok().flatten())
                .and_then(|captures| captures.get(1))
                .and_then(|value| value.as_str().parse::<i64>().ok())
                .unwrap_or(360);
            let mut request = Request::new(&source_url);
            request.set_method("HEAD").map_err(map_request_error)?;
            if context.request(&request).is_err() {
                continue;
            }
            formats.push(serde_json::json!({
                "url": source_url,
                "format_id": format!("{height}p"),
                "height": height,
                "ext": "mp4",
            }));
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("VidLii video {video_id} has no playable source URLs"),
            ));
        }

        let title = Regex::new(r#"(?is)<h1\b[^>]*>(.*?)</h1>"#)
            .ok()
            .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| html_text_fragment(value.as_str()))
            .filter(|value| !value.is_empty())
            .or_else(|| {
                html_title_value(&webpage)
                    .map(|value| value.trim_end_matches(" - VidLii").trim().to_owned())
            })
            .unwrap_or_else(|| video_id.to_owned());
        let description = html_meta_value(&webpage, "description")
            .or_else(|| html_meta_value(&webpage, "twitter:description"))
            .or_else(|| {
                html_element_by_id(&webpage, "des_text")
                    .map(|value| html_text_fragment(&value))
                    .filter(|value| !value.is_empty())
            });
        let thumbnail = html_meta_value(&webpage, "twitter:image").or_else(|| {
            Regex::new(r#"(?is)\bimg\s*:\s*["']([^"']+)["']"#)
                .ok()
                .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
                .and_then(|captures| captures.get(1).map(|value| value.as_str()))
                .and_then(|value| {
                    parsed_page
                        .join(&proto_relative_url(value, "https:"))
                        .ok()
                        .map(|value| value.to_string())
                })
        });
        let (uploader_id, uploader) = Regex::new(
            r#"(?is)<div[^>]*class=["'][^"']*\bwt_person\b[^"']*["'][^>]*>\s*<a[^>]*href=["']/user/([^"'/?#]+)["'][^>]*>(.*?)</a>"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
        .map(|captures| {
            let uploader_id = captures.get(1).map(|value| value.as_str().to_owned());
            let uploader = captures
                .get(2)
                .map(|value| html_text_fragment(value.as_str()))
                .filter(|value| !value.is_empty());
            (uploader_id, uploader)
        })
        .unwrap_or((None, None));
        let upload_date = html_meta_value(&webpage, "datePublished")
            .or_else(|| {
                Regex::new(r#"(?is)<date\b[^>]*>([^<]+)"#)
                    .ok()
                    .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
                    .and_then(|captures| {
                        captures
                            .get(1)
                            .map(|value| value.as_str().trim().to_owned())
                    })
            })
            .and_then(parse_timestamp);
        let duration = html_meta_value(&webpage, "video:duration")
            .or_else(|| html_json_number(&webpage, "duration"))
            .and_then(|value| value.parse::<f64>().ok());
        let view_count = Regex::new(
            r#"(?is)(?:<strong>\s*([0-9,]+)\s*</strong>\s*views|Views\s*:\s*<strong>\s*([0-9,]+)\s*</strong>)"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
        .and_then(|captures| captures.get(1).or_else(|| captures.get(2)))
        .and_then(|value| value.as_str().replace(',', "").parse::<i64>().ok());
        let comment_count = Regex::new(
            r#"(?is)(?:<span[^>]*id=["']cmt_num["'][^>]*>\s*(\d+)|Comments\s*:\s*<strong>\s*(\d+))"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
        .and_then(|captures| captures.get(1).or_else(|| captures.get(2)))
        .and_then(|value| value.as_str().parse::<i64>().ok());
        let average_rating = Regex::new(r#"(?is)\brating\s*:\s*([0-9.]+)"#)
            .ok()
            .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
            .and_then(|captures| captures.get(1))
            .and_then(|value| value.as_str().parse::<f64>().ok());
        let category =
            Regex::new(r#"(?is)<div>\s*Category\s*:\s*</div>\s*<div>\s*<a[^>]*>([^<]+)"#)
                .ok()
                .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
                .and_then(|captures| captures.get(1))
                .map(|value| html_text_fragment(value.as_str()))
                .filter(|value| !value.is_empty());
        let tags =
            Regex::new(r#"(?is)<a[^>]*\bhref=["']/results\?[^"']*\bq=[^"']*["'][^>]*>([^<]+)</a>"#)
                .ok()
                .map(|matcher| {
                    matcher
                        .captures_iter(&webpage)
                        .flatten()
                        .filter_map(|captures| captures.get(1))
                        .map(|value| html_text_fragment(value.as_str()))
                        .filter(|value| !value.is_empty())
                        .map(serde_json::Value::String)
                        .collect::<Vec<_>>()
                })
                .filter(|values| !values.is_empty());
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert(
            "url",
            first.get("url").cloned().unwrap_or(serde_json::Value::Null),
        );
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some("description", description);
        info.insert_if_some("thumbnail", thumbnail);
        info.insert_if_some(
            "uploader_url",
            uploader_id
                .as_deref()
                .map(|value| format!("https://www.vidlii.com/user/{value}")),
        );
        info.insert_if_some("uploader_id", uploader_id);
        info.insert_if_some("uploader", uploader);
        info.insert_if_some("timestamp", upload_date);
        info.insert_if_some("duration", duration);
        info.insert_if_some("view_count", view_count);
        info.insert_if_some("comment_count", comment_count);
        info.insert_if_some("average_rating", average_rating);
        info.insert_if_some("categories", category.map(|value| vec![value]));
        info.insert_if_some("tags", tags);
        Ok(ExtractorResult::single(info))
    }
}
