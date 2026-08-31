//! Native download primitives for the Rust migration.
//!
//! This first slice handles a complete direct-resource transaction: request
//! construction is supplied by the caller, the native director performs the
//! HTTP exchange, and the response is written through a temporary sibling
//! file before being committed. Fragmented protocols and postprocessing will
//! HLS, DASH, and bounded fragment assembly build on this result contract.

use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};
use url::Url;
use yt_dlp_networking::{ErrorKind, Request, RequestDirector, RequestError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadOptions {
    pub simulate: bool,
    pub overwrite: bool,
    pub resume: bool,
    pub retries: usize,
    pub concurrent: usize,
}

impl Default for DownloadOptions {
    fn default() -> Self {
        Self {
            simulate: false,
            overwrite: true,
            resume: true,
            retries: 10,
            concurrent: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadResult {
    pub url: String,
    pub status: u16,
    pub bytes: usize,
    pub path: Option<PathBuf>,
    pub simulated: bool,
    pub fragments: Option<usize>,
    pub resumed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fragment {
    pub index: usize,
    pub request: Request,
}

#[derive(Debug)]
pub enum DownloadError {
    Request(RequestError),
    Io(io::Error),
    OutputExists(PathBuf),
    InvalidOutput(PathBuf),
    InvalidPlaylist(String),
    Unsupported(String),
}

impl std::fmt::Display for DownloadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Request(error) => error.fmt(formatter),
            Self::Io(error) => error.fmt(formatter),
            Self::OutputExists(path) => write!(formatter, "output already exists: {path:?}"),
            Self::InvalidOutput(path) => write!(formatter, "invalid output path: {path:?}"),
            Self::InvalidPlaylist(message) => write!(formatter, "invalid HLS playlist: {message}"),
            Self::Unsupported(message) => write!(formatter, "unsupported download: {message}"),
        }
    }
}

impl std::error::Error for DownloadError {}

impl From<RequestError> for DownloadError {
    fn from(error: RequestError) -> Self {
        Self::Request(error)
    }
}

impl From<io::Error> for DownloadError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub struct DirectDownloader {
    director: RequestDirector,
}

impl DirectDownloader {
    pub fn new(director: RequestDirector) -> Self {
        Self { director }
    }

    pub fn native() -> Self {
        Self::new(RequestDirector::native())
    }

    fn send_with_retries(
        &self,
        request: &Request,
        retries: usize,
    ) -> Result<yt_dlp_networking::Response, DownloadError> {
        let mut last_error = None;
        for attempt in 0..=retries {
            match self.director.send(request) {
                Ok(response) if response.status() >= 500 && attempt < retries => continue,
                Ok(response) => return Ok(response),
                Err(error) if attempt < retries => last_error = Some(error),
                Err(error) => return Err(error.into()),
            }
        }
        Err(DownloadError::Request(last_error.unwrap_or_else(|| {
            RequestError::new(ErrorKind::Transport, "download retry failed")
        })))
    }

    fn check_response(response: &yt_dlp_networking::Response) -> Result<(), DownloadError> {
        if response.status() >= 400 {
            return Err(DownloadError::Request(RequestError::new(
                ErrorKind::Http {
                    status: response.status(),
                    reason: response.reason().to_owned(),
                },
                format!("HTTP request failed with status {}", response.status()),
            )));
        }
        Ok(())
    }

    pub fn download(
        &self,
        request: &Request,
        output: Option<&Path>,
        options: &DownloadOptions,
    ) -> Result<DownloadResult, DownloadError> {
        let mut request = request.clone();
        let mut prefix = Vec::new();
        let mut resumed = false;
        if options.resume {
            if let Some(output) = output {
                if output.is_file() {
                    prefix = fs::read(output)?;
                    if !prefix.is_empty() {
                        request
                            .headers_mut()
                            .set("Range", format!("bytes={}-", prefix.len()));
                    }
                }
            }
        }
        let response = self.send_with_retries(&request, options.retries)?;
        Self::check_response(&response)?;
        let mut body = response.body().to_vec();
        if !prefix.is_empty() && response.status() == 206 {
            prefix.extend_from_slice(&body);
            body = prefix;
            resumed = true;
        }

        let path = if options.simulate {
            None
        } else if let Some(output) = output {
            Some(write_atomic(output, &body, options.overwrite || resumed)?)
        } else {
            None
        };

        Ok(DownloadResult {
            url: response.url().to_owned(),
            status: response.status(),
            bytes: body.len(),
            path,
            simulated: options.simulate,
            fragments: None,
            resumed,
        })
    }

    /// Download an HLS media playlist and concatenate its initialization and
    /// media segments in playlist order. Master playlists select their first
    /// variant until adaptive selection is added.
    pub fn download_hls(
        &self,
        request: &Request,
        output: Option<&Path>,
        options: &DownloadOptions,
    ) -> Result<DownloadResult, DownloadError> {
        let manifest = self.send_with_retries(request, options.retries)?;
        Self::check_response(&manifest)?;
        let playlist = parse_hls_playlist(request.url(), manifest.body())?;
        if let Some(variant) = playlist.variant {
            let mut variant_request = request.clone();
            variant_request.set_url(variant);
            variant_request.set_data(None);
            variant_request.set_method("GET")?;
            return self.download_hls(&variant_request, output, options);
        }

        let fragments = playlist
            .segments
            .iter()
            .enumerate()
            .map(|(index, segment)| {
                let mut segment_request = request.clone();
                segment_request.set_url(segment);
                segment_request.set_data(None);
                segment_request.set_method("GET")?;
                Ok(Fragment {
                    index,
                    request: segment_request,
                })
            })
            .collect::<Result<Vec<_>, DownloadError>>()?;
        let (status, body) = self.fetch_fragments(&fragments, options)?;
        let path = if options.simulate {
            None
        } else if let Some(output) = output {
            Some(write_atomic(output, &body, options.overwrite)?)
        } else {
            None
        };
        Ok(DownloadResult {
            url: request.url().to_owned(),
            status,
            bytes: body.len(),
            path,
            simulated: options.simulate,
            fragments: Some(fragments.len()),
            resumed: false,
        })
    }

    fn fetch_fragments(
        &self,
        fragments: &[Fragment],
        options: &DownloadOptions,
    ) -> Result<(u16, Vec<u8>), DownloadError> {
        if fragments.is_empty() {
            return Err(DownloadError::InvalidPlaylist(
                "no media fragments".to_owned(),
            ));
        }
        let worker_count = options.concurrent.max(1).min(fragments.len());
        let queue = Arc::new(Mutex::new(VecDeque::from(fragments.to_vec())));
        let results = Arc::new(Mutex::new(Vec::with_capacity(fragments.len())));
        std::thread::scope(|scope| {
            for _ in 0..worker_count {
                let queue = Arc::clone(&queue);
                let results = Arc::clone(&results);
                scope.spawn(move || {
                    loop {
                        let fragment = queue.lock().ok().and_then(|mut queue| queue.pop_front());
                        let Some(fragment) = fragment else {
                            break;
                        };
                        let result = self
                            .send_with_retries(&fragment.request, options.retries)
                            .and_then(|response| {
                                Self::check_response(&response)?;
                                Ok((fragment.index, response.status(), response.body().to_vec()))
                            });
                        let failed = result.is_err();
                        if let Ok(mut results) = results.lock() {
                            results.push(result);
                        }
                        if failed {
                            break;
                        }
                    }
                });
            }
        });

        let mut results = Arc::try_unwrap(results)
            .map_err(|_| DownloadError::InvalidPlaylist("fragment result lock busy".to_owned()))?
            .into_inner()
            .map_err(|_| {
                DownloadError::InvalidPlaylist("fragment result lock poisoned".to_owned())
            })?;
        if let Some(position) = results.iter().position(Result::is_err) {
            if let Err(error) = results.swap_remove(position) {
                return Err(error);
            }
        }
        results.sort_by_key(|result| result.as_ref().map_or(usize::MAX, |result| result.0));
        let status = results
            .first()
            .and_then(|result| result.as_ref().ok().map(|result| result.1))
            .unwrap_or(200);
        let mut body = Vec::new();
        for result in results {
            let (_, _, fragment_body) = result?;
            body.extend_from_slice(&fragment_body);
        }
        Ok((status, body))
    }

    /// Fetch an explicitly ordered fragment set and atomically assemble it.
    pub fn download_fragments(
        &self,
        fragments: &[Fragment],
        output: Option<&Path>,
        options: &DownloadOptions,
    ) -> Result<DownloadResult, DownloadError> {
        let (status, body) = self.fetch_fragments(fragments, options)?;
        let path = if options.simulate {
            None
        } else if let Some(output) = output {
            Some(write_atomic(output, &body, options.overwrite)?)
        } else {
            None
        };
        Ok(DownloadResult {
            url: fragments
                .first()
                .map(|fragment| fragment.request.url().to_owned())
                .unwrap_or_default(),
            status,
            bytes: body.len(),
            path,
            simulated: options.simulate,
            fragments: Some(fragments.len()),
            resumed: false,
        })
    }

    pub fn download_dash(
        &self,
        request: &Request,
        output: Option<&Path>,
        options: &DownloadOptions,
    ) -> Result<DownloadResult, DownloadError> {
        let manifest = self.send_with_retries(request, options.retries)?;
        Self::check_response(&manifest)?;
        let playlist = parse_dash_mpd(request.url(), manifest.body())?;
        let fragments = playlist
            .segments
            .iter()
            .enumerate()
            .map(|(index, segment)| {
                let mut segment_request = request.clone();
                segment_request.set_url(segment);
                segment_request.set_data(None);
                segment_request.set_method("GET")?;
                Ok(Fragment {
                    index,
                    request: segment_request,
                })
            })
            .collect::<Result<Vec<_>, DownloadError>>()?;
        let (status, body) = self.fetch_fragments(&fragments, options)?;
        let path = if options.simulate {
            None
        } else if let Some(output) = output {
            Some(write_atomic(output, &body, options.overwrite)?)
        } else {
            None
        };
        Ok(DownloadResult {
            url: request.url().to_owned(),
            status: manifest.status().max(status),
            bytes: body.len(),
            path,
            simulated: options.simulate,
            fragments: Some(fragments.len()),
            resumed: false,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HlsPlaylist {
    pub variant: Option<String>,
    pub segments: Vec<String>,
}

fn quoted_attribute(line: &str, name: &str) -> Option<String> {
    let attributes = line
        .split_once(':')
        .map_or(line, |(_, attributes)| attributes);
    let value = attributes
        .split(',')
        .find_map(|attribute| attribute.strip_prefix(&format!("{name}=")))?;
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or(Some(value))
        .map(str::to_owned)
}

pub fn parse_hls_playlist(base_url: &str, body: &[u8]) -> Result<HlsPlaylist, DownloadError> {
    let text = String::from_utf8_lossy(body);
    if !text.lines().any(|line| line.trim() == "#EXTM3U") {
        return Err(DownloadError::InvalidPlaylist(
            "missing #EXTM3U header".to_owned(),
        ));
    }
    let base = Url::parse(base_url).map_err(|error| {
        DownloadError::InvalidPlaylist(format!("invalid playlist URL: {error}"))
    })?;
    let mut variant_pending = false;
    let mut variant = None;
    let mut segments = Vec::new();
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if line.starts_with("#EXT-X-KEY:") {
            let method = quoted_attribute(line, "METHOD").unwrap_or_default();
            if !method.eq_ignore_ascii_case("NONE") {
                return Err(DownloadError::Unsupported(
                    "encrypted HLS segments are not implemented".to_owned(),
                ));
            }
            continue;
        }
        if line.starts_with("#EXT-X-BYTERANGE:") {
            return Err(DownloadError::Unsupported(
                "HLS byte ranges are not implemented".to_owned(),
            ));
        }
        if line == "#EXT-X-STREAM-INF:" || line.starts_with("#EXT-X-STREAM-INF:") {
            variant_pending = true;
            continue;
        }
        if line.starts_with('#') {
            if line.starts_with("#EXT-X-MAP:") {
                let uri = quoted_attribute(line, "URI").ok_or_else(|| {
                    DownloadError::InvalidPlaylist("#EXT-X-MAP has no URI".to_owned())
                })?;
                let url = base.join(&uri).map_err(|error| {
                    DownloadError::InvalidPlaylist(format!("invalid segment URL: {error}"))
                })?;
                segments.push(url.to_string());
            }
            continue;
        }

        let url = base.join(line).map_err(|error| {
            DownloadError::InvalidPlaylist(format!("invalid segment URL: {error}"))
        })?;
        if variant_pending && variant.is_none() {
            variant = Some(url.to_string());
            variant_pending = false;
        } else if !variant_pending {
            segments.push(url.to_string());
        }
    }
    if variant.is_none() && segments.is_empty() {
        return Err(DownloadError::InvalidPlaylist(
            "playlist contains no media segments".to_owned(),
        ));
    }
    Ok(HlsPlaylist { variant, segments })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashManifest {
    pub segments: Vec<String>,
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
                    if xml_attribute(&start, b"mediaRange").is_some() {
                        return Err(DownloadError::Unsupported(
                            "DASH byte ranges are not implemented".to_owned(),
                        ));
                    }
                    if let Some(media) = xml_attribute(&start, b"media") {
                        segments.push(resolve_url(bases.last().unwrap(), &media)?);
                    }
                } else if name == "Initialization" {
                    if xml_attribute(&start, b"range").is_some() {
                        return Err(DownloadError::Unsupported(
                            "DASH byte ranges are not implemented".to_owned(),
                        ));
                    }
                    if let Some(source) = xml_attribute(&start, b"sourceURL") {
                        let url = resolve_url(bases.last().unwrap(), &source)?;
                        if !segments.contains(&url) {
                            segments.insert(0, url);
                        }
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
                    if xml_attribute(&empty, b"mediaRange").is_some() {
                        return Err(DownloadError::Unsupported(
                            "DASH byte ranges are not implemented".to_owned(),
                        ));
                    }
                    if let Some(media) = xml_attribute(&empty, b"media") {
                        segments.push(resolve_url(&base, &media)?);
                    }
                } else if name == "Initialization" {
                    if xml_attribute(&empty, b"range").is_some() {
                        return Err(DownloadError::Unsupported(
                            "DASH byte ranges are not implemented".to_owned(),
                        ));
                    }
                    if let Some(source) = xml_attribute(&empty, b"sourceURL") {
                        let url = resolve_url(&base, &source)?;
                        if !segments.contains(&url) {
                            segments.insert(0, url);
                        }
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
        }
    }
    if segments.is_empty() {
        return Err(DownloadError::InvalidPlaylist(
            "MPD contains no media segments".to_owned(),
        ));
    }
    Ok(DashManifest { segments })
}

fn write_atomic(path: &Path, body: &[u8], overwrite: bool) -> Result<PathBuf, DownloadError> {
    if path.as_os_str().is_empty() || path == Path::new("-") {
        return Err(DownloadError::InvalidOutput(path.to_owned()));
    }
    if path.exists() && !overwrite {
        return Err(DownloadError::OutputExists(path.to_owned()));
    }
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }

    let temporary = path.with_extension(format!(
        "{}.part",
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("download")
    ));
    let mut file = File::create(&temporary)?;
    file.write_all(body)?;
    file.sync_all()?;
    drop(file);
    if overwrite && path.exists() {
        fs::remove_file(path)?;
    }
    fs::rename(&temporary, path)?;
    Ok(path.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn direct_downloader_writes_response_atomically() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 1024];
            let _ = std::io::Read::read(&mut stream, &mut request);
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\nConnection: close\r\n\r\ncontent",
                )
                .unwrap();
        });

        let output = std::env::temp_dir().join(format!(
            "yt-dlp-rs-download-{}-{}.bin",
            std::process::id(),
            address.port()
        ));
        let result = DirectDownloader::native()
            .download(
                &Request::new(format!("http://{address}/media.bin")),
                Some(&output),
                &DownloadOptions::default(),
            )
            .unwrap();

        assert_eq!(result.status, 200);
        assert_eq!(result.bytes, 7);
        assert_eq!(result.path.as_deref(), Some(output.as_path()));
        assert_eq!(fs::read(&output).unwrap(), b"content");
        assert!(!output.with_extension("bin.part").exists());
        fs::remove_file(output).unwrap();
        server.join().unwrap();
    }

    #[test]
    fn direct_downloader_resumes_existing_file_with_range_request() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 2048];
            let count = std::io::Read::read(&mut stream, &mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..count]);
            assert!(request.contains("Range: bytes=4-\r\n"));
            stream
                .write_all(
                    b"HTTP/1.1 206 Partial Content\r\nContent-Length: 4\r\nContent-Range: bytes 4-7/8\r\nConnection: close\r\n\r\nrest",
                )
                .unwrap();
        });

        let output = std::env::temp_dir().join(format!(
            "yt-dlp-rs-resume-{}-{}.bin",
            std::process::id(),
            address.port()
        ));
        fs::write(&output, b"part").unwrap();
        let result = DirectDownloader::native()
            .download(
                &Request::new(format!("http://{address}/media.bin")),
                Some(&output),
                &DownloadOptions {
                    simulate: false,
                    overwrite: false,
                    resume: true,
                    retries: 0,
                    concurrent: 1,
                },
            )
            .unwrap();

        assert!(result.resumed);
        assert_eq!(result.bytes, 8);
        assert_eq!(fs::read(&output).unwrap(), b"partrest");
        fs::remove_file(output).unwrap();
        server.join().unwrap();
    }

    #[test]
    fn simulated_download_does_not_create_output() {
        let output =
            std::env::temp_dir().join(format!("yt-dlp-rs-simulated-{}.bin", std::process::id()));
        let error = write_atomic(&output, b"body", false).unwrap();
        assert_eq!(error, output);
        fs::remove_file(&output).unwrap();

        let options = DownloadOptions {
            simulate: true,
            overwrite: false,
            resume: true,
            retries: 0,
            concurrent: 1,
        };
        assert!(options.simulate);
    }

    #[test]
    fn parses_hls_media_and_master_playlists() {
        let media = parse_hls_playlist(
            "http://example.test/video/playlist.m3u8",
            b"#EXTM3U\n#EXT-X-MAP:URI=\"init.mp4\"\n#EXTINF:1,\npart.ts\n",
        )
        .unwrap();
        assert_eq!(media.variant, None);
        assert_eq!(media.segments.len(), 2);
        assert_eq!(media.segments[0], "http://example.test/video/init.mp4");
        assert_eq!(media.segments[1], "http://example.test/video/part.ts");

        let master = parse_hls_playlist(
            "http://example.test/master.m3u8",
            b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1\nvideo/low.m3u8\n",
        )
        .unwrap();
        assert_eq!(
            master.variant.as_deref(),
            Some("http://example.test/video/low.m3u8")
        );
        assert!(master.segments.is_empty());

        assert!(matches!(
            parse_hls_playlist(
                "http://example.test/encrypted.m3u8",
                b"#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"key\"\npart.ts\n",
            ),
            Err(DownloadError::Unsupported(_))
        ));
    }

    #[test]
    fn parses_dash_segment_lists_with_base_url_scope() {
        let manifest = parse_dash_mpd(
            "http://example.test/manifests/main.mpd",
            br#"<MPD><Period><AdaptationSet><Representation>
                <BaseURL>video/</BaseURL>
                <SegmentList>
                    <Initialization sourceURL="init.mp4" />
                    <SegmentURL media="one.m4s" />
                    <SegmentURL media="two.m4s" />
                </SegmentList>
            </Representation></AdaptationSet></Period></MPD>"#,
        )
        .unwrap();
        assert_eq!(
            manifest.segments,
            [
                "http://example.test/manifests/video/init.mp4",
                "http://example.test/manifests/video/one.m4s",
                "http://example.test/manifests/video/two.m4s",
            ]
        );

        let timeline = parse_dash_mpd(
            "http://example.test/main.mpd",
            br#"<MPD><Period><Representation id="v1">
                <BaseURL>video/</BaseURL>
                <SegmentTemplate timescale="1" media="seg-$Number%02d$.m4s" initialization="init.mp4">
                    <SegmentTimeline><S t="0" d="2" r="1" /></SegmentTimeline>
                </SegmentTemplate>
            </Representation></Period></MPD>"#,
        )
        .unwrap();
        assert_eq!(
            timeline.segments,
            [
                "http://example.test/video/init.mp4",
                "http://example.test/video/seg-01.m4s",
                "http://example.test/video/seg-02.m4s",
            ]
        );

        let duration = parse_dash_mpd(
            "http://example.test/main.mpd",
            br#"<MPD mediaPresentationDuration="PT5S"><Period><Representation>
                <SegmentTemplate duration="2" media="seg-$Number$.m4s" />
            </Representation></Period></MPD>"#,
        )
        .unwrap();
        assert_eq!(duration.segments.len(), 3);
    }

    #[test]
    fn hls_downloader_concatenates_initialization_and_media_segments() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            for _ in 0..4 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0; 2048];
                let count = std::io::Read::read(&mut stream, &mut request).unwrap();
                let request = String::from_utf8_lossy(&request[..count]);
                let path = request.split_whitespace().nth(1).unwrap_or_default();
                let body = match path {
                    "/playlist.m3u8" => {
                        b"#EXTM3U\n#EXT-X-MAP:URI=\"init.mp4\"\n#EXTINF:1,\npart1.m4s\n#EXTINF:1,\npart2.m4s\n".to_vec()
                    }
                    "/init.mp4" => b"INIT".to_vec(),
                    "/part1.m4s" => b"ONE".to_vec(),
                    "/part2.m4s" => b"TWO".to_vec(),
                    _ => Vec::new(),
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
                stream.write_all(&body).unwrap();
            }
        });

        let output = std::env::temp_dir().join(format!(
            "yt-dlp-rs-hls-{}-{}.mp4",
            std::process::id(),
            address.port()
        ));
        let result = DirectDownloader::native()
            .download_hls(
                &Request::new(format!("http://{address}/playlist.m3u8")),
                Some(&output),
                &DownloadOptions {
                    simulate: false,
                    overwrite: true,
                    resume: false,
                    retries: 0,
                    concurrent: 1,
                },
            )
            .unwrap();

        assert_eq!(result.status, 200);
        assert_eq!(result.fragments, Some(3));
        assert_eq!(result.bytes, 10);
        assert_eq!(fs::read(&output).unwrap(), b"INITONETWO");
        fs::remove_file(output).unwrap();
        server.join().unwrap();
    }

    #[test]
    fn dash_downloader_concatenates_segment_list() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            for _ in 0..4 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0; 2048];
                let count = std::io::Read::read(&mut stream, &mut request).unwrap();
                let request = String::from_utf8_lossy(&request[..count]);
                let path = request.split_whitespace().nth(1).unwrap_or_default();
                let body = match path {
                    "/main.mpd" => br#"<MPD><Period><Representation>
                        <BaseURL>video/</BaseURL><SegmentList>
                        <Initialization sourceURL="init.mp4" />
                        <SegmentURL media="one.m4s" /><SegmentURL media="two.m4s" />
                        </SegmentList></Representation></Period></MPD>"#
                        .to_vec(),
                    "/video/init.mp4" => b"INIT".to_vec(),
                    "/video/one.m4s" => b"ONE".to_vec(),
                    "/video/two.m4s" => b"TWO".to_vec(),
                    _ => Vec::new(),
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
                stream.write_all(&body).unwrap();
            }
        });

        let output = std::env::temp_dir().join(format!(
            "yt-dlp-rs-dash-{}-{}.mp4",
            std::process::id(),
            address.port()
        ));
        let result = DirectDownloader::native()
            .download_dash(
                &Request::new(format!("http://{address}/main.mpd")),
                Some(&output),
                &DownloadOptions {
                    simulate: false,
                    overwrite: true,
                    resume: false,
                    retries: 0,
                    concurrent: 1,
                },
            )
            .unwrap();

        assert_eq!(result.status, 200);
        assert_eq!(result.fragments, Some(3));
        assert_eq!(result.bytes, 10);
        assert_eq!(fs::read(&output).unwrap(), b"INITONETWO");
        fs::remove_file(output).unwrap();
        server.join().unwrap();
    }

    #[test]
    fn fragment_downloader_limits_workers_and_restores_playlist_order() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            for _ in 0..3 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0; 2048];
                let count = std::io::Read::read(&mut stream, &mut request).unwrap();
                let request = String::from_utf8_lossy(&request[..count]);
                let body = match request.split_whitespace().nth(1).unwrap_or_default() {
                    "/zero" => b"ZERO".to_vec(),
                    "/one" => b"ONE".to_vec(),
                    "/two" => b"TWO".to_vec(),
                    _ => Vec::new(),
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
                stream.write_all(&body).unwrap();
            }
        });

        let fragments = ["zero", "one", "two"]
            .into_iter()
            .enumerate()
            .map(|(index, name)| Fragment {
                index,
                request: Request::new(format!("http://{address}/{name}")),
            })
            .collect::<Vec<_>>();
        let output = std::env::temp_dir().join(format!(
            "yt-dlp-rs-fragments-{}-{}.bin",
            std::process::id(),
            address.port()
        ));
        let result = DirectDownloader::native()
            .download_fragments(
                &fragments,
                Some(&output),
                &DownloadOptions {
                    simulate: false,
                    overwrite: true,
                    resume: false,
                    retries: 0,
                    concurrent: 2,
                },
            )
            .unwrap();

        assert_eq!(result.fragments, Some(3));
        assert_eq!(fs::read(&output).unwrap(), b"ZEROONETWO");
        fs::remove_file(output).unwrap();
        server.join().unwrap();
    }
}
