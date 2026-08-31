//! Postprocessing contracts for the Rust migration.
//!
//! yt-dlp's postprocessors are stateful Python classes, but their observable
//! contract is small: receive an info dictionary containing `filepath`, run a
//! tool when needed, return the updated info dictionary, and identify files
//! that may be removed after a successful operation.  This crate establishes
//! that contract and provides the first safe FFmpeg subprocess integration.

use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use indexmap::IndexMap;
use serde_json::json;
use yt_dlp_core::InfoDict;

include!("postprocessor_parts/contracts.rs");
include!("postprocessor_parts/command.rs");
include!("postprocessor_parts/remux.rs");
include!("postprocessor_parts/audio.rs");
include!("postprocessor_parts/convert.rs");
include!("postprocessor_parts/helpers.rs");
include!("postprocessor_parts/tests.rs");
