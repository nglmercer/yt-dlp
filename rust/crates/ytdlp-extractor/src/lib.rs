//! Ordered extractor registry for the Rust migration.
//!
//! Descriptors can be registered before their implementation is ported. Such
//! entries remain visible and return an explicit TODO error, which prevents a
//! missing extractor from being mistaken for a generic match.

mod common;
mod native;
mod registry;
#[cfg(test)]
mod tests;

pub use common::{
    DescriptorExtractor, ExtractionContext, ExtractorDescriptor, ExtractorError,
    ExtractorErrorKind, ExtractorResult, InfoExtractor,
};
pub use native::*;
pub use registry::ExtractorRegistry;
