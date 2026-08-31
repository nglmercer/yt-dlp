/// Native Archive.org metadata extractor. Archive items are represented from
/// the public metadata JSON, with files sharing their 'original' name grouped
/// into one entry and multiple media entries returned as a native playlist.
pub struct ArchiveOrgExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl ArchiveOrgExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for ArchiveOrgExtractor {
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
                "Archive.org URL did not match its native pattern",
            )
        })?;
        let requested_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Archive.org URL has no ID")
            })?;
        let requested_id = decode_url_component(requested_id);
        let (requested_identifier, requested_entry) = requested_id
            .split_once('/')
            .map_or((requested_id.clone(), None), |(identifier, entry)| {
                (identifier.to_owned(), Some(entry.to_owned()))
            });
        let metadata = context.get_json(&format!(
            "https://archive.org/metadata/{requested_identifier}"
        ))?;
        let metadata_info = metadata.get("metadata").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Archive.org metadata has no metadata object",
            )
        })?;
        let identifier = json_string(metadata_info, "identifier")
            .unwrap_or(requested_identifier.as_str())
            .to_owned();

        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(identifier));
        info.insert(
            "webpage_url",
            serde_json::json!(format!("https://archive.org/details/{identifier}")),
        );
        info.insert_if_some("title", archive_text_value(metadata_info.get("title")));
        info.insert_if_some(
            "description",
            archive_text_value(metadata_info.get("description")),
        );
        info.insert_if_some(
            "uploader",
            archive_text_value(
                metadata_info
                    .get("uploader")
                    .or_else(|| metadata_info.get("adder")),
            ),
        );
        info.insert_if_some("license", json_string(metadata_info, "licenseurl"));
        info.insert_if_some("location", json_string(metadata_info, "venue"));
        info.insert_if_some("release_year", json_i64(metadata_info, "year"));
        info.insert_if_some("release_date", json_string(metadata_info, "date"));
        if let Some(value) = metadata_info.get("creator") {
            info.insert("creators", value.clone());
        }

        let files = metadata
            .get("files")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Archive.org metadata has no files array",
                )
            })?;
        let mut entries = Vec::<InfoDict>::new();
        for file in files {
            if json_string(file, "format") == Some("Thumbnail") {
                continue;
            }
            let Some(name) = json_string(file, "name") else {
                continue;
            };
            let Some(extension) = archive_file_extension(name) else {
                continue;
            };
            let group = json_string(file, "original").unwrap_or(name);
            if let Some(requested_entry) = requested_entry.as_deref()
                && requested_entry != name
                && requested_entry != group
            {
                continue;
            }
            let entry_index = entries
                .iter()
                .position(|entry| entry.get_str("_archive_group") == Some(group))
                .unwrap_or_else(|| {
                    let mut entry = InfoDict::new();
                    entry.insert("_archive_group", serde_json::json!(group));
                    entry.insert("id", serde_json::json!(format!("{identifier}/{group}")));
                    entry.insert("display_id", serde_json::json!(group));
                    entry.insert(
                        "title",
                        serde_json::json!(json_string(file, "title").unwrap_or(group)),
                    );
                    entry.insert("formats", serde_json::json!([]));
                    entries.push(entry);
                    entries.len() - 1
                });
            let entry = &mut entries[entry_index];
            if let Some(value) = json_string(file, "description") {
                if !entry.contains_key("description") {
                    entry.insert("description", serde_json::json!(value));
                }
            }
            if let Some(value) = json_string(file, "creator") {
                if !entry.contains_key("creators") {
                    entry.insert("creators", serde_json::json!([value]));
                }
            }
            entry.insert_if_some(
                "duration",
                json_f64(file, "length")
                    .or_else(|| json_string(file, "length").and_then(yt_dlp_core::parse_duration)),
            );
            entry.insert_if_some("track_number", json_i64(file, "track"));
            entry.insert_if_some("album", json_string(file, "album"));
            entry.insert_if_some("discnumber", json_i64(file, "disc"));
            let file_url = archive_download_url(&identifier, name);
            let format = serde_json::json!({
                "url": file_url,
                "format": file.get("format").cloned().unwrap_or(serde_json::Value::Null),
                "ext": extension,
                "width": json_i64(file, "width"),
                "height": json_i64(file, "height"),
                "filesize": json_i64(file, "size"),
                "protocol": "https",
                "format_note": file.get("source").cloned().unwrap_or(serde_json::Value::Null),
            });
            let mut formats = entry
                .remove("formats")
                .and_then(|value| value.as_array().cloned())
                .unwrap_or_default();
            formats.push(format);
            entry.insert("formats", serde_json::Value::Array(formats));
            if !entry.contains_key("url") {
                entry.insert("url", serde_json::json!(file_url));
                entry.insert("ext", serde_json::json!(extension));
            }
        }
        for entry in &mut entries {
            entry.remove("_archive_group");
        }
        if entries.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Archive.org item {identifier} has no playable media files"),
            ));
        }

        if let Some(requested_entry) = requested_entry.as_deref() {
            let selected = entries
                .into_iter()
                .find(|entry| entry.get_str("display_id") == Some(requested_entry))
                .ok_or_else(|| {
                    ExtractorError::new(
                        ExtractorErrorKind::Extraction,
                        format!("Archive.org item has no requested file {requested_entry}"),
                    )
                })?;
            let mut merged = info;
            for (key, value) in selected.iter() {
                merged.insert(key, value.clone());
            }
            return Ok(ExtractorResult::single(merged));
        }
        if entries.len() == 1 {
            let selected = entries.pop().expect("one Archive.org entry");
            let mut merged = info;
            for (key, value) in selected.iter() {
                merged.insert(key, value.clone());
            }
            return Ok(ExtractorResult::single(merged));
        }
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

fn google_drive_mime_extension(mime_type: Option<&str>) -> Option<&'static str> {
    match mime_type {
        Some("video/mp4") => Some("mp4"),
        Some("video/webm") => Some("webm"),
        Some("video/ogg") => Some("ogv"),
        Some("audio/mpeg") => Some("mp3"),
        Some("audio/mp4") => Some("m4a"),
        Some("audio/webm") => Some("webm"),
        Some("audio/ogg") => Some("ogg"),
        Some("audio/flac") => Some("flac"),
        _ => None,
    }
}

fn google_drive_filename(content_disposition: Option<&str>) -> Option<String> {
    let matcher = Regex::new(r#"(?i)\bfilename\s*=\s*(?:["']([^"']+)["']|([^;\s]+))"#).ok()?;
    let captures = matcher.captures(content_disposition?).ok().flatten()?;
    captures
        .get(1)
        .or_else(|| captures.get(2))
        .map(|value| value.as_str().to_owned())
}
