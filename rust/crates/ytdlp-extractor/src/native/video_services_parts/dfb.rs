/// Native tv.dfb.de XML/token/HLS extractor.
pub struct DfbExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl DfbExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for DfbExtractor {
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
                "DFB URL did not match its native pattern",
            )
        })?;
        let display_id = captures
            .name("display_id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "DFB URL has no display ID")
            })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "DFB URL has no video ID")
            })?;
        let player_xml = context.get(&format!(
            "http://tv.dfb.de/server/hd_video.php?play={video_id}"
        ))?;
        let player_xml = String::from_utf8_lossy(player_xml.body());
        let stream_access_url = xml_element_text(&player_xml, "url")
            .map(|value| proto_relative_url(&value, "https:"))
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("DFB video {video_id} has no stream access URL"),
                )
            })?;
        let mut formats = Vec::new();
        for access_url in [
            stream_access_url.clone(),
            format!("{stream_access_url}&area=&format=iphone"),
        ] {
            let response = context.get(&access_url)?;
            let xml = String::from_utf8_lossy(response.body());
            let (token_url, auth) = dfb_token_attributes(&xml).ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("DFB stream access response for {video_id} has no token"),
                )
            })?;
            let manifest_url = format!("{token_url}?hdnea={auth}");
            if manifest_url.to_ascii_lowercase().contains(".f4m") {
                return Err(ExtractorError::new(
                    ExtractorErrorKind::Unsupported,
                    format!(
                        "TODO: DFB native extractor does not implement HDS/F4M manifests: {manifest_url}"
                    ),
                ));
            }
            if !manifest_url.starts_with("http://") && !manifest_url.starts_with("https://") {
                return Err(ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("DFB video {video_id} has an invalid manifest URL"),
                ));
            }
            formats.push(serde_json::json!({
                "url": manifest_url,
                "format_id": "hls",
                "protocol": "m3u8_native",
                "ext": "mp4",
            }));
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("DFB video {video_id} has no playable formats"),
            ));
        }
        let first_url = formats
            .first()
            .and_then(|format| format.get("url"))
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let title = xml_element_text(&player_xml, "title").unwrap_or_else(|| display_id.clone());
        let mut output = InfoDict::new();
        output.insert("id", serde_json::json!(video_id));
        output.insert("display_id", serde_json::json!(display_id));
        output.insert("title", serde_json::json!(title));
        output.insert(
            "thumbnail",
            serde_json::json!(format!("http://tv.dfb.de/images/{video_id}_640x360.jpg")),
        );
        output.insert_if_some(
            "upload_date",
            xml_element_text(&player_xml, "time_date").and_then(dfb_upload_date),
        );
        output.insert("url", first_url);
        output.insert("ext", serde_json::json!("mp4"));
        output.insert("formats", serde_json::Value::Array(formats));
        output.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(output))
    }
}

fn dfb_token_attributes(xml: &str) -> Option<(String, String)> {
    let attributes = Regex::new(r#"(?is)<token\b([^>]*)>"#)
        .ok()?
        .captures(xml)
        .ok()
        .flatten()?
        .get(1)?
        .as_str()
        .to_owned();
    let attribute = |name: &str| {
        let pattern = format!(
            r#"(?is)\b{}\s*=\s*["']([^"']*)"#,
            regex::escape(name)
        );
        Regex::new(&pattern)
            .ok()?
            .captures(&attributes)
            .ok()
            .flatten()
            .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
    };
    Some((attribute("url")?, attribute("auth")?))
}

fn dfb_upload_date(value: String) -> Option<String> {
    let year_first = Regex::new(r#"(?P<year>\d{4})[./-](?P<month>\d{2})[./-](?P<day>\d{2})"#)
        .ok()
        .and_then(|matcher| matcher.captures(&value).ok().flatten())
        .and_then(|captures| {
            Some(format!(
                "{}{}{}",
                captures.name("year")?.as_str(),
                captures.name("month")?.as_str(),
                captures.name("day")?.as_str()
            ))
        });
    if year_first.is_some() {
        return year_first;
    }
    Regex::new(r#"(?P<day>\d{2})[./-](?P<month>\d{2})[./-](?P<year>\d{4})"#)
        .ok()
        .and_then(|matcher| matcher.captures(&value).ok().flatten())
        .and_then(|captures| {
            Some(format!(
                "{}{}{}",
                captures.name("year")?.as_str(),
                captures.name("month")?.as_str(),
                captures.name("day")?.as_str()
            ))
        })
}
