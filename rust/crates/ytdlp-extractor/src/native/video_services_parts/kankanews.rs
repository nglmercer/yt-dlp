// Native KankaNews extraction is split into signing, page parsing, and API
// mapping so the legacy signature does not grow the service implementation.
include!("kankanews_parts/signing.rs");
include!("kankanews_parts/page.rs");
include!("kankanews_parts/api.rs");
