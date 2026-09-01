// Native Lecturio extraction is split into API transport, media/caption
// mapping, and descriptor-facing lecture/course implementations.
include!("lecturio_parts/api.rs");
include!("lecturio_parts/media.rs");
include!("lecturio_parts/extractor.rs");
