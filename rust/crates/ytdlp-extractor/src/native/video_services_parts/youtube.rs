// Native YouTube extraction is deliberately split by responsibility. The
// source extractor is a large state machine; these parts keep the Rust port
// reviewable while preserving a single native extractor contract.
include!("youtube_parts/url.rs");
include!("youtube_parts/json.rs");
include!("youtube_parts/api.rs");
include!("youtube_parts/media.rs");
include!("youtube_parts/extractor.rs");
