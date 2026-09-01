/// Native Mx3 track extractor shared by mx3.ch, neo.mx3.ch, and
/// volksmusik.mx3.ch.
pub struct Mx3Extractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
    domain: &'static str,
}

impl Mx3Extractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        let domain = match descriptor.key.as_str() {
            "Mx3NeoIE" => "neo.mx3.ch",
            "Mx3VolksmusikIE" => "volksmusik.mx3.ch",
            _ => "mx3.ch",
        };
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
            domain,
        })
    }
}

impl InfoExtractor for Mx3Extractor {
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
        let track_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Mx3 URL has no track ID")
            })?;
        let webpage_response = context.get(url)?;
        let webpage = String::from_utf8_lossy(webpage_response.body());
        let more_info = mx3_class_fragment(&webpage, "single-more-info").unwrap_or_default();
        let data = context.get_json(&format!("https://{}/t/{track_id}.json", self.domain))?;
        let formats = mx3_extract_formats(context, self.domain, &track_id);

        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(track_id));
        info.insert("formats", serde_json::Value::Array(formats.clone()));
        info.insert_if_some("genre", mx3_genre(&webpage));
        info.insert_if_some(
            "release_year",
            mx3_info_field(&more_info, "Year of creation")
                .and_then(|value| value.parse::<i64>().ok()),
        );
        info.insert_if_some("description", mx3_info_field(&more_info, "Description"));
        info.insert_if_some("tags", mx3_tags(&more_info));
        info.insert_if_some("title", json_string(&data, "title"));
        info.insert_if_some(
            "artist",
            json_string(&data, "performer_name").or_else(|| json_string(&data, "artist")),
        );
        info.insert_if_some("album_artist", json_string(&data, "artist"));
        info.insert_if_some("composer", json_string(&data, "composer_name"));
        info.insert_if_some(
            "thumbnail",
            json_string(&data, "picture_url_xlarge")
                .or_else(|| json_string(&data, "picture_url")),
        );
        if let Some(first) = formats.first() {
            info.insert_if_some("url", first.get("url").cloned());
            info.insert_if_some("ext", first.get("ext").cloned());
        }
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}
