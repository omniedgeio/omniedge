//! Plugin manifest definition
//!
//! The manifest describes plugin metadata, capabilities, and requirements.

use crate::types::Capability;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Plugin manifest - describes a plugin's metadata and requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Unique plugin identifier (e.g., "com.omniedge.slack-notifier")
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Plugin version (semver)
    pub version: String,

    /// Plugin author/vendor
    pub author: String,

    /// Short description
    pub description: String,

    /// Plugin homepage URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,

    /// License identifier (SPDX)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,

    /// Required capabilities
    pub capabilities: Vec<Capability>,

    /// Minimum OmniEdge version required
    pub min_omniedge_version: String,

    /// Maximum OmniEdge version supported (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_omniedge_version: Option<String>,

    /// Supported platforms (empty = all platforms)
    #[serde(default)]
    pub platforms: Vec<Platform>,

    /// Plugin configuration schema (JSON Schema)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_schema: Option<serde_json::Value>,

    /// Default configuration values
    #[serde(default)]
    pub default_config: HashMap<String, serde_json::Value>,

    /// Entry points for different plugin types
    #[serde(default)]
    pub entry_points: EntryPoints,

    /// Plugin icon (base64 encoded or URL)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,

    /// Tags for discovery
    #[serde(default)]
    pub tags: Vec<String>,

    /// Dependencies on other plugins
    #[serde(default)]
    pub dependencies: Vec<PluginDependency>,
}

impl PluginManifest {
    /// Create a new plugin manifest with minimal required fields
    pub fn new(id: impl Into<String>, name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            version: version.into(),
            author: String::new(),
            description: String::new(),
            homepage: None,
            license: None,
            capabilities: Vec::new(),
            min_omniedge_version: "0.1.0".to_string(),
            max_omniedge_version: None,
            platforms: Vec::new(),
            config_schema: None,
            default_config: HashMap::new(),
            entry_points: EntryPoints::default(),
            icon: None,
            tags: Vec::new(),
            dependencies: Vec::new(),
        }
    }

    /// Validate the manifest
    pub fn validate(&self) -> Result<(), ManifestValidationError> {
        // Check required fields
        if self.id.is_empty() {
            return Err(ManifestValidationError::MissingField("id".to_string()));
        }
        if self.name.is_empty() {
            return Err(ManifestValidationError::MissingField("name".to_string()));
        }
        if self.version.is_empty() {
            return Err(ManifestValidationError::MissingField("version".to_string()));
        }

        // Validate version format (semver)
        if !is_valid_semver(&self.version) {
            return Err(ManifestValidationError::InvalidVersion(
                self.version.clone(),
            ));
        }

        // Validate plugin ID format (reverse domain notation)
        if !is_valid_plugin_id(&self.id) {
            return Err(ManifestValidationError::InvalidId(self.id.clone()));
        }

        // Must have at least one capability
        if self.capabilities.is_empty() {
            return Err(ManifestValidationError::NoCapabilities);
        }

        Ok(())
    }

    /// Check if plugin supports the current platform
    pub fn supports_platform(&self, platform: &Platform) -> bool {
        self.platforms.is_empty() || self.platforms.contains(platform)
    }

    /// Check if plugin has a specific capability
    pub fn has_capability(&self, capability: &Capability) -> bool {
        self.capabilities.contains(capability)
    }

    /// Get the slug (simplified ID for filesystem use)
    pub fn slug(&self) -> String {
        self.id.replace('.', "-").replace('_', "-").to_lowercase()
    }
}

/// Supported platforms
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Windows,
    MacOS,
    Linux,
    Android,
    IOS,
}

impl Platform {
    /// Get the current platform
    #[cfg(target_os = "windows")]
    pub fn current() -> Self {
        Platform::Windows
    }

    #[cfg(target_os = "macos")]
    pub fn current() -> Self {
        Platform::MacOS
    }

    #[cfg(target_os = "linux")]
    pub fn current() -> Self {
        Platform::Linux
    }

    #[cfg(target_os = "android")]
    pub fn current() -> Self {
        Platform::Android
    }

    #[cfg(target_os = "ios")]
    pub fn current() -> Self {
        Platform::IOS
    }

    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "linux",
        target_os = "android",
        target_os = "ios"
    )))]
    pub fn current() -> Self {
        Platform::Linux // Default fallback
    }
}

/// Entry points for different plugin capabilities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EntryPoints {
    /// WASM module file (required for all plugins)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wasm: Option<String>,

    /// Widget bundle directory (for UI plugins)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub widget: Option<String>,

    /// Widget entry point (HTML file within bundle)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub widget_entry: Option<String>,
}

/// Plugin dependency declaration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDependency {
    /// Dependency plugin ID
    pub id: String,

    /// Required version range (semver)
    pub version: String,

    /// Whether the dependency is optional
    #[serde(default)]
    pub optional: bool,
}

/// Manifest validation errors
#[derive(Debug, thiserror::Error)]
pub enum ManifestValidationError {
    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Invalid version format: {0}")]
    InvalidVersion(String),

    #[error("Invalid plugin ID format: {0}")]
    InvalidId(String),

    #[error("Plugin must declare at least one capability")]
    NoCapabilities,
}

/// Basic semver validation (simplified)
fn is_valid_semver(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() < 2 || parts.len() > 3 {
        return false;
    }
    parts.iter().all(|p| {
        // Allow pre-release suffixes like "1.0.0-beta"
        let numeric_part = p.split('-').next().unwrap_or(p);
        numeric_part.parse::<u32>().is_ok()
    })
}

/// Validate plugin ID format (reverse domain notation)
fn is_valid_plugin_id(id: &str) -> bool {
    if id.is_empty() || id.len() > 128 {
        return false;
    }

    let parts: Vec<&str> = id.split('.').collect();
    if parts.len() < 2 {
        return false;
    }

    parts.iter().all(|p| {
        !p.is_empty()
            && p.chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_creation() {
        let manifest = PluginManifest::new("com.example.test-plugin", "Test Plugin", "1.0.0");

        assert_eq!(manifest.id, "com.example.test-plugin");
        assert_eq!(manifest.slug(), "com-example-test-plugin");
    }

    #[test]
    fn test_manifest_validation() {
        let mut manifest = PluginManifest::new("com.example.test", "Test", "1.0.0");

        // Should fail without capabilities
        assert!(manifest.validate().is_err());

        // Should pass with capability
        manifest.capabilities.push(Capability::EventHooks);
        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn test_semver_validation() {
        assert!(is_valid_semver("1.0.0"));
        assert!(is_valid_semver("1.0"));
        assert!(is_valid_semver("1.0.0-beta"));
        assert!(!is_valid_semver("invalid"));
        assert!(!is_valid_semver("1"));
    }

    #[test]
    fn test_plugin_id_validation() {
        assert!(is_valid_plugin_id("com.example.plugin"));
        assert!(is_valid_plugin_id("io.omniedge.slack-notifier"));
        assert!(!is_valid_plugin_id("invalid")); // No dots
        assert!(!is_valid_plugin_id("")); // Empty
    }
}
