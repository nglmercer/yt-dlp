/// Native AudioBoom HTML/API extractor. The page embeds the same clip store
/// used by the source implementation; Rust reads that JSON directly and
/// falls back to Open Graph/audio metadata when the player attributes change.
pub struct AudioBoomExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl AudioBoomExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for AudioBoomExtractor {
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
                "AudioBoom URL did not match its native pattern",
            )
        })?;
        let audio_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "AudioBoom URL has no ID")
            })?;
        let webpage = context.get(&format!("https://audioboom.com/posts/{audio_id}"))?;
        let html = String::from_utf8_lossy(webpage.body());
        let clip_store = audio_boom_clip_store(&html);
        let clip = clip_store
            .as_ref()
            .and_then(|store| store.get("clips"))
            .and_then(serde_json::Value::as_array)
            .and_then(|clips| clips.first());

        let media_url = clip
            .and_then(|clip| json_string(clip, "clipURLPriorToLoading"))
            .map(str::to_owned)
            .or_else(|| {
                html_meta_value(&html, "og:audio").map(|value| unescape_html_attribute(&value))
            })
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("AudioBoom page has no playable audio for {audio_id}"),
                )
            })?;
        let ext = yt_dlp_core::determine_ext(Some(&media_url), "mp3");
        let title = clip
            .and_then(|clip| json_string(clip, "title"))
            .map(str::to_owned)
            .or_else(|| {
                ["og:title", "og:audio:title", "audio_title"]
                    .iter()
                    .find_map(|key| html_meta_value(&html, key))
            })
            .unwrap_or_else(|| audio_id.to_owned());
        let description = clip
            .and_then(|clip| json_string(clip, "description"))
            .map(str::to_owned)
            .or_else(|| {
                clip.and_then(|clip| json_string(clip, "formattedDescription"))
                    .map(html_text_fragment)
            })
            .or_else(|| html_meta_value(&html, "og:description"));
        let duration = clip
            .and_then(|clip| json_f64(clip, "duration"))
            .or_else(|| {
                html_meta_value(&html, "weibo:audio:duration")
                    .and_then(|value| value.parse::<f64>().ok())
            });
        let uploader = clip
            .and_then(|clip| json_string(clip, "author"))
            .map(str::to_owned)
            .or_else(|| {
                [
                    "og:audio:artist",
                    "twitter:audio:artist_name",
                    "audio_artist",
                ]
                .iter()
                .find_map(|key| html_meta_value(&html, key))
            });
        let uploader_url = Regex::new(
            r#"(?is)<div\b[^>]*class\s*=\s*["'][^"']*\bavatar\b[^"']*["'][^>]*>.*?<a\b[^>]*href\s*=\s*["'](https?://[^"']+)"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&html).ok().flatten())
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()));

        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(audio_id));
        info.insert("title", serde_json::json!(title));
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!(ext.clone()));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "audio",
                "ext": ext,
                "vcodec": "none",
                "acodec": "mp3",
            }]),
        );
        info.insert_if_some("description", description);
        info.insert_if_some("duration", duration);
        info.insert_if_some("uploader", uploader);
        info.insert_if_some("uploader_url", uploader_url);
        Ok(ExtractorResult::single(info))
    }
}
