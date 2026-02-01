//! Hello World OmniEdge Plugin
//!
//! This is an example event plugin that demonstrates how to:
//! - Implement the base-plugin and event-plugin interfaces
//! - Use host functions for logging
//! - React to VPN lifecycle events
//!
//! ## Building
//!
//! ```bash
//! # Install the WASM component toolchain
//! cargo install cargo-component
//!
//! # Build the plugin
//! cargo component build --release
//! ```
//!
//! The compiled plugin will be at `target/wasm32-wasip1/release/hello_world_plugin.wasm`

// Generate bindings from the WIT interfaces
wit_bindgen::generate!({
    world: "event-plugin-world",
    path: "wit",
});

// Import types from the generated bindings
use omniedge::plugin::types::{
    Capability, ConnectionState, ConnectionStats, NetworkEvent, NetworkEventType, PeerInfo,
    PluginError, PluginManifest, StateChangeEvent,
};

// Import host logging functions
use omniedge::plugin::logging;

// Import the Guest traits from exports
use exports::omniedge::plugin::base_plugin::Guest as BasePluginGuest;
use exports::omniedge::plugin::event_plugin::Guest as EventPluginGuest;

/// The plugin implementation struct
struct HelloWorldPlugin;

impl BasePluginGuest for HelloWorldPlugin {
    fn manifest() -> PluginManifest {
        PluginManifest {
            id: "com.omniedge.hello-world".to_string(),
            name: "Hello World Plugin".to_string(),
            version: "0.1.0".to_string(),
            author: "OmniEdge Team".to_string(),
            description: "Example plugin that logs VPN lifecycle events".to_string(),
            capabilities: vec![Capability::EventHooks],
        }
    }

    fn on_load() -> Result<(), PluginError> {
        logging::log_info("[HelloWorld] Plugin loaded successfully!");
        logging::log_info("[HelloWorld] This plugin will log all VPN lifecycle events.");
        Ok(())
    }

    fn on_unload() -> Result<(), PluginError> {
        logging::log_info("[HelloWorld] Plugin unloading. Goodbye!");
        Ok(())
    }
}

impl EventPluginGuest for HelloWorldPlugin {
    fn on_state_change(event: StateChangeEvent) {
        let old_state = format_connection_state(&event.old_state);
        let new_state = format_connection_state(&event.new_state);

        logging::log_info(&format!(
            "[HelloWorld] State changed: {} -> {} (network: {:?})",
            old_state, new_state, event.network_id
        ));
    }

    fn on_peer_discovered(peer: PeerInfo) {
        logging::log_info(&format!(
            "[HelloWorld] Peer discovered! IP: {}, Endpoint: {:?}, RX: {} bytes, TX: {} bytes",
            peer.virtual_ip, peer.endpoint, peer.rx_bytes, peer.tx_bytes
        ));
    }

    fn on_peer_disconnected(peer: PeerInfo) {
        logging::log_warn(&format!(
            "[HelloWorld] Peer disconnected: {} (last handshake: {})",
            peer.virtual_ip, peer.last_handshake
        ));
    }

    fn on_network_change(event: NetworkEvent) {
        let event_type = match event.event_type {
            NetworkEventType::Joined => "Joined",
            NetworkEventType::Left => "Left",
            NetworkEventType::PeerAdded => "PeerAdded",
            NetworkEventType::PeerRemoved => "PeerRemoved",
        };

        logging::log_info(&format!(
            "[HelloWorld] Network event: {} on network {} at timestamp {}",
            event_type, event.network_id, event.timestamp
        ));
    }

    fn on_stats_update(stats: ConnectionStats) {
        logging::log_debug(&format!(
            "[HelloWorld] Stats: RX={} bytes, TX={} bytes, Peers={}, Uptime={}s, Latency={:?}ms",
            stats.rx_bytes,
            stats.tx_bytes,
            stats.connected_peers,
            stats.uptime_seconds,
            stats.latency_ms
        ));
    }
}

/// Helper function to format connection state for logging
fn format_connection_state(state: &ConnectionState) -> String {
    match state {
        ConnectionState::Disconnected => "Disconnected".to_string(),
        ConnectionState::Authenticating => "Authenticating".to_string(),
        ConnectionState::Authenticated => "Authenticated".to_string(),
        ConnectionState::Joining(network) => format!("Joining({})", network),
        ConnectionState::Connected(network) => format!("Connected({})", network),
    }
}

// Export the plugin implementation
export!(HelloWorldPlugin);
