// Native Medal.tv extraction is split into API transport, format/fallback
// mapping, and descriptor-facing metadata assembly.
include!("medaltv_parts/api.rs");
include!("medaltv_parts/media.rs");
include!("medaltv_parts/extractor.rs");
