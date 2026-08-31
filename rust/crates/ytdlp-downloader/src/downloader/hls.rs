#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ByteRange {
    pub start: u64,
    pub length: u64,
}

impl ByteRange {
    fn end_inclusive(&self) -> Result<u64, DownloadError> {
        self.start
            .checked_add(self.length.saturating_sub(1))
            .ok_or_else(|| DownloadError::InvalidPlaylist("byte range exceeds u64".to_owned()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HlsPlaylist {
    pub variant: Option<String>,
    pub segments: Vec<String>,
    pub segment_ranges: Vec<Option<ByteRange>>,
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

fn parse_hls_byte_range(value: &str) -> Result<(u64, Option<u64>), DownloadError> {
    let value = value.trim().trim_matches('"');
    let (length, offset) = value.split_once('@').unwrap_or((value, ""));
    let length = length.parse::<u64>().map_err(|_| {
        DownloadError::InvalidPlaylist(format!("invalid HLS byte-range length: {value}"))
    })?;
    if length == 0 {
        return Err(DownloadError::InvalidPlaylist(
            "HLS byte range has zero length".to_owned(),
        ));
    }
    let offset = (!offset.is_empty())
        .then(|| {
            offset.parse::<u64>().map_err(|_| {
                DownloadError::InvalidPlaylist(format!("invalid HLS byte-range offset: {value}"))
            })
        })
        .transpose()?;
    Ok((length, offset))
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
    let mut segment_ranges = Vec::new();
    let mut pending_range = None;
    let mut previous_range: Option<(String, u64)> = None;
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
            pending_range = Some(parse_hls_byte_range(
                line.split_once(':').map_or("", |(_, value)| value),
            )?);
            continue;
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
                let range = quoted_attribute(line, "BYTERANGE")
                    .map(|value| parse_hls_byte_range(&value))
                    .transpose()?
                    .map(|(length, offset)| ByteRange {
                        start: offset.unwrap_or(0),
                        length,
                    });
                segments.push(url.to_string());
                segment_ranges.push(range);
                previous_range = None;
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
            let range = pending_range
                .take()
                .map(|(length, offset)| {
                    let start = offset
                        .or_else(|| {
                            previous_range
                                .as_ref()
                                .filter(|(previous_url, _)| previous_url == &url.to_string())
                                .map(|(_, end)| *end)
                        })
                        .unwrap_or(0);
                    let end = start.checked_add(length).ok_or_else(|| {
                        DownloadError::InvalidPlaylist("HLS byte range exceeds u64".to_owned())
                    })?;
                    previous_range = Some((url.to_string(), end));
                    Ok::<ByteRange, DownloadError>(ByteRange { start, length })
                })
                .transpose()?;
            if range.is_none() {
                previous_range = None;
            }
            segments.push(url.to_string());
            segment_ranges.push(range);
        }
    }
    if variant.is_none() && segments.is_empty() {
        return Err(DownloadError::InvalidPlaylist(
            "playlist contains no media segments".to_owned(),
        ));
    }
    Ok(HlsPlaylist {
        variant,
        segments,
        segment_ranges,
    })
}
