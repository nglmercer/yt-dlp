//! Native download primitives for the Rust migration.
//!
//! This crate handles direct resources, bounded fragment assembly, HLS, DASH,
//! resume, and atomic output commits without a compatibility runtime.

include!("downloader/mod.rs");
