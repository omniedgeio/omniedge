//! Plugin Manager - orchestrates plugin lifecycle
//!
//! The PluginManager is the main entry point for the plugin system.
//! It handles discovery, loading, unloading, and event dispatching.

use crate::error::{PluginError, PluginResult};
use crate::host::HostState;
use crate::loader::{PluginLoader, PluginPackage};
use crate::registry::{PluginRegistry, PluginState, RegistryEntry};
use crate::sandbox::{PluginInstance, PluginSandbox, PluginStoreState};
use crate::types::*;
use crate::PluginConfig;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use wasmtime::Store;

/// A running plugin instance
struct RunningPlugin {
    /// Plugin instance
    instance: PluginInstance,
    /// Store with plugin state
    store: Store<PluginStoreState>,
    /// Plugin package info
    package: PluginPackage,
}

/// Plugin Manager - main orchestrator for the plugin system
pub struct PluginManager {
    /// Configuration
    config: PluginConfig,
    /// Plugin sandbox
    sandbox: PluginSandbox,
    /// Plugin loader
    loader: PluginLoader,
    /// Plugin registry
    registry: PluginRegistry,
    /// Running plugin instances
    running: Arc<RwLock<HashMap<String, RunningPlugin>>>,
}

impl PluginManager {
    /// Create a new plugin manager with the given configuration
    pub fn new(config: PluginConfig) -> PluginResult<Self> {
        let sandbox = crate::sandbox::SandboxBuilder::new()
            .max_memory(config.max_memory)
            .max_execution_time_ms(config.max_execution_time_ms)
            .build()?;

        let loader = PluginLoader::new(config.plugins_dir())
            .require_signatures(config.require_signatures)
            .with_trusted_signers(config.trusted_signers.clone());

        let registry = PluginRegistry::new(config.config_file());

        Ok(Self {
            config,
            sandbox,
            loader,
            registry,
            running: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Create a new plugin manager with default configuration
    pub fn with_defaults() -> PluginResult<Self> {
        Self::new(PluginConfig::default())
    }

    /// Get plugin configuration
    pub fn config(&self) -> &PluginConfig {
        &self.config
    }

    /// Get the registry
    pub fn registry(&self) -> &PluginRegistry {
        &self.registry
    }

    /// Initialize the plugin manager
    pub async fn initialize(&mut self) -> PluginResult<()> {
        // Ensure directories exist
        std::fs::create_dir_all(self.config.plugins_dir())?;
        std::fs::create_dir_all(self.config.cache_dir())?;

        // Link host functions
        self.sandbox.link_host_functions()?;

        // Load registry state
        self.registry.load_state()?;

        info!("Plugin manager initialized");
        Ok(())
    }

    /// Discover all plugins in the plugins directory
    pub async fn discover_plugins(&self) -> PluginResult<Vec<String>> {
        let packages = self.loader.discover_plugins()?;
        let mut discovered = Vec::new();

        for package in packages {
            let id = package.manifest.id.clone();
            self.registry.register(&package)?;
            discovered.push(id);
        }

        info!("Discovered {} plugins", discovered.len());
        Ok(discovered)
    }

    /// Load a plugin by ID
    pub async fn load_plugin(&self, plugin_id: &str) -> PluginResult<()> {
        // Check if already running
        {
            let running = self.running.read().await;
            if running.contains_key(plugin_id) {
                return Err(PluginError::AlreadyLoaded(plugin_id.to_string()));
            }
        }

        // Get registry entry
        let entry = self
            .registry
            .get(plugin_id)
            .ok_or_else(|| PluginError::NotFound(plugin_id.to_string()))?;

        if !entry.enabled {
            return Err(PluginError::LifecycleError(format!(
                "Plugin {} is disabled",
                plugin_id
            )));
        }

        // Update state to loading
        self.registry.set_state(plugin_id, PluginState::Loading)?;

        // Load the package
        let package = self.loader.load_plugin_package(&entry.path)?;

        // Load into sandbox
        let instance = match self.loader.load_into_sandbox(&package, &self.sandbox) {
            Ok(inst) => inst,
            Err(e) => {
                self.registry.set_error(plugin_id, e.to_string())?;
                return Err(e);
            }
        };

        // Create host state with plugin config
        let host_state = HostState::new(plugin_id).with_config(entry.config.clone());

        // Create store
        let store = self.sandbox.create_store(host_state);

        // Create running plugin
        let running_plugin = RunningPlugin {
            instance,
            store,
            package,
        };

        // Add to running plugins
        {
            let mut running = self.running.write().await;
            running.insert(plugin_id.to_string(), running_plugin);
        }

        // Update state
        self.registry.set_state(plugin_id, PluginState::Loaded)?;

        info!("Loaded plugin: {}", plugin_id);
        Ok(())
    }

    /// Unload a plugin by ID
    pub async fn unload_plugin(&self, plugin_id: &str) -> PluginResult<()> {
        let mut running = self.running.write().await;

        match running.remove(plugin_id) {
            Some(_plugin) => {
                // Plugin dropped, resources freed
                self.registry.set_state(plugin_id, PluginState::Stopped)?;
                info!("Unloaded plugin: {}", plugin_id);
                Ok(())
            }
            None => Err(PluginError::NotFound(plugin_id.to_string())),
        }
    }

    /// Load all enabled plugins
    pub async fn load_all(&self) -> PluginResult<()> {
        let enabled = self.registry.get_enabled();

        for entry in enabled {
            if let Err(e) = self.load_plugin(&entry.manifest.id).await {
                warn!("Failed to load plugin {}: {}", entry.manifest.id, e);
                // Continue loading other plugins
            }
        }

        Ok(())
    }

    /// Unload all plugins
    pub async fn unload_all(&self) -> PluginResult<()> {
        let plugin_ids: Vec<String> = {
            let running = self.running.read().await;
            running.keys().cloned().collect()
        };

        for id in plugin_ids {
            if let Err(e) = self.unload_plugin(&id).await {
                warn!("Failed to unload plugin {}: {}", id, e);
            }
        }

        Ok(())
    }

    /// Install a plugin from a zip file
    pub async fn install_plugin(&self, zip_path: &Path) -> PluginResult<String> {
        let package = self.loader.install_from_zip(zip_path)?;
        let id = package.manifest.id.clone();

        self.registry.register(&package)?;
        self.registry.save_state()?;

        info!("Installed plugin: {}", id);
        Ok(id)
    }

    /// Install a plugin from bytes
    pub async fn install_plugin_bytes(&self, zip_bytes: &[u8]) -> PluginResult<String> {
        let package = self.loader.install_from_bytes(zip_bytes)?;
        let id = package.manifest.id.clone();

        self.registry.register(&package)?;
        self.registry.save_state()?;

        info!("Installed plugin: {}", id);
        Ok(id)
    }

    /// Uninstall a plugin
    pub async fn uninstall_plugin(&self, plugin_id: &str) -> PluginResult<()> {
        // Unload if running
        let _ = self.unload_plugin(plugin_id).await;

        // Unregister
        self.registry.unregister(plugin_id)?;

        // Remove files
        self.loader.uninstall(plugin_id)?;

        // Save state
        self.registry.save_state()?;

        info!("Uninstalled plugin: {}", plugin_id);
        Ok(())
    }

    /// Enable a plugin
    pub async fn enable_plugin(&self, plugin_id: &str) -> PluginResult<()> {
        self.registry.enable(plugin_id)?;
        self.registry.save_state()?;
        info!("Enabled plugin: {}", plugin_id);
        Ok(())
    }

    /// Disable a plugin
    pub async fn disable_plugin(&self, plugin_id: &str) -> PluginResult<()> {
        // Unload if running
        let _ = self.unload_plugin(plugin_id).await;

        self.registry.disable(plugin_id)?;
        self.registry.save_state()?;
        info!("Disabled plugin: {}", plugin_id);
        Ok(())
    }

    /// Update plugin configuration
    pub async fn update_config(
        &self,
        plugin_id: &str,
        config: HashMap<String, serde_json::Value>,
    ) -> PluginResult<()> {
        self.registry.set_config(plugin_id, config)?;
        self.registry.save_state()?;

        // If plugin is running, reload it with new config
        {
            let running = self.running.read().await;
            if running.contains_key(plugin_id) {
                drop(running);
                self.unload_plugin(plugin_id).await?;
                self.load_plugin(plugin_id).await?;
            }
        }

        Ok(())
    }

    /// Get list of running plugins
    pub async fn running_plugins(&self) -> Vec<String> {
        let running = self.running.read().await;
        running.keys().cloned().collect()
    }

    /// Get plugin info
    pub fn get_plugin_info(&self, plugin_id: &str) -> Option<PluginInfo> {
        let entry = self.registry.get(plugin_id)?;
        Some(PluginInfo::from_entry(&entry))
    }

    /// List all plugins
    pub fn list_plugins(&self) -> Vec<PluginInfo> {
        self.registry
            .list_all()
            .iter()
            .map(PluginInfo::from_entry)
            .collect()
    }

    /// Get plugins by capability
    pub fn get_plugins_by_capability(&self, capability: Capability) -> Vec<PluginInfo> {
        self.registry
            .get_by_capability(capability)
            .iter()
            .map(PluginInfo::from_entry)
            .collect()
    }

    // =========================================================================
    // Event Dispatch
    // =========================================================================

    /// Emit an event to all event plugins
    pub async fn emit_event(&self, _event: Event) {
        let running = self.running.read().await;

        for (id, _plugin) in running.iter() {
            // Check if plugin handles events
            if let Some(entry) = self.registry.get(id) {
                if entry.has_capability(&Capability::EventHooks) {
                    debug!("Dispatching event to plugin: {}", id);
                    // TODO: Actually call the plugin's event handler
                    // This requires invoking the WASM function
                }
            }
        }
    }

    /// Emit state change event
    pub async fn emit_state_change(&self, event: StateChangeEvent) {
        self.emit_event(Event::StateChange(event)).await;
    }

    /// Emit peer discovered event
    pub async fn emit_peer_discovered(&self, peer: PeerInfo) {
        self.emit_event(Event::PeerDiscovered { peer }).await;
    }

    /// Emit peer disconnected event
    pub async fn emit_peer_disconnected(&self, peer: PeerInfo) {
        self.emit_event(Event::PeerDisconnected { peer }).await;
    }

    /// Emit network change event
    pub async fn emit_network_change(&self, event: NetworkEvent) {
        self.emit_event(Event::NetworkChange(event)).await;
    }

    /// Emit stats update event
    pub async fn emit_stats_update(&self, stats: ConnectionStats) {
        self.emit_event(Event::StatsUpdate { stats }).await;
    }

    // =========================================================================
    // Policy Queries
    // =========================================================================

    /// Query policy plugins for network selection
    pub async fn query_network_selection(&self, _ctx: &PolicyContext) -> Option<String> {
        let running = self.running.read().await;

        for (id, _plugin) in running.iter() {
            if let Some(entry) = self.registry.get(id) {
                if entry.has_capability(&Capability::NetworkPolicy) {
                    debug!("Querying policy plugin for network selection: {}", id);
                    // TODO: Actually call the plugin's select_network function
                }
            }
        }

        None // No policy plugin made a selection
    }

    /// Query policy plugins for exit node selection
    pub async fn query_exit_node_selection(&self, _ctx: &PolicyContext) -> Option<String> {
        let running = self.running.read().await;

        for (id, _plugin) in running.iter() {
            if let Some(entry) = self.registry.get(id) {
                if entry.has_capability(&Capability::NetworkPolicy) {
                    debug!("Querying policy plugin for exit node selection: {}", id);
                    // TODO: Actually call the plugin's select_exit_node function
                }
            }
        }

        None
    }

    /// Query policy plugins for connection validation
    pub async fn query_connection_validation(&self, _ctx: &PolicyContext) -> PolicyDecision {
        let running = self.running.read().await;

        for (id, _plugin) in running.iter() {
            if let Some(entry) = self.registry.get(id) {
                if entry.has_capability(&Capability::NetworkPolicy) {
                    debug!("Querying policy plugin for connection validation: {}", id);
                    // TODO: Actually call the plugin's validate_connection function
                    // Return first Deny or RequireAuth
                }
            }
        }

        PolicyDecision::Allow
    }

    // =========================================================================
    // QoS Queries
    // =========================================================================

    /// Query QoS plugins for packet classification
    pub async fn classify_packet(&self, _packet: &PacketInfo) -> TrafficClass {
        let running = self.running.read().await;

        for (id, _plugin) in running.iter() {
            if let Some(entry) = self.registry.get(id) {
                if entry.has_capability(&Capability::QosEnforcement) {
                    debug!("Querying QoS plugin for packet classification: {}", id);
                    // TODO: Actually call the plugin's classify_packet function
                }
            }
        }

        // Default to standard traffic
        TrafficClass::Standard { dscp: 0 }
    }

    /// Shutdown the plugin manager
    pub async fn shutdown(&self) -> PluginResult<()> {
        info!("Shutting down plugin manager");

        // Unload all plugins
        self.unload_all().await?;

        // Save registry state
        self.registry.save_state()?;

        Ok(())
    }
}

/// Plugin information for API responses
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub capabilities: Vec<Capability>,
    pub state: PluginState,
    pub enabled: bool,
    pub error: Option<String>,
}

impl PluginInfo {
    fn from_entry(entry: &RegistryEntry) -> Self {
        Self {
            id: entry.manifest.id.clone(),
            name: entry.manifest.name.clone(),
            version: entry.manifest.version.clone(),
            author: entry.manifest.author.clone(),
            description: entry.manifest.description.clone(),
            capabilities: entry.manifest.capabilities.clone(),
            state: entry.state,
            enabled: entry.enabled,
            error: entry.last_error.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_manager_creation() {
        let dir = tempdir().unwrap();
        let config = PluginConfig {
            data_dir: dir.path().to_path_buf(),
            ..Default::default()
        };

        let manager = PluginManager::new(config);
        assert!(manager.is_ok());
    }

    #[tokio::test]
    async fn test_manager_initialize() {
        let dir = tempdir().unwrap();
        let config = PluginConfig {
            data_dir: dir.path().to_path_buf(),
            ..Default::default()
        };

        let mut manager = PluginManager::new(config).unwrap();
        let result = manager.initialize().await;
        assert!(result.is_ok());

        // Check directories were created
        assert!(dir.path().join("plugins").join("installed").exists());
    }

    #[tokio::test]
    async fn test_manager_discover_empty() {
        let dir = tempdir().unwrap();
        let config = PluginConfig {
            data_dir: dir.path().to_path_buf(),
            ..Default::default()
        };

        let mut manager = PluginManager::new(config).unwrap();
        manager.initialize().await.unwrap();

        let discovered = manager.discover_plugins().await.unwrap();
        assert!(discovered.is_empty());
    }
}
