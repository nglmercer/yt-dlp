/// Native Clipchamp Next.js/Cloudflare Stream extractor.
pub struct ClipchampExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl ClipchampExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for ClipchampExtractor {
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
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Clipchamp URL has no video ID",
                )
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let next_data = html_script_json(&webpage, "__NEXT_DATA__")?;
        let video = next_data
            .get("props")
            .and_then(|value| value.get("pageProps"))
            .and_then(|value| value.get("video"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Clipchamp video {video_id} has no Next.js video data"),
                )
            })?;
        if json_string(video, "storage_location") != Some("cf_stream") {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!(
                    "TODO: Clipchamp native extractor does not implement storage location {}",
                    json_string(video, "storage_location").unwrap_or("unknown")
                ),
            ));
        }
        let stream_path = json_string(video, "download_url")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Clipchamp video {video_id} has no Cloudflare Stream path"),
                )
            })?;
        let iframe_url = format!("https://iframe.cloudflarestream.com/{stream_path}");
        let iframe_response = context.get(&iframe_url)?;
        let iframe = String::from_utf8_lossy(iframe_response.body());
        let subdomain = Regex::new(
            r#"(?is)\bcustomer-domain-prefix\s*=\s*["']([\w-]+)["']"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&iframe).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().to_owned())
        .unwrap_or_else(|| "customer-2ut9yn3y6fta1yxe".to_owned());
        let manifest_base = format!(
            "https://{subdomain}.cloudflarestream.com/{stream_path}/manifest/video"
        );
        let dash_url = format!("{manifest_base}.mpd?parentOrigin=https%3A%2F%2Fclipchamp.com");
        let hls_url = format!("{manifest_base}.m3u8?parentOrigin=https%3A%2F%2Fclipchamp.com");
        let formats = serde_json::json!([
            {
                "format_id": "dash",
                "url": dash_url,
                "ext": "mp4",
                "protocol": "http_dash_segments",
            },
            {
                "format_id": "hls",
                "url": hls_url,
                "ext": "mp4",
                "protocol": "m3u8_native",
            },
        ]);
        let title = video
            .get("project")
            .and_then(|project| json_string(project, "project_name"))
            .filter(|value| !value.is_empty())
            .unwrap_or(&video_id)
            .to_owned();
        let uploader = video
            .get("creator")
            .map(|creator| {
                let name = ["first_name", "last_name"]
                    .iter()
                    .filter_map(|key| json_string(creator, key).filter(|value| !value.is_empty()))
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
                    .join(" ");
                (!name.is_empty()).then_some(name)
            })
            .flatten();
        let first_url = formats
            .as_array()
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("url"))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Clipchamp video {video_id} has no generated manifest URL"),
                )
            })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("url", serde_json::json!(first_url));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert_if_some("uploader", uploader);
        info.insert_if_some(
            "timestamp",
            json_string(video, "created_at")
                .map(str::to_owned)
                .and_then(parse_timestamp),
        );
        info.insert_if_some("thumbnail", json_string(video, "thumbnail_url"));
        info.insert("formats", formats);
        Ok(ExtractorResult::single(info))
    }
}
