pub struct LeExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

pub struct LePlaylistExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl LeExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl LePlaylistExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

fn le_video_id(matcher: &Regex, url: &str) -> Result<String, ExtractorError> {
    matcher
        .captures(url)
        .ok()
        .flatten()
        .and_then(|captures| captures.name("id"))
        .map(|value| value.as_str().to_owned())
        .ok_or_else(|| {
            ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Le URL has no video ID")
        })
}

fn le_publish_timestamp(webpage: &str) -> Option<i64> {
    let raw = Regex::new(r#"(?is)发布时间\s*(?:&nbsp;|\s)*([^<>]+)\s"#)
        .ok()
        .and_then(|matcher| matcher.captures(webpage).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().trim().to_owned())?;
    let matcher =
        Regex::new(r#"^(\d{4})[-/.](\d{1,2})[-/.](\d{1,2})[ T]+(\d{1,2}):(\d{2})(?::(\d{2}))?"#)
            .ok()?;
    if let Some(captures) = matcher.captures(&raw).ok().flatten() {
        let year = captures.get(1)?.as_str();
        let month = captures.get(2)?.as_str();
        let day = captures.get(3)?.as_str();
        let hour = captures.get(4)?.as_str();
        let minute = captures.get(5)?.as_str();
        let second = captures.get(6).map_or("00", |value| value.as_str());
        if let Some(timestamp) = yt_dlp_core::parse_iso8601(&format!(
            "{year}-{month:0>2}-{day:0>2}T{hour:0>2}:{minute:0>2}:{second:0>2}+08:00"
        )) {
            return Some(timestamp);
        }
    }
    parse_timestamp(raw)
}

impl InfoExtractor for LeExtractor {
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
        let video_id = le_video_id(&self.matcher, url)?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let play_json = le_play_json(context, &video_id)?;
        let playurl = play_json
            .get("msgs")
            .and_then(|msgs| msgs.get("playurl"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Le video {video_id} has no play URL object"),
                )
            })?;
        let domain = playurl
            .get("domain")
            .and_then(serde_json::Value::as_array)
            .and_then(|domains| domains.first())
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Le video {video_id} has no play domain"),
                )
            })?;
        let dispatch = playurl
            .get("dispatch")
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Le video {video_id} has no format dispatch"),
                )
            })?;
        let mut formats = Vec::new();
        let mut seen = Vec::new();
        for (format_id, format_data) in dispatch {
            if !seen.insert_unique(format_id.clone()) {
                continue;
            }
            formats.push(le_dispatch_format(
                context,
                &video_id,
                domain,
                format_id,
                format_data,
            )?);
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Le video {video_id} has no playable dispatched formats"),
            ));
        }
        let title = json_string(playurl, "title")
            .map(unescape_html_attribute)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Le video {video_id} has no title"),
                )
            })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some(
            "thumbnail",
            json_string(playurl, "pic").map(unescape_html_attribute),
        );
        info.insert_if_some(
            "description",
            html_meta_value(&webpage, "description").map(|value| unescape_html_attribute(&value)),
        );
        info.insert_if_some("timestamp", le_publish_timestamp(&webpage));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert(
            "_format_sort_fields",
            serde_json::json!(["res", "quality"]),
        );
        Ok(ExtractorResult::single(info))
    }
}

impl InfoExtractor for LePlaylistExtractor {
    fn descriptor(&self) -> &ExtractorDescriptor {
        &self.descriptor
    }

    fn suitable(&self, url: &str) -> bool {
        self.matcher.is_match(url).unwrap_or(false)
            && !url.contains("/ptv/vplay/")
            && !url.contains("sports.le.com/video/")
            && !url.contains("sports.le.com/match/")
            && !url.contains("lesports.com/video/")
            && !url.contains("lesports.com/match/")
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
        let playlist_id = le_video_id(&self.matcher, url)?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let matcher = Regex::new(
            r#"(?is)<a[^>]+\bhref\s*=\s*["']http://www\.letv\.com/ptv/vplay/(\d+)\.html"#,
        )
        .map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Le playlist media matcher: {error}"),
            )
        })?;
        let mut media_ids = Vec::new();
        for captures in matcher.captures_iter(&webpage).flatten() {
            let Some(media_id) = captures.get(1).map(|value| value.as_str().to_owned()) else {
                continue;
            };
            if media_ids.insert_unique(media_id) {}
        }
        let entries = media_ids
            .iter()
            .map(|media_id| {
                let mut entry = native_url_result(&format!(
                    "http://www.le.com/ptv/vplay/{media_id}.html"
                ));
                entry.insert("ie_key", serde_json::json!("Le"));
                entry
            })
            .collect::<Vec<_>>();
        let title = html_meta_value(&webpage, "keywords")
            .map(|value| value.split('，').next().unwrap_or(&value).trim().to_owned())
            .filter(|value| !value.is_empty());
        let description = html_meta_value(&webpage, "description")
            .map(|value| unescape_html_attribute(&value))
            .filter(|value| !value.is_empty());
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(playlist_id));
        info.insert_if_some("title", title);
        info.insert_if_some("description", description);
        Ok(ExtractorResult::Playlist { info, entries })
    }
}
