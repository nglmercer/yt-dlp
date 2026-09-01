//! Native extractor implementations grouped by protocol and site family.
//!
//! The files are included into this module intentionally: extractor helpers
//! are shared across families, and keeping one Rust namespace preserves the
//! existing contracts while the implementation is decomposed into reviewable
//! units. New ports should live in the narrowest matching file rather than
//! growing a second monolith.

include!("crypto.rs");
include!("general.rs");
include!("webcams.rs");
include!("broadcast.rs");
include!("sports.rs");
include!("web_platforms.rs");
include!("community.rs");
include!("media.rs");
include!("video_services.rs");
include!("audio_services.rs");
