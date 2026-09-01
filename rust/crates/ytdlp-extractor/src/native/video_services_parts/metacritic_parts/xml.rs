#[derive(Default)]
struct MetacriticClip {
    id: Option<String>,
    title: Option<String>,
    duration: Option<String>,
    files: Vec<MetacriticFile>,
}

#[derive(Default)]
struct MetacriticFile {
    rate: Option<String>,
    url: Option<String>,
}

fn metacritic_parse_xml(body: &[u8]) -> Result<Vec<MetacriticClip>, ExtractorError> {
    let fixed_body = metacritic_fix_xml_ampersands(body);
    let mut reader = quick_xml::Reader::from_reader(fixed_body.as_slice());
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut path = Vec::new();
    let mut clips = Vec::new();
    let mut clip = None;
    let mut file = None;

    loop {
        buffer.clear();
        let event = reader.read_event_into(&mut buffer).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Metacritic video XML: {error}"),
            )
        })?;
        match event {
            quick_xml::events::Event::Start(start) => {
                let name = String::from_utf8_lossy(start.name().as_ref()).into_owned();
                if name == "clip" {
                    clip = Some(MetacriticClip::default());
                } else if name == "videoFile" && clip.is_some() {
                    file = Some(MetacriticFile::default());
                }
                path.push(name);
            }
            quick_xml::events::Event::Text(text) => {
                let value = text
                    .xml_content(quick_xml::XmlVersion::Implicit1_0)
                    .map(|value| value.into_owned())
                    .unwrap_or_default();
                metacritic_append_text(&mut clip, &mut file, &path, &value);
            }
            quick_xml::events::Event::CData(text) => {
                let value = String::from_utf8_lossy(text.as_ref()).into_owned();
                metacritic_append_text(&mut clip, &mut file, &path, &value);
            }
            quick_xml::events::Event::GeneralRef(reference) => {
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
                metacritic_append_text(&mut clip, &mut file, &path, &value);
            }
            quick_xml::events::Event::End(end) => {
                let name = String::from_utf8_lossy(end.name().as_ref()).into_owned();
                if name == "videoFile" {
                    if let (Some(file), Some(clip)) = (file.take(), clip.as_mut()) {
                        clip.files.push(file);
                    }
                } else if name == "clip" {
                    if let Some(clip) = clip.take() {
                        clips.push(clip);
                    }
                }
                path.pop();
            }
            quick_xml::events::Event::Eof => break,
            _ => {}
        }
    }
    Ok(clips)
}

fn metacritic_append_text(
    clip: &mut Option<MetacriticClip>,
    file: &mut Option<MetacriticFile>,
    path: &[String],
    value: &str,
) {
    let Some(field) = path.last().map(String::as_str) else {
        return;
    };
    if let Some(file) = file.as_mut() {
        let target = match field {
            "rate" => &mut file.rate,
            "filePath" => &mut file.url,
            _ => return,
        };
        target.get_or_insert_with(String::new).push_str(value);
        return;
    }
    let Some(clip) = clip.as_mut() else {
        return;
    };
    let target = match field {
        "id" => &mut clip.id,
        "title" => &mut clip.title,
        "duration" => &mut clip.duration,
        _ => return,
    };
    target.get_or_insert_with(String::new).push_str(value);
}

fn metacritic_fix_xml_ampersands(body: &[u8]) -> Vec<u8> {
    let mut fixed = Vec::with_capacity(body.len());
    let mut offset = 0;
    while offset < body.len() {
        if body[offset] != b'&' {
            fixed.push(body[offset]);
            offset += 1;
            continue;
        }
        let remainder = &body[offset + 1..];
        let entity_length = remainder
            .iter()
            .position(|byte| *byte == b';')
            .filter(|length| *length <= 10);
        let is_entity = entity_length.is_some_and(|length| {
            let entity = &remainder[..length];
            matches!(entity, b"amp" | b"lt" | b"gt" | b"quot" | b"apos")
                || (entity.starts_with(b"#")
                    && entity[1..]
                        .iter()
                        .all(|byte| byte.is_ascii_digit() || *byte == b'x' || *byte == b'X'))
        });
        if is_entity {
            fixed.push(b'&');
        } else {
            fixed.extend_from_slice(b"&amp;");
        }
        offset += 1;
    }
    fixed
}
