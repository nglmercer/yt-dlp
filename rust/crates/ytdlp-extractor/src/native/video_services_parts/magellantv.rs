// Native MagellanTV extraction is split into Next.js state traversal, media
// format construction, and descriptor-facing metadata assembly.
include!("magellantv_parts/state.rs");
include!("magellantv_parts/media.rs");
include!("magellantv_parts/extractor.rs");
