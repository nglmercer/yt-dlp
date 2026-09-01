/// Native Kuwo album playlist extractor.
pub struct KuwoAlbumExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl KuwoAlbumExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for KuwoAlbumExtractor {
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
        let album_id = kuwo_match_id(&self.matcher, url, "album")?;
        let (webpage, _) = kuwo_page(context, url, "album detail")?;
        let matcher = Regex::new(
            r#"(?is)<div[^>]+class\s*=\s*["']comm["'][^<]*<h1[^>]+title\s*=\s*["']([^"']+)["']"#,
        )
        .map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Kuwo album title matcher: {error}"),
            )
        })?;
        let album_name = matcher
            .captures(&webpage)
            .ok()
            .flatten()
            .and_then(|captures| captures.get(1))
            .map(|value| html_text_fragment(value.as_str()))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Kuwo album {album_id} has no album name"),
                )
            })?;
        let link_matcher = Regex::new(
            r#"(?is)<p[^>]+class\s*=\s*["']listen["'][^>]*>\s*<a[^>]+href\s*=\s*["']http://www\.kuwo\.cn/yinyue/(\d+)/["']"#,
        )
        .map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Kuwo album entry matcher: {error}"),
            )
        })?;
        let entries = link_matcher
            .captures_iter(&webpage)
            .flatten()
            .filter_map(|captures| captures.get(1))
            .map(|value| kuwo_entry(&kuwo_song_url(value.as_str())))
            .collect::<Vec<_>>();
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(album_id));
        info.insert("title", serde_json::json!(album_name.clone()));
        info.insert_if_some("description", kuwo_intro(&webpage, &album_name));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}
