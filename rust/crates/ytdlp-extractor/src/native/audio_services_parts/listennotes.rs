// Native Listen Notes extraction is split into HTML attribute/metadata
// parsing, episode field mapping, and descriptor-facing extraction.
include!("listennotes_parts/metadata.rs");
include!("listennotes_parts/extractor.rs");
