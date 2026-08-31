#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashManifest {
    pub segments: Vec<String>,
    pub segment_ranges: Vec<Option<ByteRange>>,
}

#[derive(Debug, Clone, Default)]
struct DashTimelineEntry {
    time: Option<u64>,
    duration: u64,
    repeat: i64,
}

#[derive(Debug, Clone)]
struct DashSegmentTemplate {
    base: Url,
    media: String,
    initialization: Option<String>,
    timescale: u64,
    duration: Option<u64>,
    start_number: u64,
    representation_id: String,
    timeline: Vec<DashTimelineEntry>,
}

fn xml_local_name(name: &[u8]) -> String {
    String::from_utf8_lossy(name).rsplit_once(':').map_or_else(
        || String::from_utf8_lossy(name).into_owned(),
        |(_, name)| name.to_owned(),
    )
}

fn xml_attribute(start: &BytesStart<'_>, name: &[u8]) -> Option<String> {
    start
        .attributes()
        .flatten()
        .find(|attribute| attribute.key.as_ref() == name)
        .and_then(|attribute| attribute.normalized_value(XmlVersion::Implicit1_0).ok())
        .map(|value| value.into_owned())
}

fn resolve_url(base: &Url, target: &str) -> Result<String, DownloadError> {
    base.join(target.trim())
        .map(|url| url.to_string())
        .map_err(|error| DownloadError::InvalidPlaylist(format!("invalid segment URL: {error}")))
}

fn replace_dash_token(template: &str, token: &str, value: u64) -> String {
    let mut result = template.replace(&format!("${token}$"), &value.to_string());
    let prefix = format!("${token}%");
    let mut search_from = 0;
    while let Some(relative_start) = result[search_from..].find(&prefix) {
        let start = search_from + relative_start;
        let Some(relative_end) = result[start + prefix.len()..].find('$') else {
            break;
        };
        let end = start + prefix.len() + relative_end;
        let spec = &result[start + prefix.len()..end];
        let Some(width) = spec
            .strip_prefix('0')
            .and_then(|spec| spec.strip_suffix('d'))
            .and_then(|width| width.parse::<usize>().ok())
        else {
            search_from = end + 1;
            continue;
        };
        let replacement = format!("{value:0width$}");
        result.replace_range(start..=end, &replacement);
        search_from = start + replacement.len();
    }
    result
}

fn expand_dash_template(template: &str, number: u64, time: u64, representation_id: &str) -> String {
    replace_dash_token(
        &replace_dash_token(
            &template.replace("$RepresentationID$", representation_id),
            "Number",
            number,
        ),
        "Time",
        time,
    )
}

fn dash_template_from_attributes(
    start: &BytesStart<'_>,
    base: &Url,
    representation_id: &str,
) -> Result<DashSegmentTemplate, DownloadError> {
    let media = xml_attribute(start, b"media").ok_or_else(|| {
        DownloadError::InvalidPlaylist("DASH SegmentTemplate has no media attribute".to_owned())
    })?;
    let timescale = xml_attribute(start, b"timescale")
        .as_deref()
        .unwrap_or("1")
        .parse::<u64>()
        .map_err(|_| DownloadError::InvalidPlaylist("invalid DASH timescale".to_owned()))?;
    let duration = xml_attribute(start, b"duration")
        .map(|value| {
            value
                .parse::<u64>()
                .map_err(|_| DownloadError::InvalidPlaylist("invalid DASH duration".to_owned()))
        })
        .transpose()?;
    let start_number = xml_attribute(start, b"startNumber")
        .as_deref()
        .unwrap_or("1")
        .parse::<u64>()
        .map_err(|_| DownloadError::InvalidPlaylist("invalid DASH startNumber".to_owned()))?;
    Ok(DashSegmentTemplate {
        base: base.clone(),
        media,
        initialization: xml_attribute(start, b"initialization"),
        timescale,
        duration,
        start_number,
        representation_id: representation_id.to_owned(),
        timeline: Vec::new(),
    })
}

fn dash_timeline_entry(start: &BytesStart<'_>) -> Result<DashTimelineEntry, DownloadError> {
    let duration = xml_attribute(start, b"d")
        .ok_or_else(|| DownloadError::InvalidPlaylist("DASH S has no duration".to_owned()))?
        .parse::<u64>()
        .map_err(|_| DownloadError::InvalidPlaylist("invalid DASH S duration".to_owned()))?;
    let repeat = xml_attribute(start, b"r")
        .as_deref()
        .unwrap_or("0")
        .parse::<i64>()
        .map_err(|_| DownloadError::InvalidPlaylist("invalid DASH S repeat".to_owned()))?;
    let time = xml_attribute(start, b"t")
        .map(|value| {
            value
                .parse::<u64>()
                .map_err(|_| DownloadError::InvalidPlaylist("invalid DASH S time".to_owned()))
        })
        .transpose()?;
    Ok(DashTimelineEntry {
        time,
        duration,
        repeat,
    })
}

fn parse_dash_duration_seconds(value: &str) -> Option<f64> {
    yt_dlp_core::parse_duration(value)
}

fn parse_dash_byte_range(value: &str) -> Result<ByteRange, DownloadError> {
    let (start, end) = value.trim().split_once('-').ok_or_else(|| {
        DownloadError::InvalidPlaylist(format!("invalid DASH byte range: {value}"))
    })?;
    let start = start.parse::<u64>().map_err(|_| {
        DownloadError::InvalidPlaylist(format!("invalid DASH byte range start: {value}"))
    })?;
    let end = end.parse::<u64>().map_err(|_| {
        DownloadError::InvalidPlaylist(format!("invalid DASH byte range end: {value}"))
    })?;
    let length = end
        .checked_sub(start)
        .and_then(|length| length.checked_add(1))
        .ok_or_else(|| {
            DownloadError::InvalidPlaylist(format!("invalid DASH byte range bounds: {value}"))
        })?;
    Ok(ByteRange { start, length })
}

fn append_dash_segment(
    segments: &mut Vec<String>,
    segment_ranges: &mut Vec<Option<ByteRange>>,
    base: &Url,
    media: &str,
    range: Option<&str>,
) -> Result<(), DownloadError> {
    segments.push(resolve_url(base, media)?);
    segment_ranges.push(range.map(parse_dash_byte_range).transpose()?);
    Ok(())
}

fn insert_dash_initialization(
    segments: &mut Vec<String>,
    segment_ranges: &mut Vec<Option<ByteRange>>,
    base: &Url,
    source: &str,
    range: Option<&str>,
) -> Result<(), DownloadError> {
    let url = resolve_url(base, source)?;
    if !segments.contains(&url) {
        segments.insert(0, url);
        segment_ranges.insert(0, range.map(parse_dash_byte_range).transpose()?);
    }
    Ok(())
}

fn expand_dash_template_segments(
    template: DashSegmentTemplate,
    presentation_duration: Option<f64>,
) -> Result<Vec<String>, DownloadError> {
    let mut segments = Vec::new();
    if let Some(initialization) = template.initialization.as_deref() {
        let initialization = expand_dash_template(
            initialization,
            template.start_number,
            0,
            &template.representation_id,
        );
        segments.push(resolve_url(&template.base, &initialization)?);
    }

    let mut number = template.start_number;
    if !template.timeline.is_empty() {
        let mut current_time = 0;
        for (index, entry) in template.timeline.iter().enumerate() {
            if let Some(time) = entry.time {
                current_time = time;
            }
            if entry.duration == 0 {
                return Err(DownloadError::InvalidPlaylist(
                    "DASH timeline entry has zero duration".to_owned(),
                ));
            }
            let repeat = if entry.repeat >= 0 {
                entry.repeat as u64
            } else {
                let end_time = template
                    .timeline
                    .get(index + 1)
                    .and_then(|next| next.time)
                    .or_else(|| {
                        presentation_duration
                            .map(|duration| (duration * template.timescale as f64) as u64)
                    })
                    .unwrap_or(current_time + entry.duration);
                end_time
                    .saturating_sub(current_time)
                    .saturating_div(entry.duration)
                    .saturating_sub(1)
            };
            for _ in 0..=repeat {
                let media = expand_dash_template(
                    &template.media,
                    number,
                    current_time,
                    &template.representation_id,
                );
                segments.push(resolve_url(&template.base, &media)?);
                number = number.saturating_add(1);
                current_time = current_time.saturating_add(entry.duration);
            }
        }
    } else {
        let duration = template.duration.ok_or_else(|| {
            DownloadError::InvalidPlaylist(
                "DASH SegmentTemplate requires duration or SegmentTimeline".to_owned(),
            )
        })?;
        let presentation_duration = presentation_duration.ok_or_else(|| {
            DownloadError::InvalidPlaylist(
                "DASH SegmentTemplate without timeline requires mediaPresentationDuration"
                    .to_owned(),
            )
        })?;
        if duration == 0 {
            return Err(DownloadError::InvalidPlaylist(
                "DASH SegmentTemplate has zero duration".to_owned(),
            ));
        }
        let total_units = (presentation_duration * template.timescale as f64).ceil() as u64;
        let count = total_units.saturating_add(duration - 1) / duration;
        for index in 0..count {
            let time = index.saturating_mul(duration);
            let media =
                expand_dash_template(&template.media, number, time, &template.representation_id);
            segments.push(resolve_url(&template.base, &media)?);
            number = number.saturating_add(1);
        }
    }
    Ok(segments)
}

/// Parse a DASH MPD's `SegmentList` or `SegmentTemplate` representation while
/// preserving XML namespace prefixes and BaseURL scoping.
pub fn parse_dash_mpd(base_url: &str, body: &[u8]) -> Result<DashManifest, DownloadError> {
    let root = Url::parse(base_url)
        .map_err(|error| DownloadError::InvalidPlaylist(format!("invalid MPD URL: {error}")))?;
    let mut reader = Reader::from_reader(body);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut elements: Vec<String> = Vec::new();
    let mut bases = vec![root.clone()];
    let mut base_text = None;
    let mut segments = Vec::new();
    let mut segment_ranges = Vec::new();
    let mut template: Option<DashSegmentTemplate> = None;
    let mut representation_id = String::new();
    let mut presentation_duration = None;

    loop {
        buffer.clear();
        let event = reader
            .read_event_into(&mut buffer)
            .map_err(|error| DownloadError::InvalidPlaylist(format!("invalid MPD XML: {error}")))?;
        match event {
            Event::Start(start) => {
                let name = xml_local_name(start.name().as_ref());
                let parent = bases.last().cloned().unwrap_or_else(|| root.clone());
                elements.push(name.clone());
                bases.push(parent);
                if name == "MPD" {
                    presentation_duration = xml_attribute(&start, b"mediaPresentationDuration")
                        .and_then(|value| parse_dash_duration_seconds(&value));
                } else if name == "Representation" {
                    if representation_id.is_empty() {
                        representation_id = xml_attribute(&start, b"id").unwrap_or_default();
                        if let Some(template) = template.as_mut() {
                            template.representation_id = representation_id.clone();
                        }
                    }
                } else if name == "SegmentURL" {
                    let media_range = xml_attribute(&start, b"mediaRange");
                    if let Some(media) = xml_attribute(&start, b"media") {
                        append_dash_segment(
                            &mut segments,
                            &mut segment_ranges,
                            bases.last().unwrap(),
                            &media,
                            media_range.as_deref(),
                        )?;
                    }
                } else if name == "Initialization" {
                    let initialization_range = xml_attribute(&start, b"range");
                    if let Some(source) = xml_attribute(&start, b"sourceURL") {
                        insert_dash_initialization(
                            &mut segments,
                            &mut segment_ranges,
                            bases.last().unwrap(),
                            &source,
                            initialization_range.as_deref(),
                        )?;
                    }
                } else if name == "SegmentTemplate" {
                    template = Some(dash_template_from_attributes(
                        &start,
                        bases.last().unwrap(),
                        &representation_id,
                    )?);
                } else if name == "S" {
                    if let Some(template) = template.as_mut() {
                        template.timeline.push(dash_timeline_entry(&start)?);
                    }
                } else if name == "BaseURL" {
                    base_text = Some(String::new());
                }
            }
            Event::Empty(empty) => {
                let name = xml_local_name(empty.name().as_ref());
                let base = bases.last().cloned().unwrap_or_else(|| root.clone());
                if name == "SegmentURL" {
                    let media_range = xml_attribute(&empty, b"mediaRange");
                    if let Some(media) = xml_attribute(&empty, b"media") {
                        append_dash_segment(
                            &mut segments,
                            &mut segment_ranges,
                            &base,
                            &media,
                            media_range.as_deref(),
                        )?;
                    }
                } else if name == "Initialization" {
                    let initialization_range = xml_attribute(&empty, b"range");
                    if let Some(source) = xml_attribute(&empty, b"sourceURL") {
                        insert_dash_initialization(
                            &mut segments,
                            &mut segment_ranges,
                            &base,
                            &source,
                            initialization_range.as_deref(),
                        )?;
                    }
                } else if name == "SegmentTemplate" {
                    template = Some(dash_template_from_attributes(
                        &empty,
                        &base,
                        &representation_id,
                    )?);
                } else if name == "S" {
                    if let Some(template) = template.as_mut() {
                        template.timeline.push(dash_timeline_entry(&empty)?);
                    }
                }
            }
            Event::Text(text) if elements.last().is_some_and(|name| name == "BaseURL") => {
                let value = text.decode().map_err(|error| {
                    DownloadError::InvalidPlaylist(format!("invalid MPD text: {error}"))
                })?;
                base_text.get_or_insert_with(String::new).push_str(&value);
            }
            Event::End(end) => {
                let name = xml_local_name(end.name().as_ref());
                let _ = bases.pop();
                let _ = elements.pop();
                if name == "BaseURL" {
                    let parent = bases.last_mut().ok_or_else(|| {
                        DownloadError::InvalidPlaylist("unbalanced BaseURL".to_owned())
                    })?;
                    if let Some(value) = base_text.take().filter(|value| !value.trim().is_empty()) {
                        *parent =
                            Url::parse(&resolve_url(parent, value.trim())?).map_err(|error| {
                                DownloadError::InvalidPlaylist(format!("invalid BaseURL: {error}"))
                            })?;
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }

    if segments.is_empty() {
        if let Some(template) = template {
            segments = expand_dash_template_segments(template, presentation_duration)?;
            segment_ranges = vec![None; segments.len()];
        }
    }
    if segments.is_empty() {
        return Err(DownloadError::InvalidPlaylist(
            "MPD contains no media segments".to_owned(),
        ));
    }
    Ok(DashManifest {
        segments,
        segment_ranges,
    })
}
