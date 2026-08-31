use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use crate::{InfoDict, str_or_none};

/// Errors raised while loading or updating a native download archive.
#[derive(Debug)]
pub enum ArchiveError {
    Io { path: PathBuf, source: io::Error },
}

impl std::fmt::Display for ArchiveError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(formatter, "download archive {path:?}: {source}")
            }
        }
    }
}

impl std::error::Error for ArchiveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
        }
    }
}

/// Persistent native equivalent of yt-dlp's download archive.
///
/// Archive records use the stable `<extractor-key> <video-id>` form. The
/// archive is deliberately independent from the CLI so embedding users can
/// apply the same skip/record semantics without loading Python state.
#[derive(Debug, Default)]
pub struct DownloadArchive {
    path: Option<PathBuf>,
    entries: HashSet<String>,
}

impl DownloadArchive {
    /// Create an archive that never reads or writes a file.
    pub fn disabled() -> Self {
        Self::default()
    }

    /// Load an archive file if configured. A missing file is treated as an
    /// empty archive and is created on the first successful record.
    pub fn open(path: Option<&Path>) -> Result<Self, ArchiveError> {
        let Some(path) = path else {
            return Ok(Self::disabled());
        };

        let mut entries = HashSet::new();
        match File::open(path) {
            Ok(file) => {
                for line in BufReader::new(file).lines() {
                    let line = line.map_err(|source| ArchiveError::Io {
                        path: path.to_owned(),
                        source,
                    })?;
                    let line = line.trim();
                    if !line.is_empty() {
                        entries.insert(line.to_owned());
                    }
                }
            }
            Err(source) if source.kind() == io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(ArchiveError::Io {
                    path: path.to_owned(),
                    source,
                });
            }
        }
        Ok(Self {
            path: Some(path.to_owned()),
            entries,
        })
    }

    pub fn is_enabled(&self) -> bool {
        self.path.is_some()
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Build the stable archive key used by yt-dlp for an info dictionary.
    pub fn id_for_info(&self, info: &InfoDict, fallback_extractor: Option<&str>) -> Option<String> {
        let video_id = str_or_none(info.get("id"), None)?;
        if video_id.is_empty() {
            return None;
        }
        let extractor = info
            .get_str("extractor_key")
            .or_else(|| info.get_str("ie_key"))
            .or(fallback_extractor)?;
        if extractor.is_empty() {
            return None;
        }
        Some(format!("{} {video_id}", extractor.to_ascii_lowercase()))
    }

    pub fn contains_info(&self, info: &InfoDict, fallback_extractor: Option<&str>) -> bool {
        self.id_for_info(info, fallback_extractor)
            .is_some_and(|id| self.entries.contains(&id))
    }

    pub fn contains_id(&self, id: &str) -> bool {
        self.entries.contains(id)
    }

    /// Record an item once. Returns `true` only when a new line was appended.
    pub fn record_info(
        &mut self,
        info: &InfoDict,
        fallback_extractor: Option<&str>,
    ) -> Result<bool, ArchiveError> {
        let Some(path) = self.path.as_deref() else {
            return Ok(false);
        };
        let Some(id) = self.id_for_info(info, fallback_extractor) else {
            return Ok(false);
        };
        if !self.entries.insert(id.clone()) {
            return Ok(false);
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|source| ArchiveError::Io {
                path: path.to_owned(),
                source,
            })?;
        if let Err(source) = writeln!(file, "{id}") {
            self.entries.remove(&id);
            return Err(ArchiveError::Io {
                path: path.to_owned(),
                source,
            });
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!("yt-dlp-rs-archive-{label}-{}", std::process::id()))
    }

    fn info(id: &str) -> InfoDict {
        let mut info = InfoDict::new();
        info.insert("id", json!(id));
        info.insert("extractor_key", json!("ExampleIE"));
        info
    }

    #[test]
    fn disabled_archive_does_not_persist_records() {
        let mut archive = DownloadArchive::disabled();
        let item = info("one");
        assert!(!archive.is_enabled());
        assert_eq!(
            archive.id_for_info(&item, None).as_deref(),
            Some("exampleie one")
        );
        assert!(!archive.contains_info(&item, None));
        assert!(!archive.record_info(&item, None).unwrap());
    }

    #[test]
    fn archive_loads_skips_and_appends_records() {
        let path = test_path("round-trip");
        let _ = std::fs::remove_file(&path);

        let mut archive = DownloadArchive::open(Some(&path)).unwrap();
        let first = info("one");
        let second = info("two");
        assert!(archive.record_info(&first, None).unwrap());
        assert!(!archive.record_info(&first, None).unwrap());
        assert!(archive.record_info(&second, None).unwrap());
        assert_eq!(archive.len(), 2);
        assert!(archive.contains_id("exampleie one"));

        let reloaded = DownloadArchive::open(Some(&path)).unwrap();
        assert!(reloaded.contains_info(&first, None));
        assert!(reloaded.contains_info(&second, None));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "exampleie one\nexampleie two\n"
        );
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn archive_uses_info_extractor_before_fallback() {
        let archive = DownloadArchive::disabled();
        let mut item = InfoDict::new();
        item.insert("id", json!(42));
        item.insert("ie_key", json!("PlaylistEntryIE"));
        assert_eq!(
            archive.id_for_info(&item, Some("FallbackIE")).as_deref(),
            Some("playlistentryie 42")
        );
    }
}
