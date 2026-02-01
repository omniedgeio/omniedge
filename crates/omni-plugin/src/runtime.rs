//! Plugin runtime for executing WASM plugins
//!
//! This module provides the runtime environment for executing plugins,
//! including component instantiation and function invocation.

use crate::error::{PluginError, PluginResult};
use crate::host::HostState;
use crate::manifest::PluginManifest;
use crate::types::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use wasmtime::component::{Component, Linker as ComponentLinker};
use wasmtime::*;

/// Plugin runtime state
pub struct PluginState {
    /// Host state for callbacks
    pub host: HostState,
    /// Resource limits
    pub limits: StoreLimits,
}

impl PluginState {
    /// Create new plugin state
    pub fn new(host: HostState) -> Self {
        Self {
            host,
            limits: StoreLimitsBuilder::new()
                .memory_size(64 * 1024 * 1024) // 64MB
                .table_elements(10_000)
                .instances(10)
                .build(),
        }
    }
}

/// A loaded and instantiated plugin
#[allow(dead_code)]
pub struct PluginRuntime {
    /// Plugin manifest
    manifest: PluginManifest,
    /// Wasmtime engine
    engine: Engine,
    /// Compiled component
    component: Component,
    /// Component linker
    linker: ComponentLinker<PluginState>,
}

impl PluginRuntime {
    /// Create a new plugin runtime from WASM bytes
    pub fn new(manifest: PluginManifest, wasm_bytes: &[u8]) -> PluginResult<Self> {
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.consume_fuel(true);
        config.async_support(false); // Sync for now

        let engine = Engine::new(&config)?;
        let component = Component::new(&engine, wasm_bytes)?;
        let linker = ComponentLinker::new(&engine);

        Ok(Self {
            manifest,
            engine,
            component,
            linker,
        })
    }

    /// Get the plugin manifest
    pub fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    /// Get the plugin ID
    pub fn id(&self) -> &str {
        &self.manifest.id
    }

    /// Create a new store for this plugin
    pub fn create_store(&self, config: HashMap<String, serde_json::Value>) -> Store<PluginState> {
        let host = HostState::new(&self.manifest.id).with_config(config);
        let state = PluginState::new(host);
        let mut store = Store::new(&self.engine, state);
        store.limiter(|s| &mut s.limits);
        let _ = store.set_fuel(1_000_000);
        store
    }

    /// Link host functions to the component linker
    pub fn link_host_functions(&mut self) -> PluginResult<()> {
        // For now, we use a simplified approach without full WIT binding
        // In a complete implementation, we would use wit-bindgen generated code

        // The component model requires us to define imports matching the WIT interfaces
        // This is a placeholder for the actual implementation

        debug!("Host functions linked for plugin: {}", self.manifest.id);
        Ok(())
    }
}

/// Event plugin runtime wrapper
#[allow(dead_code)]
pub struct EventPluginRuntime {
    inner: PluginRuntime,
    /// Cached function exports
    on_state_change: Option<String>,
    on_peer_discovered: Option<String>,
    on_peer_disconnected: Option<String>,
    on_network_change: Option<String>,
    on_stats_update: Option<String>,
}

impl EventPluginRuntime {
    /// Create from a plugin runtime
    pub fn new(runtime: PluginRuntime) -> Self {
        Self {
            inner: runtime,
            on_state_change: Some("on-state-change".to_string()),
            on_peer_discovered: Some("on-peer-discovered".to_string()),
            on_peer_disconnected: Some("on-peer-disconnected".to_string()),
            on_network_change: Some("on-network-change".to_string()),
            on_stats_update: Some("on-stats-update".to_string()),
        }
    }

    /// Get the inner runtime
    pub fn inner(&self) -> &PluginRuntime {
        &self.inner
    }

    /// Dispatch a state change event
    pub fn dispatch_state_change(
        &self,
        _store: &mut Store<PluginState>,
        event: &StateChangeEvent,
    ) -> PluginResult<()> {
        // Serialize event to JSON for passing to WASM
        let event_json =
            serde_json::to_string(event).map_err(|e| PluginError::ExecutionError(e.to_string()))?;

        debug!(
            "Dispatching state change to plugin {}: {:?} -> {:?}",
            self.inner.manifest.id, event.old_state, event.new_state
        );

        // In a complete implementation, we would:
        // 1. Allocate memory in WASM for the event data
        // 2. Copy the serialized event into WASM memory
        // 3. Call the exported function with the pointer and length
        // 4. Free the memory after the call

        // For now, log that we would dispatch
        info!(
            "Would dispatch on_state_change to plugin {} with event: {}",
            self.inner.manifest.id, event_json
        );

        Ok(())
    }

    /// Dispatch a peer discovered event
    pub fn dispatch_peer_discovered(
        &self,
        _store: &mut Store<PluginState>,
        peer: &PeerInfo,
    ) -> PluginResult<()> {
        let peer_json =
            serde_json::to_string(peer).map_err(|e| PluginError::ExecutionError(e.to_string()))?;

        debug!(
            "Dispatching peer discovered to plugin {}: {}",
            self.inner.manifest.id, peer.virtual_ip
        );

        info!(
            "Would dispatch on_peer_discovered to plugin {} with peer: {}",
            self.inner.manifest.id, peer_json
        );

        Ok(())
    }

    /// Dispatch a peer disconnected event
    pub fn dispatch_peer_disconnected(
        &self,
        _store: &mut Store<PluginState>,
        peer: &PeerInfo,
    ) -> PluginResult<()> {
        let peer_json =
            serde_json::to_string(peer).map_err(|e| PluginError::ExecutionError(e.to_string()))?;

        debug!(
            "Dispatching peer disconnected to plugin {}: {}",
            self.inner.manifest.id, peer.virtual_ip
        );

        info!(
            "Would dispatch on_peer_disconnected to plugin {} with peer: {}",
            self.inner.manifest.id, peer_json
        );

        Ok(())
    }

    /// Dispatch a network change event
    pub fn dispatch_network_change(
        &self,
        _store: &mut Store<PluginState>,
        event: &NetworkEvent,
    ) -> PluginResult<()> {
        let event_json =
            serde_json::to_string(event).map_err(|e| PluginError::ExecutionError(e.to_string()))?;

        debug!(
            "Dispatching network change to plugin {}: {:?}",
            self.inner.manifest.id, event.event_type
        );

        info!(
            "Would dispatch on_network_change to plugin {} with event: {}",
            self.inner.manifest.id, event_json
        );

        Ok(())
    }

    /// Dispatch a stats update event
    pub fn dispatch_stats_update(
        &self,
        _store: &mut Store<PluginState>,
        stats: &ConnectionStats,
    ) -> PluginResult<()> {
        let stats_json =
            serde_json::to_string(stats).map_err(|e| PluginError::ExecutionError(e.to_string()))?;

        debug!(
            "Dispatching stats update to plugin {}: {} peers",
            self.inner.manifest.id, stats.connected_peers
        );

        info!(
            "Would dispatch on_stats_update to plugin {} with stats: {}",
            self.inner.manifest.id, stats_json
        );

        Ok(())
    }
}

/// Policy plugin runtime wrapper
pub struct PolicyPluginRuntime {
    inner: PluginRuntime,
}

impl PolicyPluginRuntime {
    /// Create from a plugin runtime
    pub fn new(runtime: PluginRuntime) -> Self {
        Self { inner: runtime }
    }

    /// Query for network selection
    pub fn select_network(
        &self,
        _store: &mut Store<PluginState>,
        ctx: &PolicyContext,
    ) -> PluginResult<Option<String>> {
        let _ctx_json =
            serde_json::to_string(ctx).map_err(|e| PluginError::ExecutionError(e.to_string()))?;

        debug!(
            "Querying network selection from plugin {}: {} networks available",
            self.inner.manifest.id,
            ctx.available_networks.len()
        );

        // Placeholder - would call WASM function
        Ok(None)
    }

    /// Query for exit node selection
    pub fn select_exit_node(
        &self,
        _store: &mut Store<PluginState>,
        ctx: &PolicyContext,
    ) -> PluginResult<Option<String>> {
        let _ctx_json =
            serde_json::to_string(ctx).map_err(|e| PluginError::ExecutionError(e.to_string()))?;

        debug!(
            "Querying exit node selection from plugin {}: {} nodes available",
            self.inner.manifest.id,
            ctx.available_exit_nodes.len()
        );

        // Placeholder - would call WASM function
        Ok(None)
    }

    /// Query for connection validation
    pub fn validate_connection(
        &self,
        _store: &mut Store<PluginState>,
        ctx: &PolicyContext,
    ) -> PluginResult<PolicyDecision> {
        let _ctx_json =
            serde_json::to_string(ctx).map_err(|e| PluginError::ExecutionError(e.to_string()))?;

        debug!(
            "Validating connection from plugin {}: network type {:?}",
            self.inner.manifest.id, ctx.network_type
        );

        // Placeholder - would call WASM function
        Ok(PolicyDecision::Allow)
    }
}

/// QoS plugin runtime wrapper
pub struct QoSPluginRuntime {
    inner: PluginRuntime,
}

impl QoSPluginRuntime {
    /// Create from a plugin runtime
    pub fn new(runtime: PluginRuntime) -> Self {
        Self { inner: runtime }
    }

    /// Classify a packet
    pub fn classify_packet(
        &self,
        _store: &mut Store<PluginState>,
        packet: &PacketInfo,
    ) -> PluginResult<TrafficClass> {
        let _packet_json = serde_json::to_string(packet)
            .map_err(|e| PluginError::ExecutionError(e.to_string()))?;

        debug!(
            "Classifying packet from plugin {}: {} -> {}",
            self.inner.manifest.id, packet.source, packet.destination
        );

        // Placeholder - would call WASM function
        Ok(TrafficClass::Standard { dscp: 0 })
    }
}

/// Plugin runtime manager - handles multiple plugin runtimes
pub struct PluginRuntimeManager {
    /// Event plugin runtimes
    event_plugins: Arc<RwLock<HashMap<String, EventPluginRuntime>>>,
    /// Policy plugin runtimes
    policy_plugins: Arc<RwLock<HashMap<String, PolicyPluginRuntime>>>,
    /// QoS plugin runtimes
    qos_plugins: Arc<RwLock<HashMap<String, QoSPluginRuntime>>>,
}

impl PluginRuntimeManager {
    /// Create a new runtime manager
    pub fn new() -> Self {
        Self {
            event_plugins: Arc::new(RwLock::new(HashMap::new())),
            policy_plugins: Arc::new(RwLock::new(HashMap::new())),
            qos_plugins: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register an event plugin
    pub async fn register_event_plugin(&self, runtime: EventPluginRuntime) {
        let id = runtime.inner().manifest().id.clone();
        let mut plugins = self.event_plugins.write().await;
        plugins.insert(id.clone(), runtime);
        info!("Registered event plugin: {}", id);
    }

    /// Register a policy plugin
    pub async fn register_policy_plugin(&self, runtime: PolicyPluginRuntime) {
        let id = runtime.inner.manifest.id.clone();
        let mut plugins = self.policy_plugins.write().await;
        plugins.insert(id.clone(), runtime);
        info!("Registered policy plugin: {}", id);
    }

    /// Register a QoS plugin
    pub async fn register_qos_plugin(&self, runtime: QoSPluginRuntime) {
        let id = runtime.inner.manifest.id.clone();
        let mut plugins = self.qos_plugins.write().await;
        plugins.insert(id.clone(), runtime);
        info!("Registered QoS plugin: {}", id);
    }

    /// Unregister a plugin by ID
    pub async fn unregister(&self, plugin_id: &str) {
        let mut event_plugins = self.event_plugins.write().await;
        event_plugins.remove(plugin_id);

        let mut policy_plugins = self.policy_plugins.write().await;
        policy_plugins.remove(plugin_id);

        let mut qos_plugins = self.qos_plugins.write().await;
        qos_plugins.remove(plugin_id);

        info!("Unregistered plugin: {}", plugin_id);
    }

    /// Broadcast state change to all event plugins
    pub async fn broadcast_state_change(&self, event: &StateChangeEvent) {
        let plugins = self.event_plugins.read().await;

        for (id, plugin) in plugins.iter() {
            let mut store = plugin.inner().create_store(HashMap::new());
            if let Err(e) = plugin.dispatch_state_change(&mut store, event) {
                warn!("Failed to dispatch state change to plugin {}: {}", id, e);
            }
        }
    }

    /// Broadcast peer discovered to all event plugins
    pub async fn broadcast_peer_discovered(&self, peer: &PeerInfo) {
        let plugins = self.event_plugins.read().await;

        for (id, plugin) in plugins.iter() {
            let mut store = plugin.inner().create_store(HashMap::new());
            if let Err(e) = plugin.dispatch_peer_discovered(&mut store, peer) {
                warn!("Failed to dispatch peer discovered to plugin {}: {}", id, e);
            }
        }
    }

    /// Broadcast peer disconnected to all event plugins
    pub async fn broadcast_peer_disconnected(&self, peer: &PeerInfo) {
        let plugins = self.event_plugins.read().await;

        for (id, plugin) in plugins.iter() {
            let mut store = plugin.inner().create_store(HashMap::new());
            if let Err(e) = plugin.dispatch_peer_disconnected(&mut store, peer) {
                warn!(
                    "Failed to dispatch peer disconnected to plugin {}: {}",
                    id, e
                );
            }
        }
    }

    /// Broadcast network change to all event plugins
    pub async fn broadcast_network_change(&self, event: &NetworkEvent) {
        let plugins = self.event_plugins.read().await;

        for (id, plugin) in plugins.iter() {
            let mut store = plugin.inner().create_store(HashMap::new());
            if let Err(e) = plugin.dispatch_network_change(&mut store, event) {
                warn!("Failed to dispatch network change to plugin {}: {}", id, e);
            }
        }
    }

    /// Broadcast stats update to all event plugins
    pub async fn broadcast_stats_update(&self, stats: &ConnectionStats) {
        let plugins = self.event_plugins.read().await;

        for (id, plugin) in plugins.iter() {
            let mut store = plugin.inner().create_store(HashMap::new());
            if let Err(e) = plugin.dispatch_stats_update(&mut store, stats) {
                warn!("Failed to dispatch stats update to plugin {}: {}", id, e);
            }
        }
    }

    /// Query all policy plugins for network selection (first match wins)
    pub async fn query_network_selection(&self, ctx: &PolicyContext) -> Option<String> {
        let plugins = self.policy_plugins.read().await;

        for (id, plugin) in plugins.iter() {
            let mut store = plugin.inner.create_store(HashMap::new());
            match plugin.select_network(&mut store, ctx) {
                Ok(Some(network_id)) => {
                    info!("Policy plugin {} selected network: {}", id, network_id);
                    return Some(network_id);
                }
                Ok(None) => continue,
                Err(e) => {
                    warn!("Policy plugin {} failed to select network: {}", id, e);
                }
            }
        }

        None
    }

    /// Query all policy plugins for exit node selection (first match wins)
    pub async fn query_exit_node_selection(&self, ctx: &PolicyContext) -> Option<String> {
        let plugins = self.policy_plugins.read().await;

        for (id, plugin) in plugins.iter() {
            let mut store = plugin.inner.create_store(HashMap::new());
            match plugin.select_exit_node(&mut store, ctx) {
                Ok(Some(node_id)) => {
                    info!("Policy plugin {} selected exit node: {}", id, node_id);
                    return Some(node_id);
                }
                Ok(None) => continue,
                Err(e) => {
                    warn!("Policy plugin {} failed to select exit node: {}", id, e);
                }
            }
        }

        None
    }

    /// Query all policy plugins for connection validation (first deny/require-auth wins)
    pub async fn query_connection_validation(&self, ctx: &PolicyContext) -> PolicyDecision {
        let plugins = self.policy_plugins.read().await;

        for (id, plugin) in plugins.iter() {
            let mut store = plugin.inner.create_store(HashMap::new());
            match plugin.validate_connection(&mut store, ctx) {
                Ok(PolicyDecision::Allow) => continue,
                Ok(decision) => {
                    info!("Policy plugin {} returned: {:?}", id, decision);
                    return decision;
                }
                Err(e) => {
                    warn!("Policy plugin {} failed to validate connection: {}", id, e);
                }
            }
        }

        PolicyDecision::Allow
    }

    /// Classify a packet using QoS plugins (first classification wins)
    pub async fn classify_packet(&self, packet: &PacketInfo) -> TrafficClass {
        let plugins = self.qos_plugins.read().await;

        for (id, plugin) in plugins.iter() {
            let mut store = plugin.inner.create_store(HashMap::new());
            match plugin.classify_packet(&mut store, packet) {
                Ok(class) => {
                    debug!("QoS plugin {} classified packet as: {:?}", id, class);
                    return class;
                }
                Err(e) => {
                    warn!("QoS plugin {} failed to classify packet: {}", id, e);
                }
            }
        }

        // Default classification
        TrafficClass::Standard { dscp: 0 }
    }
}

impl Default for PluginRuntimeManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Capability;

    #[allow(dead_code)]
    fn create_test_manifest() -> PluginManifest {
        let mut manifest = PluginManifest::new("com.test.plugin", "Test Plugin", "1.0.0");
        manifest.capabilities.push(Capability::EventHooks);
        manifest
    }

    #[tokio::test]
    async fn test_runtime_manager_creation() {
        let manager = PluginRuntimeManager::new();

        // Broadcast should work even with no plugins
        let event = StateChangeEvent {
            old_state: ConnectionState::Disconnected,
            new_state: ConnectionState::Authenticating,
            timestamp: 0,
            network_id: None,
        };

        manager.broadcast_state_change(&event).await;
    }

    #[tokio::test]
    async fn test_policy_query_with_no_plugins() {
        let manager = PluginRuntimeManager::new();

        let ctx = PolicyContext {
            available_networks: vec![],
            available_exit_nodes: vec![],
            device_info: DeviceInfo {
                id: "test".to_string(),
                name: "Test Device".to_string(),
                platform: "linux".to_string(),
                virtual_ip: "100.64.0.1".to_string(),
                is_online: true,
                is_exit_node: false,
            },
            geo_location: None,
            time_of_day: 0,
            network_type: NetworkType::Wifi,
        };

        let result = manager.query_network_selection(&ctx).await;
        assert!(result.is_none());

        let decision = manager.query_connection_validation(&ctx).await;
        assert!(matches!(decision, PolicyDecision::Allow));
    }
}
