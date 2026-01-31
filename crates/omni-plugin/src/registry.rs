//! Plugin registry for managing installed plugins
//!
//! The registry tracks all discovered and loaded plugins, their states,
//! and provides methods for querying plugins by capability.

use crate::error::{PluginError, PluginResult};
use crate::loader::PluginPackage;
use crate::manifest::PluginManifest;
use crate::types::Capability;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tracing::{debug, info, warn};

/// Plugin state in the registry
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginState {
    /// Plugin discovered but not loaded
    Discovered,
    /// Plugin is being loaded
    Loading,
    /// Plugin loaded and ready
    Loaded,
    /// Plugin is running
    Running,
    /// Plugin stopped
    Stopped,
    /// Plugin failed to load/run
    Error,
    /// Plugin disabled by user
    Disabled,
}

impl std::fmt::Display for PluginState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginState::Discovered => write!(f, "discovered"),
            PluginState::Loading => write!(f, "loading"),
            PluginState::Loaded => write!(f, "loaded"),
            PluginState::Running => write!(f, "running"),
            PluginState::Stopped => write!(f, "stopped"),
            PluginState::Error => write!(f, "error"),
            PluginState::Disabled => write!(f, "disabled"),
        }
    }
}

/// Registry entry for a plugin
#[derive(Debug, Clone)]
pub struct RegistryEntry {
    /// Plugin manifest
    pub manifest: PluginManifest,
    /// Path to plugin directory
    pub path: PathBuf,
    /// Path to WASM module
    pub wasm_path: PathBuf,
    /// Current state
    pub state: PluginState,
    /// WASM hash
    pub wasm_hash: String,
    /// Last error message (if state is Error)
    pub last_error: Option<String>,
    /// Whether plugin is enabled
    pub enabled: bool,
    /// Plugin configuration
    pub config: HashMap<String, serde_json::Value>,
    /// Last loaded timestamp
    pub loaded_at: Option<u64>,
}

impl RegistryEntry {
    /// Create from a plugin package
    pub fn from_package(package: &PluginPackage) -> Self {
        Self {
            manifest: package.manifest.clone(),
            path: package.path.clone(),
            wasm_path: package.wasm_path.clone(),
            state: PluginState::Discovered,
            wasm_hash: package.wasm_hash.clone(),
            last_error: None,
            enabled: true,
            config: package.manifest.default_config.clone(),
            loaded_at: None,
        }
    }

    /// Get plugin ID
    pub fn id(&self) -> &str {
        &self.manifest.id
    }

    /// Get plugin name
    pub fn name(&self) -> &str {
        &self.manifest.name
    }

    /// Get plugin version
    pub fn version(&self) -> &str {
        &self.manifest.version
    }

    /// Check if plugin has a capability
    pub fn has_capability(&self, capability: &Capability) -> bool {
        self.manifest.has_capability(capability)
    }

    /// Check if plugin is active (loaded or running)
    pub fn is_active(&self) -> bool {
        matches!(self.state, PluginState::Loaded | PluginState::Running)
    }
}

/// Plugin registry
pub struct PluginRegistry {
    /// Registered plugins by ID
    plugins: Arc<RwLock<HashMap<String, RegistryEntry>>>,
    /// Plugins by capability index
    capability_index: Arc<RwLock<HashMap<Capability, Vec<String>>>>,
    /// State file path
    state_file: PathBuf,
}

impl PluginRegistry {
    /// Create a new registry
    pub fn new(state_file: impl Into<PathBuf>) -> Self {
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
            capability_index: Arc::new(RwLock::new(HashMap::new())),
            state_file: state_file.into(),
        }
    }

    /// Load registry state from disk
    pub fn load_state(&self) -> PluginResult<()> {
        if !self.state_file.exists() {
            debug!("Registry state file does not exist, starting fresh");
            return Ok(());
        }

        let content = std::fs::read_to_string(&self.state_file)?;
        let state: RegistryState =
            serde_json::from_str(&content).map_err(|e| PluginError::ConfigError(e.to_string()))?;

        let mut plugins = self
            .plugins
            .write()
            .map_err(|e| PluginError::Internal(format!("Lock poisoned: {}", e)))?;

        // Restore enabled/disabled state and configs
        let plugins_len = state.plugins.len();
        for (id, plugin_state) in state.plugins {
            if let Some(entry) = plugins.get_mut(&id) {
                entry.enabled = plugin_state.enabled;
                entry.config = plugin_state.config;
            }
        }

        info!("Loaded registry state with {} entries", plugins_len);
        Ok(())
    }

    /// Save registry state to disk
    pub fn save_state(&self) -> PluginResult<()> {
        let plugins = self
            .plugins
            .read()
            .map_err(|e| PluginError::Internal(format!("Lock poisoned: {}", e)))?;

        let mut state = RegistryState {
            plugins: HashMap::new(),
        };

        for (id, entry) in plugins.iter() {
            state.plugins.insert(
                id.clone(),
                PluginStateEntry {
                    enabled: entry.enabled,
                    config: entry.config.clone(),
                },
            );
        }

        // Ensure parent directory exists
        if let Some(parent) = self.state_file.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(&state)
            .map_err(|e| PluginError::ConfigError(e.to_string()))?;
        std::fs::write(&self.state_file, content)?;

        debug!("Saved registry state");
        Ok(())
    }

    /// Register a plugin from a package
    pub fn register(&self, package: &PluginPackage) -> PluginResult<()> {
        let entry = RegistryEntry::from_package(package);
        let id = entry.id().to_string();
        let capabilities = entry.manifest.capabilities.clone();

        // Add to plugins
        {
            let mut plugins = self
                .plugins
                .write()
                .map_err(|e| PluginError::Internal(format!("Lock poisoned: {}", e)))?;

            if plugins.contains_key(&id) {
                warn!("Plugin {} already registered, updating", id);
            }

            plugins.insert(id.clone(), entry);
        }

        // Update capability index
        {
            let mut index = self
                .capability_index
                .write()
                .map_err(|e| PluginError::Internal(format!("Lock poisoned: {}", e)))?;

            for cap in capabilities {
                index.entry(cap).or_default().push(id.clone());
            }
        }

        info!("Registered plugin: {}", id);
        Ok(())
    }

    /// Unregister a plugin
    pub fn unregister(&self, plugin_id: &str) -> PluginResult<()> {
        // Remove from plugins
        let capabilities = {
            let mut plugins = self
                .plugins
                .write()
                .map_err(|e| PluginError::Internal(format!("Lock poisoned: {}", e)))?;

            match plugins.remove(plugin_id) {
                Some(entry) => entry.manifest.capabilities,
                None => return Err(PluginError::NotFound(plugin_id.to_string())),
            }
        };

        // Update capability index
        {
            let mut index = self
                .capability_index
                .write()
                .map_err(|e| PluginError::Internal(format!("Lock poisoned: {}", e)))?;

            for cap in capabilities {
                if let Some(plugins) = index.get_mut(&cap) {
                    plugins.retain(|id| id != plugin_id);
                }
            }
        }

        info!("Unregistered plugin: {}", plugin_id);
        Ok(())
    }

    /// Get a plugin entry
    pub fn get(&self, plugin_id: &str) -> Option<RegistryEntry> {
        self.plugins.read().ok()?.get(plugin_id).cloned()
    }

    /// Get all registered plugins
    pub fn list_all(&self) -> Vec<RegistryEntry> {
        self.plugins
            .read()
            .map(|p| p.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Get plugins by capability
    pub fn get_by_capability(&self, capability: Capability) -> Vec<RegistryEntry> {
        let plugin_ids: Vec<String> = self
            .capability_index
            .read()
            .map(|idx| idx.get(&capability).cloned().unwrap_or_default())
            .unwrap_or_default();

        let plugins = self.plugins.read().ok();

        plugin_ids
            .iter()
            .filter_map(|id| plugins.as_ref()?.get(id).cloned())
            .filter(|e| e.enabled)
            .collect()
    }

    /// Get enabled plugins
    pub fn get_enabled(&self) -> Vec<RegistryEntry> {
        self.plugins
            .read()
            .map(|p| p.values().filter(|e| e.enabled).cloned().collect())
            .unwrap_or_default()
    }

    /// Get plugins in a specific state
    pub fn get_by_state(&self, state: PluginState) -> Vec<RegistryEntry> {
        self.plugins
            .read()
            .map(|p| p.values().filter(|e| e.state == state).cloned().collect())
            .unwrap_or_default()
    }

    /// Update plugin state
    pub fn set_state(&self, plugin_id: &str, state: PluginState) -> PluginResult<()> {
        let mut plugins = self
            .plugins
            .write()
            .map_err(|e| PluginError::Internal(format!("Lock poisoned: {}", e)))?;

        match plugins.get_mut(plugin_id) {
            Some(entry) => {
                entry.state = state;
                if state == PluginState::Loaded || state == PluginState::Running {
                    entry.loaded_at = Some(crate::host::host_time_now_ms());
                }
                if state != PluginState::Error {
                    entry.last_error = None;
                }
                Ok(())
            }
            None => Err(PluginError::NotFound(plugin_id.to_string())),
        }
    }

    /// Set plugin error
    pub fn set_error(&self, plugin_id: &str, error: impl Into<String>) -> PluginResult<()> {
        let mut plugins = self
            .plugins
            .write()
            .map_err(|e| PluginError::Internal(format!("Lock poisoned: {}", e)))?;

        match plugins.get_mut(plugin_id) {
            Some(entry) => {
                entry.state = PluginState::Error;
                entry.last_error = Some(error.into());
                Ok(())
            }
            None => Err(PluginError::NotFound(plugin_id.to_string())),
        }
    }

    /// Enable a plugin
    pub fn enable(&self, plugin_id: &str) -> PluginResult<()> {
        let mut plugins = self
            .plugins
            .write()
            .map_err(|e| PluginError::Internal(format!("Lock poisoned: {}", e)))?;

        match plugins.get_mut(plugin_id) {
            Some(entry) => {
                entry.enabled = true;
                if entry.state == PluginState::Disabled {
                    entry.state = PluginState::Discovered;
                }
                Ok(())
            }
            None => Err(PluginError::NotFound(plugin_id.to_string())),
        }
    }

    /// Disable a plugin
    pub fn disable(&self, plugin_id: &str) -> PluginResult<()> {
        let mut plugins = self
            .plugins
            .write()
            .map_err(|e| PluginError::Internal(format!("Lock poisoned: {}", e)))?;

        match plugins.get_mut(plugin_id) {
            Some(entry) => {
                entry.enabled = false;
                entry.state = PluginState::Disabled;
                Ok(())
            }
            None => Err(PluginError::NotFound(plugin_id.to_string())),
        }
    }

    /// Update plugin configuration
    pub fn set_config(
        &self,
        plugin_id: &str,
        config: HashMap<String, serde_json::Value>,
    ) -> PluginResult<()> {
        let mut plugins = self
            .plugins
            .write()
            .map_err(|e| PluginError::Internal(format!("Lock poisoned: {}", e)))?;

        match plugins.get_mut(plugin_id) {
            Some(entry) => {
                entry.config = config;
                Ok(())
            }
            None => Err(PluginError::NotFound(plugin_id.to_string())),
        }
    }

    /// Get plugin configuration
    pub fn get_config(&self, plugin_id: &str) -> Option<HashMap<String, serde_json::Value>> {
        self.plugins
            .read()
            .ok()?
            .get(plugin_id)
            .map(|e| e.config.clone())
    }

    /// Get plugin count
    pub fn count(&self) -> usize {
        self.plugins.read().map(|p| p.len()).unwrap_or(0)
    }

    /// Get enabled plugin count
    pub fn enabled_count(&self) -> usize {
        self.plugins
            .read()
            .map(|p| p.values().filter(|e| e.enabled).count())
            .unwrap_or(0)
    }
}

/// Serializable registry state
#[derive(Debug, Serialize, Deserialize)]
struct RegistryState {
    plugins: HashMap<String, PluginStateEntry>,
}

/// Serializable plugin state entry
#[derive(Debug, Serialize, Deserialize)]
struct PluginStateEntry {
    enabled: bool,
    config: HashMap<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Capability;
    use tempfile::tempdir;

    fn create_test_manifest() -> PluginManifest {
        let mut manifest = PluginManifest::new("com.test.plugin", "Test Plugin", "1.0.0");
        manifest.capabilities.push(Capability::EventHooks);
        manifest
    }

    fn create_test_package() -> PluginPackage {
        PluginPackage {
            manifest: create_test_manifest(),
            path: PathBuf::from("/test"),
            wasm_path: PathBuf::from("/test/plugin.wasm"),
            wasm_hash: "abc123".to_string(),
            signature: None,
        }
    }

    #[test]
    fn test_registry_register() {
        let dir = tempdir().unwrap();
        let state_file = dir.path().join("registry.json");
        let registry = PluginRegistry::new(state_file);

        let package = create_test_package();
        registry.register(&package).unwrap();

        assert_eq!(registry.count(), 1);

        let entry = registry.get("com.test.plugin").unwrap();
        assert_eq!(entry.name(), "Test Plugin");
        assert_eq!(entry.state, PluginState::Discovered);
    }

    #[test]
    fn test_registry_capability_index() {
        let dir = tempdir().unwrap();
        let state_file = dir.path().join("registry.json");
        let registry = PluginRegistry::new(state_file);

        let package = create_test_package();
        registry.register(&package).unwrap();

        let event_plugins = registry.get_by_capability(Capability::EventHooks);
        assert_eq!(event_plugins.len(), 1);

        let auth_plugins = registry.get_by_capability(Capability::Authentication);
        assert_eq!(auth_plugins.len(), 0);
    }

    #[test]
    fn test_registry_enable_disable() {
        let dir = tempdir().unwrap();
        let state_file = dir.path().join("registry.json");
        let registry = PluginRegistry::new(state_file);

        let package = create_test_package();
        registry.register(&package).unwrap();

        registry.disable("com.test.plugin").unwrap();
        let entry = registry.get("com.test.plugin").unwrap();
        assert!(!entry.enabled);
        assert_eq!(entry.state, PluginState::Disabled);

        registry.enable("com.test.plugin").unwrap();
        let entry = registry.get("com.test.plugin").unwrap();
        assert!(entry.enabled);
    }

    #[test]
    fn test_registry_state_persistence() {
        let dir = tempdir().unwrap();
        let state_file = dir.path().join("registry.json");

        // Create and save
        {
            let registry = PluginRegistry::new(&state_file);
            let package = create_test_package();
            registry.register(&package).unwrap();
            registry.disable("com.test.plugin").unwrap();
            registry.save_state().unwrap();
        }

        // Load in new registry
        {
            let registry = PluginRegistry::new(&state_file);
            let package = create_test_package();
            registry.register(&package).unwrap();
            registry.load_state().unwrap();

            let entry = registry.get("com.test.plugin").unwrap();
            assert!(!entry.enabled);
        }
    }
}
