/// Native HR Fernsehen/Hessenschau media-player loader extractor.
pub struct HrFernsehenExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl HrFernsehenExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for HrFernsehenExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "HR Fernsehen URL has no ID")
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let loader = hrfernsehen_loader_data(&webpage).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("HR Fernsehen video {video_id} has no media-player loader data"),
            )
        })?;
        let media = loader
            .get("mediaCollection")
            .and_then(|collection| collection.get("streams"))
            .and_then(serde_json::Value::as_array)
            .and_then(|streams| streams.first())
            .and_then(|stream| stream.get("media"))
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("HR Fernsehen video {video_id} has no stream list"),
                )
            })?;
        let mut formats = Vec::new();
        for (index, stream) in media.iter().skip(1).enumerate() {
            let Some(media_url) = json_string(stream, "url")
                .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            else {
                continue;
            };
            let height = json_i64(stream, "maxHResolutionPx");
            let mut format = serde_json::json!({
                "url": media_url,
                "format_id": height.map_or_else(|| format!("http-{index}"), |value| format!("{value}p")),
                "ext": yt_dlp_core::determine_ext(Some(media_url), "mp4"),
            });
            if let Some(height) = height {
                format["height"] = serde_json::json!(height);
            }
            if let Some((width, parsed_height, fps, bitrate)) = hrfernsehen_stream_details(media_url)
            {
                format["width"] = serde_json::json!(width);
                format["height"] = serde_json::json!(parsed_height);
                format["fps"] = serde_json::json!(fps);
                format["tbr"] = serde_json::json!(bitrate);
            }
            if format.get("ext").and_then(serde_json::Value::as_str) == Some("m3u8") {
                format["ext"] = serde_json::json!("mp4");
                format["protocol"] = serde_json::json!("m3u8_native");
            }
            formats.push(format);
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("HR Fernsehen video {video_id} has no playable streams"),
            ));
        }
        let first_format = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let title = hrfernsehen_meta(&webpage, &["og:title", "twitter:title", "name"])
            .unwrap_or_else(|| video_id.clone());
        let subtitle_url = loader
            .get("mediaCollection")
            .and_then(|collection| collection.get("subTitles"))
            .and_then(serde_json::Value::as_array)
            .and_then(|subtitles| subtitles.first())
            .and_then(|subtitle| subtitle.get("sources"))
            .and_then(serde_json::Value::as_array)
            .and_then(|sources| sources.first())
            .and_then(|source| json_string(source, "url"));
        let release_date = hrfernsehen_capture(
            &webpage,
            r#"(?is)<time\b[^>]*\bdatetime\s*=\s*["'](\d{4}\W\d{1,2}\W\d{1,2})"#,
        );
        let duration = loader
            .get("playerConfig")
            .and_then(|config| config.get("pluginData"))
            .and_then(|plugins| plugins.get("trackingAti@all"))
            .and_then(|tracking| tracking.get("richMedia"))
            .and_then(|rich_media| json_i64(rich_media, "duration"));
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", hrfernsehen_meta(&webpage, &["description"]));
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
        if let Some(subtitle_url) = subtitle_url {
            info.insert(
                "subtitles",
                serde_json::json!({"de": [{"url": subtitle_url}]}),
            );
        } else {
            info.insert("subtitles", serde_json::json!({}));
        }
        info.insert_if_some(
            "timestamp",
            release_date.clone().and_then(|value| parse_timestamp(value)),
        );
        info.insert_if_some(
            "upload_date",
            release_date.as_deref().and_then(date_digits),
        );
        info.insert_if_some("duration", duration);
        info.insert_if_some(
            "thumbnail",
            hrfernsehen_capture(&webpage, r#"(?is)thumbnailUrl\W*([^"']+)"#)
                .or_else(|| html_meta_value(&webpage, "og:image")),
        );
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

fn hrfernsehen_loader_data(html: &str) -> Option<serde_json::Value> {
    for pattern in [
        r#"(?is)\bdata-(?:new-)?hr-mediaplayer-loader\s*=\s*'([^']*)"#,
        r#"(?is)\bdata-(?:new-)?hr-mediaplayer-loader\s*=\s*"([^"]*)""#,
    ] {
        let Some(raw) = hrfernsehen_capture(html, pattern) else {
            continue;
        };
        if let Ok(value) = serde_json::from_str(&unescape_html_attribute(&raw)) {
            return Some(value);
        }
    }
    None
}

fn hrfernsehen_stream_details(url: &str) -> Option<(i64, i64, i64, i64)> {
    let matcher =
        Regex::new(r#"(?i)([0-9]{3,4})x([0-9]{3,4})-([0-9]{2})p-([0-9]{3,4})kbit"#).ok()?;
    let captures = matcher.captures(url).ok().flatten()?;
    Some((
        captures.get(1)?.as_str().parse().ok()?,
        captures.get(2)?.as_str().parse().ok()?,
        captures.get(3)?.as_str().parse().ok()?,
        captures.get(4)?.as_str().parse().ok()?,
    ))
}

fn hrfernsehen_meta(html: &str, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| html_meta_value(html, key))
}

fn hrfernsehen_capture(html: &str, pattern: &str) -> Option<String> {
    Regex::new(pattern)
        .ok()
        .and_then(|matcher| matcher.captures(html).ok().flatten())
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
}
