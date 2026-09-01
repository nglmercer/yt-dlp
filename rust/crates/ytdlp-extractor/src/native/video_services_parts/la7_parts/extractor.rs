pub struct La7PodcastEpisodeExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

pub struct La7PodcastExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl La7PodcastEpisodeExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl La7PodcastExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

fn la7_descriptor_id(matcher: &Regex, url: &str) -> Result<String, ExtractorError> {
    matcher
        .captures(url)
        .ok()
        .flatten()
        .and_then(|captures| captures.name("id"))
        .map(|value| value.as_str().to_owned())
        .ok_or_else(|| ExtractorError::new(ExtractorErrorKind::InvalidUrl, "LA7 URL has no ID"))
}

impl InfoExtractor for La7PodcastEpisodeExtractor {
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
        let video_id = la7_descriptor_id(&self.matcher, url)?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        Ok(ExtractorResult::single(la7_podcast_info(
            &webpage,
            url,
            Some(&video_id),
            None,
        )?))
    }
}

impl InfoExtractor for La7PodcastExtractor {
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
        let playlist_id = la7_descriptor_id(&self.matcher, url)?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let ppn = la7_first_capture(
            &webpage,
            &[r#"(?is)window\.ppN\s*=\s*[\"']([^\"']+)[\"']"#],
        );
        let episode_matcher = Regex::new(
            r#"(?is)<div[^>]+\bclass\s*=\s*[\"'][^\"']*\bcontainer-podcast-property\b[^\"']*[\"'][^>]*>(.*?)(?:</div>\s*){3}"#,
        )
        .map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid LA7 podcast episode matcher: {error}"),
            )
        })?;
        let mut entries = Vec::new();
        for captures in episode_matcher.captures_iter(&webpage).flatten() {
            let Some(fragment) = captures.get(1).map(|value| value.as_str()) else {
                continue;
            };
            entries.push(la7_podcast_info(fragment, url, None, ppn.as_deref())?);
        }
        let title = la7_first_fragment(&webpage, &[r#"(?is)<h1\b[^>]*>(.*?)</h1>"#])
            .or_else(|| html_meta_value(&webpage, "og:title"));
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(playlist_id));
        info.insert_if_some("title", title);
        Ok(ExtractorResult::Playlist { info, entries })
    }
}
