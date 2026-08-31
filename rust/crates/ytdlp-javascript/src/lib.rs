//! JavaScript runtime adapters used by the native extractor runtime boundary.
//!
//! The adapter owns executable discovery, version probing, stdin/stdout
//! execution, and the QuickJS temporary-file difference.  Challenge-solving
//! scripts remain an independent input so the runtime layer can be tested
//! without bundling a particular EJS release into the Rust binary.

use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

include!("javascript_parts/model.rs");
include!("javascript_parts/runtime.rs");
include!("javascript_parts/discovery.rs");
include!("javascript_parts/tests.rs");
