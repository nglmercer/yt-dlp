// Native MBN extraction is split into page/API transport, authenticated HLS
// format mapping, and descriptor-facing metadata assembly.
include!("mbn_parts/api.rs");
include!("mbn_parts/media.rs");
include!("mbn_parts/extractor.rs");
