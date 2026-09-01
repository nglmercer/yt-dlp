/// Native Fifth TV HTML/player URL extractor.
pub struct FiveTvExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FiveTvExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FiveTvExtractor {
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
                "FiveTV URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .or_else(|| captures.name("path"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "FiveTV URL has no video ID")
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let raw_media_url = [
            r#"(?is)<div\b[^>]*\bclass\s*=\s*["'][^"']*(?:flow)?player[^"']*["'][^>]*\bdata-href\s*=\s*["']([^"']+)"#,
            r#"(?is)<a\b[^>]*\bhref\s*=\s*["']([^"']+)["'][^>]*\bclass\s*=\s*["'][^"']*videoplayer[^"']*["']"#,
        ]
        .iter()
        .find_map(|pattern| {
            Regex::new(pattern)
                .ok()
                .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
                .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
        })
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("FiveTV video {video_id} has no player URL"),
            )
        })?;
        let media_url = resolve_url(url, &raw_media_url);
        let extension = yt_dlp_core::determine_ext(Some(&media_url), "mp4");
        let protocol = match extension.as_str() {
            "m3u8" => "m3u8_native",
            "mpd" => "http_dash_segments",
            _ => "http",
        };
        let duration = html_meta_value(&webpage, "video:duration")
            .and_then(|value| value.parse::<i64>().ok());
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert(
            "title",
            serde_json::json!(html_title_value(&webpage).unwrap_or_else(|| video_id.clone())),
        );
        info.insert_if_some(
            "description",
            html_meta_value(&webpage, "og:description"),
        );
        info.insert_if_some("thumbnail", html_meta_value(&webpage, "og:image"));
        info.insert_if_some("duration", duration);
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!(if extension == "mpd" || extension == "m3u8" {
            "mp4"
        } else {
            extension.as_str()
        }));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "http",
                "protocol": protocol,
                "ext": if extension == "mpd" || extension == "m3u8" { "mp4" } else { extension.as_str() },
            }]),
        );
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}
