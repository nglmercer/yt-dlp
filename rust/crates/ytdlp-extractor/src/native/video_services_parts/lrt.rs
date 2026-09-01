// Native LRT extraction is split into API transport, media mapping, and
// descriptor-facing implementations.
include!("lrt_parts/api.rs");
include!("lrt_parts/media.rs");
include!("lrt_parts/extractor.rs");
