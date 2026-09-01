// Native Loom extraction is split into GraphQL/URL transport, media and
// metadata mapping, and descriptor-facing result construction.
include!("loom_parts/api.rs");
include!("loom_parts/media.rs");
include!("loom_parts/extractor.rs");
