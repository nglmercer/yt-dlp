// Native Mx3 track extraction is split into HEAD/media probing, HTML metadata,
// and one descriptor implementation shared by the three Mx3 domains.
include!("mx3_parts/media.rs");
include!("mx3_parts/metadata.rs");
include!("mx3_parts/extractor.rs");
