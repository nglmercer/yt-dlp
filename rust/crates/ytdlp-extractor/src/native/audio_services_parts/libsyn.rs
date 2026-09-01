// Native Libsyn extraction is split into page metadata, media formats, and
// descriptor-facing extraction.
include!("libsyn_parts/metadata.rs");
include!("libsyn_parts/media.rs");
include!("libsyn_parts/extractor.rs");
