#[derive(Default)]
struct MagentaSmilMedia {
    src: String,
    proto: Option<String>,
    ext: Option<String>,
    streamer: Option<String>,
    bitrate: Option<f64>,
    filesize: Option<i64>,
    width: Option<i64>,
    height: Option<i64>,
}

fn magentamusik_xml_name(name: &[u8]) -> String {
    String::from_utf8_lossy(name)
        .rsplit_once(':')
        .map_or_else(|| String::from_utf8_lossy(name).into_owned(), |(_, name)| name.to_owned())
}

fn magentamusik_xml_attribute(
    start: &quick_xml::events::BytesStart<'_>,
    name: &[u8],
) -> Option<String> {
    start
        .attributes()
        .flatten()
        .find(|attribute| attribute.key.as_ref() == name)
        .and_then(|attribute| {
            attribute
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .ok()
        })
        .map(|value| value.into_owned())
}

fn magentamusik_smil_media(
    start: &quick_xml::events::BytesStart<'_>,
) -> Option<MagentaSmilMedia> {
    let src = magentamusik_xml_attribute(start, b"src")?;
    if src.trim().is_empty() {
        return None;
    }
    let bitrate = magentamusik_xml_attribute(start, b"system-bitrate")
        .or_else(|| magentamusik_xml_attribute(start, b"systemBitrate"))
        .and_then(|value| value.parse::<f64>().ok())
        .map(|value| value / 1000.0);
    Some(MagentaSmilMedia {
        src,
        proto: magentamusik_xml_attribute(start, b"proto"),
        ext: magentamusik_xml_attribute(start, b"ext"),
        streamer: magentamusik_xml_attribute(start, b"streamer"),
        bitrate,
        filesize: magentamusik_xml_attribute(start, b"size")
            .or_else(|| magentamusik_xml_attribute(start, b"fileSize"))
            .and_then(|value| value.parse().ok()),
        width: magentamusik_xml_attribute(start, b"width").and_then(|value| value.parse().ok()),
        height: magentamusik_xml_attribute(start, b"height").and_then(|value| value.parse().ok()),
    })
}

fn magentamusik_smil_format_id(
    protocol: &str,
    bitrate: Option<f64>,
    index: usize,
) -> String {
    let quality = bitrate.map_or_else(
        || index.to_string(),
        |value| (value as i64).to_string(),
    );
    match protocol {
        "m3u8_native" => format!("hls-{quality}"),
        "http_dash_segments" => format!("dash-{quality}"),
        "rtmp" => format!("rtmp-{quality}"),
        _ => format!("http-{quality}"),
    }
}

fn magentamusik_append_smil_format(
    formats: &mut Vec<serde_json::Value>,
    seen_sources: &mut Vec<String>,
    media: MagentaSmilMedia,
    base_url: &str,
) {
    let source = media.src.trim().to_owned();
    if source.is_empty() || seen_sources.iter().any(|value| value == &source) {
        return;
    }
    seen_sources.push(source.clone());
    let source_ext = yt_dlp_core::determine_ext(Some(&source), "unknown");
    let streamer = media
        .streamer
        .clone()
        .unwrap_or_else(|| base_url.to_owned());
    let is_rtmp = media.proto.as_deref() == Some("rtmp") || streamer.starts_with("rtmp");
    let source_url = resolve_url(base_url, &source).trim().to_owned();
    if is_rtmp {
        let mut format = serde_json::json!({
            "url": streamer,
            "play_path": source,
            "ext": "flv",
            "format_id": magentamusik_smil_format_id(
                "rtmp",
                media.bitrate,
                formats.len() + 1,
            ),
        });
        if let Some(tbr) = media.bitrate {
            format["tbr"] = serde_json::json!(tbr);
        }
        if let Some(filesize) = media.filesize {
            format["filesize"] = serde_json::json!(filesize);
        }
        if let Some(width) = media.width {
            format["width"] = serde_json::json!(width);
        }
        if let Some(height) = media.height {
            format["height"] = serde_json::json!(height);
        }
        formats.push(format);
        return;
    }
    if !source_url.starts_with("http://") && !source_url.starts_with("https://") {
        return;
    }
    let source_ext = if source_ext == "unknown" {
        media.ext.clone().unwrap_or_else(|| "mp4".to_owned())
    } else {
        source_ext
    };
    let lower_source_url = source_url.to_ascii_lowercase();
    let protocol = if media.proto.as_deref() == Some("m3u8")
        || source_ext.eq_ignore_ascii_case("m3u8")
    {
        "m3u8_native"
    } else if media.proto.as_deref() == Some("mpd") || source_ext.eq_ignore_ascii_case("mpd") {
        "http_dash_segments"
    } else if source_ext.eq_ignore_ascii_case("f4m") {
        "f4m"
    } else if lower_source_url.contains(".ism/manifest") {
        "mss"
    } else {
        "http"
    };
    let extension = if matches!(
        protocol,
        "m3u8_native" | "http_dash_segments" | "f4m" | "mss"
    ) {
        "mp4".to_owned()
    } else {
        media.ext.unwrap_or(source_ext)
    };
    let mut format = serde_json::json!({
        "url": source_url,
        "ext": extension,
        "format_id": magentamusik_smil_format_id(
            protocol,
            media.bitrate,
            formats.len() + 1,
        ),
    });
    if protocol != "http" {
        format["protocol"] = serde_json::json!(protocol);
    }
    if let Some(tbr) = media.bitrate {
        format["tbr"] = serde_json::json!(tbr);
    }
    if let Some(filesize) = media.filesize {
        format["filesize"] = serde_json::json!(filesize);
    }
    if let Some(width) = media.width {
        format["width"] = serde_json::json!(width);
    }
    if let Some(height) = media.height {
        format["height"] = serde_json::json!(height);
    }
    formats.push(format);
}

fn magentamusik_parse_smil(
    body: &[u8],
    smil_url: &str,
    video_id: &str,
) -> Result<Vec<serde_json::Value>, ExtractorError> {
    let mut reader = quick_xml::Reader::from_reader(body);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut base_url = smil_url.to_owned();
    let mut formats = Vec::new();
    let mut seen_sources = Vec::new();
    loop {
        buffer.clear();
        let event = reader.read_event_into(&mut buffer).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid MagentaMusik SMIL for {video_id}: {error}"),
            )
        })?;
        match event {
            quick_xml::events::Event::Start(start) | quick_xml::events::Event::Empty(start) => {
                let name = magentamusik_xml_name(start.name().as_ref());
                if name == "meta" {
                    let is_base = magentamusik_xml_attribute(&start, b"name")
                        .is_some_and(|value| value == "base" || value == "httpBase");
                    if is_base {
                        if let Some(value) = magentamusik_xml_attribute(&start, b"content") {
                            base_url = resolve_url(smil_url, value.trim());
                        }
                    }
                } else if matches!(name.as_str(), "video" | "audio" | "media") {
                    if let Some(media) = magentamusik_smil_media(&start) {
                        magentamusik_append_smil_format(
                            &mut formats,
                            &mut seen_sources,
                            media,
                            &base_url,
                        );
                    }
                }
            }
            quick_xml::events::Event::Eof => break,
            _ => {}
        }
    }
    if formats.is_empty() {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("MagentaMusik SMIL for {video_id} has no playable media"),
        ));
    }
    Ok(formats)
}
