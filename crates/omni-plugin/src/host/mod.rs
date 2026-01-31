//! Host functions exposed to WASM plugins
//!
//! These functions are callable from within the WASM sandbox and provide
//! controlled access to host capabilities like logging, configuration,
//! and state queries.

use crate::error::{PluginError, PluginResult};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::{debug, error, info, trace, warn};

/// Host state accessible to plugins
#[derive(Clone)]
pub struct HostState {
    /// Plugin ID for context
    pub plugin_id: String,
    /// Plugin configuration
    pub config: Arc<RwLock<HashMap<String, serde_json::Value>>>,
    /// Key-value store for plugin state
    pub kv_store: Arc<RwLock<HashMap<String, Vec<u8>>>>,
    /// Log buffer (for testing/debugging)
    pub log_buffer: Arc<RwLock<Vec<LogEntry>>>,
    /// HTTP allowlist (hosts the plugin can call)
    pub http_allowlist: Vec<String>,
}

impl Default for HostState {
    fn default() -> Self {
        Self {
            plugin_id: String::new(),
            config: Arc::new(RwLock::new(HashMap::new())),
            kv_store: Arc::new(RwLock::new(HashMap::new())),
            log_buffer: Arc::new(RwLock::new(Vec::new())),
            http_allowlist: Vec::new(),
        }
    }
}

impl HostState {
    /// Create a new host state for a plugin
    pub fn new(plugin_id: impl Into<String>) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            ..Default::default()
        }
    }

    /// Set plugin configuration
    pub fn with_config(mut self, config: HashMap<String, serde_json::Value>) -> Self {
        self.config = Arc::new(RwLock::new(config));
        self
    }

    /// Set HTTP allowlist
    pub fn with_http_allowlist(mut self, allowlist: Vec<String>) -> Self {
        self.http_allowlist = allowlist;
        self
    }
}

/// Log entry stored in the log buffer
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: u64,
    pub level: LogLevel,
    pub message: String,
    pub plugin_id: String,
}

/// Log level for host logging
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "TRACE"),
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
        }
    }
}

// ============================================================================
// Host Functions - Logging
// ============================================================================

/// Log a message from a plugin
pub fn host_log(state: &HostState, level: LogLevel, message: &str) {
    let plugin_id = &state.plugin_id;

    // Log to tracing
    match level {
        LogLevel::Trace => trace!(plugin_id = %plugin_id, "{}", message),
        LogLevel::Debug => debug!(plugin_id = %plugin_id, "{}", message),
        LogLevel::Info => info!(plugin_id = %plugin_id, "{}", message),
        LogLevel::Warn => warn!(plugin_id = %plugin_id, "{}", message),
        LogLevel::Error => error!(plugin_id = %plugin_id, "{}", message),
    }

    // Store in buffer
    if let Ok(mut buffer) = state.log_buffer.write() {
        let entry = LogEntry {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            level,
            message: message.to_string(),
            plugin_id: plugin_id.clone(),
        };
        buffer.push(entry);

        // Keep buffer bounded
        if buffer.len() > 1000 {
            buffer.remove(0);
        }
    }
}

/// Convenience function for trace logging
pub fn host_log_trace(state: &HostState, message: &str) {
    host_log(state, LogLevel::Trace, message);
}

/// Convenience function for debug logging
pub fn host_log_debug(state: &HostState, message: &str) {
    host_log(state, LogLevel::Debug, message);
}

/// Convenience function for info logging
pub fn host_log_info(state: &HostState, message: &str) {
    host_log(state, LogLevel::Info, message);
}

/// Convenience function for warn logging
pub fn host_log_warn(state: &HostState, message: &str) {
    host_log(state, LogLevel::Warn, message);
}

/// Convenience function for error logging
pub fn host_log_error(state: &HostState, message: &str) {
    host_log(state, LogLevel::Error, message);
}

// ============================================================================
// Host Functions - Configuration
// ============================================================================

/// Get a configuration value
pub fn host_config_get(state: &HostState, key: &str) -> Option<serde_json::Value> {
    state.config.read().ok()?.get(key).cloned()
}

/// Get a configuration value as a string
pub fn host_config_get_string(state: &HostState, key: &str) -> Option<String> {
    host_config_get(state, key).and_then(|v| v.as_str().map(|s| s.to_string()))
}

/// Get a configuration value as an integer
pub fn host_config_get_int(state: &HostState, key: &str) -> Option<i64> {
    host_config_get(state, key).and_then(|v| v.as_i64())
}

/// Get a configuration value as a float
pub fn host_config_get_float(state: &HostState, key: &str) -> Option<f64> {
    host_config_get(state, key).and_then(|v| v.as_f64())
}

/// Get a configuration value as a boolean
pub fn host_config_get_bool(state: &HostState, key: &str) -> Option<bool> {
    host_config_get(state, key).and_then(|v| v.as_bool())
}

/// Get all configuration keys
pub fn host_config_keys(state: &HostState) -> Vec<String> {
    state
        .config
        .read()
        .map(|c| c.keys().cloned().collect())
        .unwrap_or_default()
}

// ============================================================================
// Host Functions - Key-Value Store
// ============================================================================

/// Store a value in the plugin's key-value store
pub fn host_kv_set(state: &HostState, key: &str, value: &[u8]) -> PluginResult<()> {
    state
        .kv_store
        .write()
        .map_err(|e| PluginError::Internal(format!("KV store lock poisoned: {}", e)))?
        .insert(key.to_string(), value.to_vec());
    Ok(())
}

/// Get a value from the plugin's key-value store
pub fn host_kv_get(state: &HostState, key: &str) -> Option<Vec<u8>> {
    state.kv_store.read().ok()?.get(key).cloned()
}

/// Delete a value from the plugin's key-value store
pub fn host_kv_delete(state: &HostState, key: &str) -> bool {
    state
        .kv_store
        .write()
        .ok()
        .map(|mut store| store.remove(key).is_some())
        .unwrap_or(false)
}

/// List all keys in the plugin's key-value store
pub fn host_kv_keys(state: &HostState) -> Vec<String> {
    state
        .kv_store
        .read()
        .map(|store| store.keys().cloned().collect())
        .unwrap_or_default()
}

/// Clear all values in the plugin's key-value store
pub fn host_kv_clear(state: &HostState) -> PluginResult<()> {
    state
        .kv_store
        .write()
        .map_err(|e| PluginError::Internal(format!("KV store lock poisoned: {}", e)))?
        .clear();
    Ok(())
}

// ============================================================================
// Host Functions - Time
// ============================================================================

/// Get current timestamp in milliseconds since Unix epoch
pub fn host_time_now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Get current timestamp in nanoseconds since Unix epoch
pub fn host_time_now_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

// ============================================================================
// Host Functions - Random
// ============================================================================

/// Generate a random UUID v4
pub fn host_random_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Generate random bytes
pub fn host_random_bytes(count: usize) -> Vec<u8> {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};

    let mut result = Vec::with_capacity(count);
    let hasher_builder = RandomState::new();

    while result.len() < count {
        let mut hasher = hasher_builder.build_hasher();
        hasher.write_u64(host_time_now_ns());
        let hash = hasher.finish();
        result.extend_from_slice(&hash.to_le_bytes());
    }

    result.truncate(count);
    result
}

// ============================================================================
// Host Functions - HTTP (Restricted)
// ============================================================================

/// Check if a URL is allowed for HTTP requests
pub fn host_http_is_allowed(state: &HostState, url: &str) -> bool {
    if state.http_allowlist.is_empty() {
        return false;
    }

    // Parse the URL to get the host
    let host = url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or("");

    state.http_allowlist.iter().any(|allowed| {
        if allowed.starts_with("*.") {
            // Wildcard match
            let suffix = &allowed[1..]; // Remove *
            host.ends_with(suffix)
        } else {
            host == allowed
        }
    })
}

// ============================================================================
// WIT Interface Bindings (for wasmtime component model)
// ============================================================================

/// Host functions module for linking with WASM components
pub mod wit_bindings {
    use super::*;

    /// Logging interface functions
    pub mod logging {
        use super::*;

        pub fn log_trace(state: &HostState, message: &str) {
            host_log_trace(state, message);
        }

        pub fn log_debug(state: &HostState, message: &str) {
            host_log_debug(state, message);
        }

        pub fn log_info(state: &HostState, message: &str) {
            host_log_info(state, message);
        }

        pub fn log_warn(state: &HostState, message: &str) {
            host_log_warn(state, message);
        }

        pub fn log_error(state: &HostState, message: &str) {
            host_log_error(state, message);
        }
    }

    /// Config interface functions
    pub mod config {
        use super::*;

        pub fn get_string(state: &HostState, key: &str) -> Option<String> {
            host_config_get_string(state, key)
        }

        pub fn get_int(state: &HostState, key: &str) -> Option<i64> {
            host_config_get_int(state, key)
        }

        pub fn get_float(state: &HostState, key: &str) -> Option<f64> {
            host_config_get_float(state, key)
        }

        pub fn get_bool(state: &HostState, key: &str) -> Option<bool> {
            host_config_get_bool(state, key)
        }

        pub fn keys(state: &HostState) -> Vec<String> {
            host_config_keys(state)
        }
    }

    /// Key-value store interface functions
    pub mod kv {
        use super::*;

        pub fn set(state: &HostState, key: &str, value: &[u8]) -> Result<(), String> {
            host_kv_set(state, key, value).map_err(|e| e.to_string())
        }

        pub fn get(state: &HostState, key: &str) -> Option<Vec<u8>> {
            host_kv_get(state, key)
        }

        pub fn delete(state: &HostState, key: &str) -> bool {
            host_kv_delete(state, key)
        }

        pub fn keys(state: &HostState) -> Vec<String> {
            host_kv_keys(state)
        }
    }

    /// Time interface functions
    pub mod time {
        use super::*;

        pub fn now_ms() -> u64 {
            host_time_now_ms()
        }

        pub fn now_ns() -> u64 {
            host_time_now_ns()
        }
    }

    /// Random interface functions
    pub mod random {
        use super::*;

        pub fn uuid() -> String {
            host_random_uuid()
        }

        pub fn bytes(count: u32) -> Vec<u8> {
            host_random_bytes(count as usize)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logging() {
        let state = HostState::new("test-plugin");

        host_log_info(&state, "Test message");

        let buffer = state.log_buffer.read().unwrap();
        assert_eq!(buffer.len(), 1);
        assert_eq!(buffer[0].level, LogLevel::Info);
        assert_eq!(buffer[0].message, "Test message");
    }

    #[test]
    fn test_config() {
        let mut config = HashMap::new();
        config.insert("key1".to_string(), serde_json::json!("value1"));
        config.insert("key2".to_string(), serde_json::json!(42));

        let state = HostState::new("test-plugin").with_config(config);

        assert_eq!(
            host_config_get_string(&state, "key1"),
            Some("value1".to_string())
        );
        assert_eq!(host_config_get_int(&state, "key2"), Some(42));
        assert_eq!(host_config_get_string(&state, "nonexistent"), None);
    }

    #[test]
    fn test_kv_store() {
        let state = HostState::new("test-plugin");

        host_kv_set(&state, "key", b"value").unwrap();
        assert_eq!(host_kv_get(&state, "key"), Some(b"value".to_vec()));

        assert!(host_kv_delete(&state, "key"));
        assert_eq!(host_kv_get(&state, "key"), None);
    }

    #[test]
    fn test_http_allowlist() {
        let state = HostState::new("test-plugin").with_http_allowlist(vec![
            "api.example.com".to_string(),
            "*.slack.com".to_string(),
        ]);

        assert!(host_http_is_allowed(
            &state,
            "https://api.example.com/webhook"
        ));
        assert!(host_http_is_allowed(
            &state,
            "https://hooks.slack.com/services/xxx"
        ));
        assert!(!host_http_is_allowed(&state, "https://malicious.com/steal"));
    }
}
