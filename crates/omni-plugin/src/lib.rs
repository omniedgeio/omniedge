//! # OmniEdge Plugin System
//!
//! This crate provides a WebAssembly-based plugin system for OmniEdge,
//! enabling dynamic extensibility for authentication, event handling,
//! network policies, robotics data management, and UI widgets.
//!
//! ## Architecture
//!
//! The plugin system is designed with the following principles:
//!
//! - **OmniNervous stays plugin-free**: Core VPN transport remains auditable and deterministic
//! - **WASM for safety**: Memory-safe sandboxing with capability-based access control
//! - **Event-driven architecture**: Plugins react to VPN lifecycle events
//! - **Hot-reload support**: Dynamic loading without VPN restart
//! - **Cross-platform**: WASM runs on Windows, macOS, Linux
//!
//! ## Plugin Categories
//!
//! 1. **Event Hooks** - React to VPN lifecycle events
//! 2. **Authentication Providers** - Custom SSO, enterprise identity
//! 3. **Network Policy Engines** - Automatic network/exit node selection
//! 4. **Data Triage** - High-bandwidth sensor data buffering (robotics)
//! 5. **QoS Enforcement** - Traffic prioritization, DSCP tagging
//! 6. **PdM Reporting** - Predictive maintenance for actuators
//! 7. **Compliance/FL** - Privacy compliance, federated learning
//!
//! ## Example Usage
//!
//! ```rust,ignore
//! use omni_plugin::{PluginManager, PluginConfig};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let config = PluginConfig::default();
//!     let mut manager = PluginManager::new(config)?;
//!     
//!     // Discover and load plugins
//!     manager.discover_plugins().await?;
//!     
//!     // Emit an event to all plugins
//!     manager.emit_event(Event::StateChange { ... });
//!     
//!     Ok(())
//! }
//! ```

pub mod error;
pub mod host;
pub mod loader;
pub mod manager;
pub mod manifest;
pub mod registry;
pub mod runtime;
pub mod sandbox;
pub mod traits;
pub mod types;

#[cfg(feature = "widgets")]
pub mod widget;

// Re-export main types
pub use error::{PluginError, PluginResult};
pub use manager::PluginManager;
pub use manifest::PluginManifest;
pub use registry::PluginRegistry;
pub use types::*;

// Re-export traits
pub use traits::{
    AuthPlugin, CompliancePlugin, DataTriagePlugin, EventPlugin, OmniEdgePlugin, PdMPlugin,
    PolicyPlugin, QoSPlugin,
};

// Re-export runtime
pub use runtime::PluginRuntimeManager;

/// Plugin system configuration
#[derive(Debug, Clone)]
pub struct PluginConfig {
    /// Base directory for plugin storage (~/.omniedge)
    pub data_dir: std::path::PathBuf,

    /// Maximum memory per plugin (bytes)
    pub max_memory: u64,

    /// Maximum execution time per callback (milliseconds)
    pub max_execution_time_ms: u64,

    /// Whether to require plugin signatures
    pub require_signatures: bool,

    /// Trusted signers (public keys)
    pub trusted_signers: Vec<String>,
}

impl Default for PluginConfig {
    fn default() -> Self {
        let data_dir = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".omniedge");

        Self {
            data_dir,
            max_memory: 64 * 1024 * 1024, // 64MB
            max_execution_time_ms: 100,
            require_signatures: false,
            trusted_signers: Vec::new(),
        }
    }
}

impl PluginConfig {
    /// Get the plugins installation directory
    pub fn plugins_dir(&self) -> std::path::PathBuf {
        self.data_dir.join("plugins").join("installed")
    }

    /// Get the widgets directory
    pub fn widgets_dir(&self) -> std::path::PathBuf {
        self.data_dir.join("plugins").join("widgets")
    }

    /// Get the plugin data directory
    pub fn plugin_data_dir(&self, plugin_slug: &str) -> std::path::PathBuf {
        self.data_dir.join("plugins").join("data").join(plugin_slug)
    }

    /// Get the plugin cache directory
    pub fn cache_dir(&self) -> std::path::PathBuf {
        self.data_dir.join("cache").join("plugins")
    }

    /// Get the plugin configuration file path
    pub fn config_file(&self) -> std::path::PathBuf {
        self.data_dir.join("config").join("plugins.json")
    }
}
