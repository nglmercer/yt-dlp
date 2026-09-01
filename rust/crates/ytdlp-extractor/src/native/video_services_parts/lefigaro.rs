// Native Le Figaro extraction is split into GraphQL transport, transparent
// JWPlatform mapping, and descriptor-facing implementations.
include!("lefigaro_parts/api.rs");
include!("lefigaro_parts/extractor.rs");
