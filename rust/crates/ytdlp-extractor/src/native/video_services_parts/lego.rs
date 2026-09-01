// Native LEGO extraction is split into API transport, format/subtitle mapping,
// and descriptor-facing extraction.
include!("lego_parts/api.rs");
include!("lego_parts/media.rs");
include!("lego_parts/extractor.rs");
