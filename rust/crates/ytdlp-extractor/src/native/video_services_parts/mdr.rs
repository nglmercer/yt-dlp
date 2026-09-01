// Native MDR extraction is split into page/config discovery, XML decoding,
// media format construction, and descriptor-facing metadata assembly.
include!("mdr_parts/page.rs");
include!("mdr_parts/xml.rs");
include!("mdr_parts/media.rs");
include!("mdr_parts/extractor.rs");
