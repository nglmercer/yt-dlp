// Native Loco extraction is split into token/playback transport, stream
// metadata and format mapping, and descriptor-facing extraction.
include!("loco_parts/api.rs");
include!("loco_parts/media.rs");
include!("loco_parts/extractor.rs");
