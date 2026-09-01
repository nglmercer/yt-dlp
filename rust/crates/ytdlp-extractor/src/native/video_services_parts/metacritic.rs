// Native Metacritic extraction is split into page/HTTP access, malformed-XML
// normalization and decoding, and descriptor-facing result assembly.
include!("metacritic_parts/page.rs");
include!("metacritic_parts/xml.rs");
include!("metacritic_parts/extractor.rs");
