// Native Markiza extraction is split into legacy API transport, JW-style
// source mapping, and descriptor-facing video/page playlist implementations.
include!("markiza_parts/api.rs");
include!("markiza_parts/media.rs");
include!("markiza_parts/extractor.rs");
