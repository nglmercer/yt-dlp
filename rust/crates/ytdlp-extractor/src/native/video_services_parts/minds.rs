// Native Minds extraction is split into cookie-aware API transport, media
// mapping, and descriptor-facing video/feed implementations.
include!("minds_parts/api.rs");
include!("minds_parts/media.rs");
include!("minds_parts/extractor.rs");
