// Native Library of Congress extraction is split into page/API discovery,
// media format construction, and descriptor-facing extraction.
include!("libraryofcongress_parts/api.rs");
include!("libraryofcongress_parts/media.rs");
include!("libraryofcongress_parts/extractor.rs");
