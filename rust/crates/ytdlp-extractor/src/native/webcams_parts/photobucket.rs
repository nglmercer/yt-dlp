/// Native Photobucket page/API extractor.
pub struct PhotobucketExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl PhotobucketExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for PhotobucketExtractor {
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
                "Photobucket URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Photobucket URL has no ID")
            })?;
        let extension = captures
            .name("ext")
            .map(|value| value.as_str().to_ascii_lowercase())
            .unwrap_or_else(|| "mp4".to_owned());
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let data = json_object_after_marker(&html, "Pb.Data.Shared.MEDIA").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Photobucket media {video_id} has no shared metadata"),
            )
        })?;
        let html_code = data
            .get("linkcodes")
            .and_then(|value| json_string(value, "html"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Photobucket media {video_id} has no HTML link code"),
                )
            })?;
        let media_url = Regex::new(r#"(?is)\bfile=([^&\s]+?\.mp4)"#)
            .ok()
            .and_then(|matcher| matcher.captures(html_code).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| resolve_url(url, &percent_decode(value.as_str())))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Photobucket media {video_id} has no file URL"),
                )
            })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!(extension.clone()));
        info.insert_if_some("uploader", json_string(&data, "username"));
        info.insert_if_some("timestamp", json_i64(&data, "creationDate"));
        info.insert_if_some("title", json_string(&data, "title"));
        info.insert_if_some("thumbnail", json_string(&data, "thumbUrl"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "source",
                "ext": extension,
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native Nobel Prize media-page extractor. Video JSON-LD and metadata are
/// read directly; query aliases id and qid are both supported.
pub struct NobelPrizeExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl NobelPrizeExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for NobelPrizeExtractor {
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
        if !self.suitable(url) {
            return Err(ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "Nobel Prize URL did not match its native pattern",
            ));
        }
        let parsed = url::Url::parse(url).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                format!("invalid Nobel Prize URL: {error}"),
            )
        })?;
        let video_id = parsed
            .query_pairs()
            .find(|(key, _)| key == "id" || key == "qid")
            .map(|(_, value)| value.into_owned())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Nobel Prize URL requires id or qid",
                )
            })?;
        let page_url = format!(
            "https://mediaplayer.nobelprize.org{}",
            parsed
                .path()
                .is_empty()
                .then_some("/mediaplayer/")
                .unwrap_or(parsed.path())
        );
        let webpage = context.get(&page_url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let data = html_json_ld(&html).unwrap_or(serde_json::Value::Null);
        let media_url = json_string(&data, "contentUrl")
            .or_else(|| json_string(&data, "url"))
            .map(str::to_owned)
            .or_else(|| html_meta_value(&html, "contentUrl"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Nobel Prize media {video_id} has no content URL"),
                )
            })?;
        let media_url = proto_relative_url(&media_url, "https:");
        let ext = yt_dlp_core::determine_ext(Some(&media_url), "mp4");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert(
            "title",
            serde_json::json!(
                html_meta_value(&html, "caption")
                    .or_else(|| json_string(&data, "name").map(str::to_owned))
                    .unwrap_or(video_id.clone())
            ),
        );
        info.insert_if_some(
            "description",
            json_string(&data, "description")
                .map(str::to_owned)
                .or_else(|| html_meta_value(&html, "description")),
        );
        info.insert_if_some("thumbnail", json_string(&data, "thumbnailUrl"));
        info.insert_if_some(
            "duration",
            json_string(&data, "duration").and_then(yt_dlp_core::parse_duration),
        );
        info.insert_if_some(
            "timestamp",
            json_string(&data, "uploadDate")
                .map(str::to_owned)
                .and_then(parse_timestamp),
        );
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!(ext.clone()));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "source",
                "ext": ext,
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native Caltrans traffic-camera live HLS extractor.
pub struct CaltransExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl CaltransExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for CaltransExtractor {
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
        let video_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Caltrans URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let media_url = Regex::new(r#"(?is)\bvideoStreamURL\s*=\s*"([^"]+)"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| proto_relative_url(value.as_str(), "https:"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Caltrans camera {video_id} has no stream URL"),
                )
            })?;
        let route_place = Regex::new(r#"(?is)\broutePlace\s*=\s*"([^"]*)""#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| value.as_str().to_owned());
        let location = Regex::new(r#"(?is)\blocationName\s*=\s*"([^"]*)""#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| value.as_str().to_owned())
            .unwrap_or_else(|| video_id.clone());
        let title = route_place
            .map(|place| format!("{place} : {location}"))
            .unwrap_or(location);
        let thumbnail = Regex::new(r#"(?is)\bposterURL\s*=\s*"([^"]*)""#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| proto_relative_url(value.as_str(), "https:"));
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("thumbnail", thumbnail);
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!("ts"));
        info.insert("is_live", serde_json::json!(true));
        info.insert("live_status", serde_json::json!("is_live"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "hls",
                "ext": "ts",
                "protocol": "m3u8_native",
                "is_live": true,
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}
