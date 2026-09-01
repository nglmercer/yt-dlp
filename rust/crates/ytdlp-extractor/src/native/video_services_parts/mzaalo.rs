// Native Mzaalo extraction is split into API/header transport, stream and
// metadata mapping, and descriptor-facing result assembly.
include!("mzaalo_parts/api.rs");
include!("mzaalo_parts/media.rs");
include!("mzaalo_parts/extractor.rs");
