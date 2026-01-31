//! Plugin error types

use thiserror::Error;

/// Result type for plugin operations
pub type PluginResult<T> = Result<T, PluginError>;

/// Errors that can occur in the plugin system
#[derive(Error, Debug)]
pub enum PluginError {
    /// Plugin not found
    #[error("Plugin not found: {0}")]
    NotFound(String),

    /// Plugin already loaded
    #[error("Plugin already loaded: {0}")]
    AlreadyLoaded(String),

    /// Invalid plugin manifest
    #[error("Invalid plugin manifest: {0}")]
    InvalidManifest(String),

    /// WASM compilation error
    #[error("WASM compilation error: {0}")]
    CompilationError(String),

    /// WASM instantiation error
    #[error("WASM instantiation error: {0}")]
    InstantiationError(String),

    /// Plugin execution error
    #[error("Plugin execution error: {0}")]
    ExecutionError(String),

    /// Plugin execution timeout
    #[error("Plugin execution timeout after {0}ms")]
    Timeout(u64),

    /// Memory limit exceeded
    #[error("Memory limit exceeded: {used} bytes > {limit} bytes")]
    MemoryLimitExceeded { used: u64, limit: u64 },

    /// Invalid capability
    #[error("Invalid capability: {0}")]
    InvalidCapability(String),

    /// Missing capability
    #[error("Missing required capability: {0}")]
    MissingCapability(String),

    /// Signature verification failed
    #[error("Signature verification failed: {0}")]
    SignatureError(String),

    /// Plugin signature required but not present
    #[error("Plugin signature required but not present")]
    SignatureRequired,

    /// Hash verification failed
    #[error("Hash verification failed: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },

    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// JSON serialization error
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// Wasmtime error
    #[error("Wasmtime error: {0}")]
    WasmtimeError(String),

    /// Host function error
    #[error("Host function error: {0}")]
    HostFunctionError(String),

    /// Plugin lifecycle error
    #[error("Lifecycle error: {0}")]
    LifecycleError(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Version incompatibility
    #[error("Version incompatibility: plugin requires {required}, client is {client}")]
    VersionIncompatible { required: String, client: String },

    /// Platform not supported
    #[error("Platform not supported: {0}")]
    PlatformNotSupported(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<wasmtime::Error> for PluginError {
    fn from(err: wasmtime::Error) -> Self {
        PluginError::WasmtimeError(err.to_string())
    }
}
