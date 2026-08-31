//! Foundational types for the experimental Rust migration.
//!
//! This crate deliberately starts with a dynamic info dictionary. Extractors
//! add service-specific fields, so a fixed Rust struct would lose information
//! and break behavioral parity.

use indexmap::IndexMap;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::sync::LazyLock;

mod archive;

pub use archive::{ArchiveError, DownloadArchive};

pub const MIGRATION_VERSION: &str = "0.0.0";

include!("core_parts/constants.rs");
include!("core_parts/model.rs");
include!("core_parts/templates.rs");
include!("core_parts/numbers.rs");
include!("core_parts/protocol.rs");
include!("core_parts/capabilities.rs");
include!("core_parts/tests.rs");
