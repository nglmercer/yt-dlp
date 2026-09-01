// Native ManyVids extraction is split into API transport, format mapping, and
// descriptor-facing metadata assembly.
include!("manyvids_parts/api.rs");
include!("manyvids_parts/media.rs");
include!("manyvids_parts/extractor.rs");
