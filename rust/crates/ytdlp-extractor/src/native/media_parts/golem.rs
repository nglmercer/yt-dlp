use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

/// Native Golem XML player/configuration extractor.
pub struct GolemExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl GolemExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for GolemExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Golem URL has no video ID")
            })?;
        let config_response = context.get(&format!("https://video.golem.de/xml/{video_id}.xml"))?;
        let config = parse_golem_config(config_response.body())?;
        if config.formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Golem video {video_id} has no playable formats"),
            ));
        }
        let formats = config
            .formats
            .iter()
            .map(|format| {
                let mut value = serde_json::json!({
                    "format_id": format.format_id,
                    "url": resolve_url("http://video.golem.de", &format.url),
                    "ext": format.extension,
                });
                if let Some(width) = format.width {
                    value["width"] = serde_json::json!(width);
                }
                if let Some(height) = format.height {
                    value["height"] = serde_json::json!(height);
                }
                if let Some(filesize) = format.filesize {
                    value["filesize"] = serde_json::json!(filesize);
                }
                value
            })
            .collect::<Vec<_>>();
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert(
            "title",
            serde_json::json!(config.title.unwrap_or_else(|| "golem".to_owned())),
        );
        info.insert_if_some("duration", config.duration);
        info.insert_if_some(
            "url",
            first.get("url").and_then(serde_json::Value::as_str),
        );
        info.insert_if_some("ext", first.get("ext").and_then(serde_json::Value::as_str));
        info.insert("formats", serde_json::Value::Array(formats));
        let thumbnails = config
            .thumbnails
            .into_iter()
            .map(|thumbnail| {
                let mut value = serde_json::json!({
                    "url": resolve_url("http://video.golem.de", &thumbnail.url),
                });
                if let Some(width) = thumbnail.width {
                    value["width"] = serde_json::json!(width);
                }
                if let Some(height) = thumbnail.height {
                    value["height"] = serde_json::json!(height);
                }
                value
            })
            .collect::<Vec<_>>();
        if !thumbnails.is_empty() {
            info.insert("thumbnails", serde_json::Value::Array(thumbnails));
        }
        Ok(ExtractorResult::single(info))
    }
}

#[derive(Default)]
struct GolemConfig {
    title: Option<String>,
    duration: Option<f64>,
    formats: Vec<GolemFormat>,
    thumbnails: Vec<GolemThumbnail>,
}

#[derive(Default)]
struct GolemFormat {
    format_id: String,
    url: String,
    filename: Option<String>,
    width: Option<i64>,
    height: Option<i64>,
    filesize: Option<i64>,
    extension: String,
}

#[derive(Default)]
struct GolemThumbnail {
    url: String,
    width: Option<i64>,
    height: Option<i64>,
}

fn parse_golem_config(body: &[u8]) -> Result<GolemConfig, ExtractorError> {
    let mut reader = Reader::from_reader(body);
    reader.config_mut().trim_text(false);
    let mut buffer = Vec::new();
    let mut elements = Vec::new();
    let mut config = GolemConfig::default();
    let mut current_format = None;
    let mut current_thumbnail = None;

    loop {
        buffer.clear();
        let event = reader.read_event_into(&mut buffer).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Golem XML configuration: {error}"),
            )
        })?;
        match event {
            Event::Start(start) => {
                let name = golem_xml_name(start.name().as_ref());
                if elements.len() == 1 {
                    if name == "teaser" {
                        current_thumbnail = Some(GolemThumbnail {
                            width: golem_xml_attribute(&start, b"width")
                                .and_then(|value| value.parse().ok()),
                            height: golem_xml_attribute(&start, b"height")
                                .and_then(|value| value.parse().ok()),
                            ..GolemThumbnail::default()
                        });
                    } else if !matches!(name.as_str(), "title" | "playtime") {
                        current_format = Some(GolemFormat {
                            format_id: name.clone(),
                            width: golem_xml_attribute(&start, b"width")
                                .and_then(|value| value.parse().ok()),
                            height: golem_xml_attribute(&start, b"height")
                                .and_then(|value| value.parse().ok()),
                            ..GolemFormat::default()
                        });
                    }
                }
                elements.push(name);
            }
            Event::Empty(start) => {
                if elements.len() == 1 && golem_xml_name(start.name().as_ref()) == "teaser" {
                    if let Some(url) = golem_xml_attribute(&start, b"url") {
                        config.thumbnails.push(GolemThumbnail {
                            url,
                            width: golem_xml_attribute(&start, b"width")
                                .and_then(|value| value.parse().ok()),
                            height: golem_xml_attribute(&start, b"height")
                                .and_then(|value| value.parse().ok()),
                        });
                    }
                }
            }
            Event::Text(text) => {
                let value = text
                    .xml_content(XmlVersion::Implicit1_0)
                    .map(|value| value.into_owned())
                    .unwrap_or_default();
                append_golem_text(
                    &mut config,
                    &mut current_format,
                    &mut current_thumbnail,
                    &elements,
                    &value,
                );
            }
            Event::GeneralRef(reference) => {
                let value = reference
                    .decode()
                    .ok()
                    .and_then(|value| {
                        let escaped = format!("&{value};");
                        quick_xml::escape::unescape(&escaped)
                            .ok()
                            .map(|value| value.into_owned())
                    })
                    .unwrap_or_default();
                append_golem_text(
                    &mut config,
                    &mut current_format,
                    &mut current_thumbnail,
                    &elements,
                    &value,
                );
            }
            Event::End(end) => {
                let name = golem_xml_name(end.name().as_ref());
                if elements.len() == 2 {
                    if let Some(mut format) = current_format.take() {
                        format.url = format.url.trim().to_owned();
                        format.filename = format
                            .filename
                            .map(|filename| filename.trim().to_owned());
                        if format.format_id == name && !format.url.is_empty() {
                            format.extension =
                                yt_dlp_core::determine_ext(format.filename.as_deref(), "unknown");
                            config.formats.push(format);
                        }
                    }
                    if let Some(thumbnail) = current_thumbnail.take() {
                        let mut thumbnail = thumbnail;
                        thumbnail.url = thumbnail.url.trim().to_owned();
                        if name == "teaser" && !thumbnail.url.is_empty() {
                            config.thumbnails.push(thumbnail);
                        }
                    }
                }
                elements.pop();
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(config)
}

fn append_golem_text(
    config: &mut GolemConfig,
    current_format: &mut Option<GolemFormat>,
    current_thumbnail: &mut Option<GolemThumbnail>,
    elements: &[String],
    value: &str,
) {
    match elements.last().map(String::as_str) {
        Some("title") if current_format.is_none() => {
            config
                .title
                .get_or_insert_with(String::new)
                .push_str(value);
        }
        Some("playtime") if current_format.is_none() => {
            config.duration = value.trim().parse().ok();
        }
        Some("url") => {
            if let Some(format) = current_format.as_mut() {
                format.url.push_str(value);
            } else if let Some(thumbnail) = current_thumbnail.as_mut() {
                thumbnail.url.push_str(value);
            }
        }
        Some("filename") => {
            if let Some(format) = current_format.as_mut() {
                format
                    .filename
                    .get_or_insert_with(String::new)
                    .push_str(value);
            }
        }
        Some("filesize") => {
            if let Some(format) = current_format.as_mut() {
                format.filesize = value.trim().parse().ok();
            }
        }
        _ => {}
    }
}

fn golem_xml_name(name: &[u8]) -> String {
    String::from_utf8_lossy(name)
        .rsplit_once(':')
        .map_or_else(|| String::from_utf8_lossy(name).into_owned(), |(_, name)| name.to_owned())
}

fn golem_xml_attribute(start: &BytesStart<'_>, name: &[u8]) -> Option<String> {
    start
        .attributes()
        .flatten()
        .find(|attribute| attribute.key.as_ref() == name)
        .and_then(|attribute| attribute.normalized_value(XmlVersion::Implicit1_0).ok())
        .map(|value| value.into_owned())
}
