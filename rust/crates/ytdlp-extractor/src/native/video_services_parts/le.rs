// Native Le.com extraction is split into legacy API transport, encryption and
// format mapping, and descriptor-facing video/playlist implementations.
include!("le_parts/api.rs");
include!("le_parts/media.rs");
include!("le_parts/extractor.rs");
