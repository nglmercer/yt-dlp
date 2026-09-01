// Native Murrtube extraction is split into age/session setup, page/media
// discovery, and descriptor-facing metadata assembly. The non-working user
// profile descriptor remains an explicit registry TODO until its GraphQL
// pagination contract is ported.
include!("murrtube_parts/session.rs");
include!("murrtube_parts/media.rs");
include!("murrtube_parts/extractor.rs");
