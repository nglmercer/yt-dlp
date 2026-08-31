/// Capabilities available in the first scaffold. No production feature is
/// claimed until it has a differential test against the offline reference.
pub const INITIAL_CAPABILITIES: &[Capability] = &[
    Capability {
        name: "info-dict-foundation",
        mode: EngineMode::Rust,
    },
    Capability {
        name: "format-bytes",
        mode: EngineMode::Rust,
    },
    Capability {
        name: "request-model",
        mode: EngineMode::Rust,
    },
    Capability {
        name: "request-director",
        mode: EngineMode::Rust,
    },
    Capability {
        name: "http-handler",
        mode: EngineMode::Rust,
    },
    Capability {
        name: "cookie-jar",
        mode: EngineMode::Rust,
    },
    Capability {
        name: "https-proxy-handler",
        mode: EngineMode::Rust,
    },
    Capability {
        name: "parse-duration",
        mode: EngineMode::Rust,
    },
    Capability {
        name: "core-url-and-scalar-utilities",
        mode: EngineMode::Rust,
    },
    Capability {
        name: "ffmpeg-postprocessor-contract",
        mode: EngineMode::Rust,
    },
    Capability {
        name: "javascript-runtime-adapter",
        mode: EngineMode::Rust,
    },
    Capability {
        name: "extractor-registry",
        mode: EngineMode::Rust,
    },
];
