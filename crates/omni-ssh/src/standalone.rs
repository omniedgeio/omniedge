//! Standalone SSH backend for running without OmniEdge cloud
//!
//! This module provides a `StandaloneSshBackend` that allows running the SSH server
//! independently of the OmniEdge network. It's useful for:
//!
//! - Testing and development
//! - Running SSH on a single machine
//! - Integration with other VPN solutions
//! - Embedded or IoT deployments
//!
//! ## Example
//!
//! ```rust,ignore
//! use omni_ssh::standalone::{StandaloneSshBackend, StandaloneConfig};
//! use omni_ssh::{SshServer, SshServerConfig};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Create standalone backend with permissive defaults
//!     let backend = StandaloneSshBackend::new(StandaloneConfig::permissive())?;
//!     
//!     // Or configure specifically
//!     let backend = StandaloneSshBackend::new(StandaloneConfig {
//!         device_id: "my-server".to_string(),
//!         network_id: "standalone".to_string(),
//!         allowed_networks: vec!["10.0.0.0/8".parse()?, "192.168.0.0/16".parse()?],
//!         allow_any_ip: false,
//!         ..Default::default()
//!     })?;
//!     
//!     let config = SshServerConfig::default();
//!     let server = SshServer::new(config, Arc::new(backend)).await?;
//!     server.start("0.0.0.0:2222".parse()?).await?;
//!     
//!     Ok(())
//! }
//! ```

use crate::server::{PeerIdentity, PeerInfo, SshBackend, SshEvent};
use crate::types::{NodeInfo, SshAction, SshPolicy, SshPrincipal, SshRule, UserProfile};
use async_trait::async_trait;
use ipnet::IpNet;
use russh_keys::key::KeyPair;
use std::collections::HashMap;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Configuration for standalone SSH backend
#[derive(Debug, Clone)]
pub struct StandaloneConfig {
    /// Device ID for this node (used in identity)
    pub device_id: String,
    /// Network ID (used in identity)
    pub network_id: String,
    /// Whether SSH server is enabled
    pub ssh_enabled: bool,
    /// Allow connections from any IP address
    pub allow_any_ip: bool,
    /// Allowed IP networks (if allow_any_ip is false)
    pub allowed_networks: Vec<IpNet>,
    /// Allowed individual IP addresses
    pub allowed_ips: Vec<IpAddr>,
    /// Path to host key files (will generate if not present)
    pub host_key_path: Option<PathBuf>,
    /// SSH user to local user mapping ("*" = same as SSH user, "=" = keep same)
    pub user_mapping: HashMap<String, String>,
    /// Default local user if no mapping found
    pub default_local_user: Option<String>,
    /// Whether to allow all SSH features (forwarding, SFTP, etc.)
    pub allow_all_features: bool,
    /// Custom SSH policy (overrides auto-generated one)
    pub custom_policy: Option<SshPolicy>,
    /// Log events to stdout
    pub log_events: bool,
}

impl Default for StandaloneConfig {
    fn default() -> Self {
        Self {
            device_id: format!(
                "standalone-{}",
                uuid::Uuid::new_v4()
                    .to_string()
                    .split('-')
                    .next()
                    .unwrap_or("0000")
            ),
            network_id: "standalone".to_string(),
            ssh_enabled: true,
            allow_any_ip: false,
            allowed_networks: vec![
                "10.0.0.0/8".parse().unwrap(),
                "172.16.0.0/12".parse().unwrap(),
                "192.168.0.0/16".parse().unwrap(),
                "127.0.0.0/8".parse().unwrap(),
            ],
            allowed_ips: Vec::new(),
            host_key_path: None,
            user_mapping: HashMap::new(),
            default_local_user: None,
            allow_all_features: true,
            custom_policy: None,
            log_events: true,
        }
    }
}

impl StandaloneConfig {
    /// Create a permissive configuration that accepts all connections
    pub fn permissive() -> Self {
        Self {
            allow_any_ip: true,
            allow_all_features: true,
            ..Default::default()
        }
    }

    /// Create a restrictive configuration (localhost only)
    pub fn localhost_only() -> Self {
        Self {
            allow_any_ip: false,
            allowed_networks: vec!["127.0.0.0/8".parse().unwrap()],
            allowed_ips: vec!["::1".parse().unwrap()],
            ..Default::default()
        }
    }

    /// Create configuration for a specific network
    pub fn for_network(network: IpNet) -> Self {
        Self {
            allow_any_ip: false,
            allowed_networks: vec![network],
            ..Default::default()
        }
    }

    /// Add an allowed network
    pub fn with_network(mut self, network: IpNet) -> Self {
        self.allowed_networks.push(network);
        self
    }

    /// Add an allowed IP
    pub fn with_ip(mut self, ip: IpAddr) -> Self {
        self.allowed_ips.push(ip);
        self
    }

    /// Set device ID
    pub fn with_device_id(mut self, id: impl Into<String>) -> Self {
        self.device_id = id.into();
        self
    }

    /// Set network ID
    pub fn with_network_id(mut self, id: impl Into<String>) -> Self {
        self.network_id = id.into();
        self
    }

    /// Set host key path
    pub fn with_host_key_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.host_key_path = Some(path.into());
        self
    }

    /// Add a user mapping (ssh_user -> local_user)
    pub fn with_user_mapping(
        mut self,
        ssh_user: impl Into<String>,
        local_user: impl Into<String>,
    ) -> Self {
        self.user_mapping.insert(ssh_user.into(), local_user.into());
        self
    }

    /// Set default local user
    pub fn with_default_user(mut self, user: impl Into<String>) -> Self {
        self.default_local_user = Some(user.into());
        self
    }

    /// Set custom policy
    pub fn with_policy(mut self, policy: SshPolicy) -> Self {
        self.custom_policy = Some(policy);
        self
    }

    /// Enable/disable event logging
    pub fn with_logging(mut self, enabled: bool) -> Self {
        self.log_events = enabled;
        self
    }
}

/// Standalone SSH backend that works without OmniEdge cloud
pub struct StandaloneSshBackend {
    config: StandaloneConfig,
    host_keys: Vec<KeyPair>,
    /// Known peers (can be populated manually)
    peers: RwLock<Vec<PeerInfo>>,
    /// Connection counter for generating synthetic identities
    connection_counter: AtomicU64,
    /// Event callback (optional)
    event_callback: Option<Arc<dyn Fn(SshEvent) + Send + Sync>>,
}

impl StandaloneSshBackend {
    /// Create a new standalone backend with the given configuration
    pub fn new(config: StandaloneConfig) -> anyhow::Result<Self> {
        let host_keys = Self::load_or_generate_keys(&config)?;

        info!(
            device_id = %config.device_id,
            network_id = %config.network_id,
            allow_any_ip = config.allow_any_ip,
            allowed_networks = ?config.allowed_networks.len(),
            "Standalone SSH backend initialized"
        );

        Ok(Self {
            config,
            host_keys,
            peers: RwLock::new(Vec::new()),
            connection_counter: AtomicU64::new(0),
            event_callback: None,
        })
    }

    /// Create with default permissive configuration
    pub fn permissive() -> anyhow::Result<Self> {
        Self::new(StandaloneConfig::permissive())
    }

    /// Create for localhost only
    pub fn localhost_only() -> anyhow::Result<Self> {
        Self::new(StandaloneConfig::localhost_only())
    }

    /// Set an event callback
    pub fn with_event_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(SshEvent) + Send + Sync + 'static,
    {
        self.event_callback = Some(Arc::new(callback));
        self
    }

    /// Add a known peer
    pub async fn add_peer(&self, peer: PeerInfo) {
        let mut peers = self.peers.write().await;
        // Remove existing peer with same IP
        peers.retain(|p| p.vpn_ip != peer.vpn_ip);
        peers.push(peer);
    }

    /// Remove a peer by IP
    pub async fn remove_peer(&self, ip: IpAddr) {
        let mut peers = self.peers.write().await;
        peers.retain(|p| p.vpn_ip != ip);
    }

    /// Clear all peers
    pub async fn clear_peers(&self) {
        let mut peers = self.peers.write().await;
        peers.clear();
    }

    /// Load or generate host keys
    fn load_or_generate_keys(config: &StandaloneConfig) -> anyhow::Result<Vec<KeyPair>> {
        if let Some(ref key_path) = config.host_key_path {
            // Try to load from files
            let ed25519_path = key_path.join("ssh_host_ed25519_key");
            let rsa_path = key_path.join("ssh_host_rsa_key");

            let mut keys = Vec::new();

            if ed25519_path.exists() {
                match russh_keys::load_secret_key(&ed25519_path, None) {
                    Ok(key) => {
                        info!("Loaded ED25519 host key from {:?}", ed25519_path);
                        keys.push(key);
                    }
                    Err(e) => {
                        warn!("Failed to load ED25519 key: {}", e);
                    }
                }
            }

            if rsa_path.exists() {
                match russh_keys::load_secret_key(&rsa_path, None) {
                    Ok(key) => {
                        info!("Loaded RSA host key from {:?}", rsa_path);
                        keys.push(key);
                    }
                    Err(e) => {
                        warn!("Failed to load RSA key: {}", e);
                    }
                }
            }

            if !keys.is_empty() {
                return Ok(keys);
            }

            // Generate and save keys
            info!("Generating new host keys at {:?}", key_path);
            std::fs::create_dir_all(key_path)?;

            let ed25519_key = KeyPair::generate_ed25519()
                .ok_or_else(|| anyhow::anyhow!("Failed to generate ED25519 key"))?;

            // Note: russh_keys doesn't have a direct save function, so we'll just use the generated keys
            // In a real implementation, you'd save the keys to disk

            Ok(vec![ed25519_key])
        } else {
            // Generate ephemeral keys
            info!("Generating ephemeral host keys (no persistence)");

            let ed25519_key = KeyPair::generate_ed25519()
                .ok_or_else(|| anyhow::anyhow!("Failed to generate ED25519 key"))?;

            Ok(vec![ed25519_key])
        }
    }

    /// Check if an IP is allowed
    fn is_ip_allowed(&self, addr: IpAddr) -> bool {
        if self.config.allow_any_ip {
            return true;
        }

        // Check individual IPs
        if self.config.allowed_ips.contains(&addr) {
            return true;
        }

        // Check networks
        for network in &self.config.allowed_networks {
            if network.contains(&addr) {
                return true;
            }
        }

        false
    }

    /// Generate a synthetic peer identity for an IP
    fn generate_identity(&self, addr: IpAddr) -> PeerIdentity {
        let conn_num = self.connection_counter.fetch_add(1, Ordering::Relaxed);

        PeerIdentity {
            node: NodeInfo {
                id: format!("standalone-node-{}", conn_num),
                name: format!("peer-{}", addr),
                virtual_ip: addr.to_string(),
                tags: vec!["standalone".to_string()],
                online: true,
                network_id: self.config.network_id.clone(),
            },
            user: UserProfile {
                id: format!("standalone-user-{}", conn_num),
                email: format!("user-{}@standalone.local", conn_num),
                name: Some(format!("Standalone User {}", conn_num)),
            },
        }
    }

    /// Generate the SSH policy based on configuration
    fn generate_policy(&self) -> SshPolicy {
        if let Some(ref custom) = self.config.custom_policy {
            return custom.clone();
        }

        // Build user mapping
        let mut ssh_users = self.config.user_mapping.clone();

        // Add wildcard mapping if not present
        if !ssh_users.contains_key("*") {
            if let Some(ref default_user) = self.config.default_local_user {
                ssh_users.insert("*".to_string(), default_user.clone());
            } else {
                // Map to same user (e.g., ssh user "root" -> local user "root")
                ssh_users.insert("*".to_string(), "=".to_string());
            }
        }

        let action = SshAction {
            accept: true,
            reject: false,
            message: None,
            allow_agent_forwarding: self.config.allow_all_features,
            allow_local_port_forwarding: self.config.allow_all_features,
            allow_remote_port_forwarding: self.config.allow_all_features,
            allow_sftp: self.config.allow_all_features,
            session_duration: None,
            record_session: false,
            recorders: Vec::new(),
            on_recording_failure: None,
            hold_and_delegate: None,
            allowed_commands: None,
            blocked_commands: None,
            allowed_paths: None,
            read_only: false,
            time_restrictions: None,
        };

        let rule = SshRule {
            id: "standalone-allow-all".to_string(),
            principals: vec![SshPrincipal {
                any: true,
                ..Default::default()
            }],
            ssh_users,
            action,
            accept_env: vec!["LANG".to_string(), "LC_*".to_string(), "TERM".to_string()],
            expires: None,
        };

        SshPolicy {
            version: 1,
            updated_at: chrono::Utc::now(),
            rules: vec![rule],
        }
    }
}

#[async_trait]
impl SshBackend for StandaloneSshBackend {
    async fn get_host_keys(&self) -> anyhow::Result<Vec<KeyPair>> {
        Ok(self.host_keys.clone())
    }

    fn ssh_enabled(&self) -> bool {
        self.config.ssh_enabled
    }

    async fn who_is(&self, addr: IpAddr) -> anyhow::Result<Option<PeerIdentity>> {
        if !self.is_ip_allowed(addr) {
            debug!(ip = %addr, "IP not allowed, returning None for who_is");
            return Ok(None);
        }

        // Check if we have a known peer
        let peers = self.peers.read().await;
        for peer in peers.iter() {
            if peer.vpn_ip == addr {
                return Ok(Some(PeerIdentity {
                    node: NodeInfo {
                        id: peer
                            .device_id
                            .clone()
                            .unwrap_or_else(|| format!("peer-{}", addr)),
                        name: peer.name.clone(),
                        virtual_ip: addr.to_string(),
                        tags: Vec::new(),
                        online: peer.online,
                        network_id: self.config.network_id.clone(),
                    },
                    user: UserProfile {
                        id: format!("user-{}", addr),
                        email: format!("{}@standalone.local", peer.name),
                        name: Some(peer.name.clone()),
                    },
                }));
            }
        }
        drop(peers);

        // Generate synthetic identity
        Ok(Some(self.generate_identity(addr)))
    }

    async fn get_ssh_policy(&self) -> anyhow::Result<SshPolicy> {
        Ok(self.generate_policy())
    }

    async fn on_ssh_event(&self, event: SshEvent) {
        if self.config.log_events {
            match &event {
                SshEvent::ConnectionAttempt { src, dst } => {
                    info!(src = %src, dst = %dst, "SSH connection attempt");
                }
                SshEvent::ConnectionAuthenticated {
                    conn_id,
                    ssh_user,
                    local_user,
                } => {
                    info!(conn_id = %conn_id, ssh_user = %ssh_user, local_user = %local_user, "SSH connection authenticated");
                }
                SshEvent::ConnectionRejected { src, reason } => {
                    warn!(src = %src, reason = %reason, "SSH connection rejected");
                }
                SshEvent::ConnectionClosed { conn_id } => {
                    info!(conn_id = %conn_id, "SSH connection closed");
                }
                SshEvent::SessionStarted {
                    conn_id,
                    session_type,
                } => {
                    info!(conn_id = %conn_id, session_type = %session_type, "SSH session started");
                }
                SshEvent::SessionEnded { conn_id } => {
                    info!(conn_id = %conn_id, "SSH session ended");
                }
                _ => {
                    debug!(?event, "SSH event");
                }
            }
        }

        if let Some(ref callback) = self.event_callback {
            callback(event);
        }
    }

    fn is_omniedge_ip(&self, addr: IpAddr) -> bool {
        self.is_ip_allowed(addr)
    }

    fn device_id(&self) -> &str {
        &self.config.device_id
    }

    fn network_id(&self) -> &str {
        &self.config.network_id
    }

    async fn resolve_peer_name(&self, name: &str) -> anyhow::Result<Option<IpAddr>> {
        let peers = self.peers.read().await;

        // Exact match
        for peer in peers.iter() {
            if peer.name.eq_ignore_ascii_case(name) {
                return Ok(Some(peer.vpn_ip));
            }
        }

        // Partial match
        for peer in peers.iter() {
            if peer.name.to_lowercase().contains(&name.to_lowercase()) {
                return Ok(Some(peer.vpn_ip));
            }
        }

        Ok(None)
    }

    async fn list_peers(&self) -> anyhow::Result<Vec<PeerInfo>> {
        let peers = self.peers.read().await;
        Ok(peers.clone())
    }
}

/// Builder for creating a standalone SSH server
pub struct StandaloneSshServerBuilder {
    config: StandaloneConfig,
    server_config: Option<crate::server::SshServerConfig>,
}

impl StandaloneSshServerBuilder {
    /// Create a new builder with default configuration
    pub fn new() -> Self {
        Self {
            config: StandaloneConfig::default(),
            server_config: None,
        }
    }

    /// Use permissive configuration
    pub fn permissive(mut self) -> Self {
        self.config = StandaloneConfig::permissive();
        self
    }

    /// Use localhost-only configuration
    pub fn localhost_only(mut self) -> Self {
        self.config = StandaloneConfig::localhost_only();
        self
    }

    /// Set device ID
    pub fn device_id(mut self, id: impl Into<String>) -> Self {
        self.config.device_id = id.into();
        self
    }

    /// Set network ID
    pub fn network_id(mut self, id: impl Into<String>) -> Self {
        self.config.network_id = id.into();
        self
    }

    /// Add allowed network
    pub fn allow_network(mut self, network: IpNet) -> Self {
        self.config.allowed_networks.push(network);
        self
    }

    /// Allow any IP
    pub fn allow_any_ip(mut self) -> Self {
        self.config.allow_any_ip = true;
        self
    }

    /// Set host key path
    pub fn host_key_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.host_key_path = Some(path.into());
        self
    }

    /// Add user mapping
    pub fn map_user(mut self, ssh_user: impl Into<String>, local_user: impl Into<String>) -> Self {
        self.config
            .user_mapping
            .insert(ssh_user.into(), local_user.into());
        self
    }

    /// Set default local user
    pub fn default_user(mut self, user: impl Into<String>) -> Self {
        self.config.default_local_user = Some(user.into());
        self
    }

    /// Set server configuration
    pub fn server_config(mut self, config: crate::server::SshServerConfig) -> Self {
        self.server_config = Some(config);
        self
    }

    /// Enable/disable event logging
    pub fn log_events(mut self, enabled: bool) -> Self {
        self.config.log_events = enabled;
        self
    }

    /// Build the backend (consumes config, keeps server_config)
    pub fn build_backend(&self) -> anyhow::Result<StandaloneSshBackend> {
        StandaloneSshBackend::new(self.config.clone())
    }

    /// Build and start the server
    pub async fn build_and_start(self, bind_addr: std::net::SocketAddr) -> anyhow::Result<()> {
        let backend = Arc::new(StandaloneSshBackend::new(self.config)?);
        let server_config = self.server_config.unwrap_or_default();

        let server = crate::server::SshServer::new(server_config, backend).await?;
        server.start(bind_addr).await
    }
}

impl Default for StandaloneSshServerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standalone_config_default() {
        let config = StandaloneConfig::default();
        assert!(config.ssh_enabled);
        assert!(!config.allow_any_ip);
        assert!(!config.allowed_networks.is_empty());
    }

    #[test]
    fn test_standalone_config_permissive() {
        let config = StandaloneConfig::permissive();
        assert!(config.allow_any_ip);
        assert!(config.allow_all_features);
    }

    #[test]
    fn test_standalone_config_localhost() {
        let config = StandaloneConfig::localhost_only();
        assert!(!config.allow_any_ip);
        assert_eq!(config.allowed_networks.len(), 1);
        assert!(config.allowed_networks[0].contains(&"127.0.0.1".parse::<IpAddr>().unwrap()));
    }

    #[test]
    fn test_ip_allowed_check() {
        let backend = StandaloneSshBackend::new(StandaloneConfig::default()).unwrap();

        // Private IPs should be allowed by default
        assert!(backend.is_ip_allowed("10.0.0.1".parse().unwrap()));
        assert!(backend.is_ip_allowed("192.168.1.1".parse().unwrap()));
        assert!(backend.is_ip_allowed("172.16.0.1".parse().unwrap()));
        assert!(backend.is_ip_allowed("127.0.0.1".parse().unwrap()));

        // Public IPs should not be allowed by default
        assert!(!backend.is_ip_allowed("8.8.8.8".parse().unwrap()));
    }

    #[test]
    fn test_ip_allowed_permissive() {
        let backend = StandaloneSshBackend::permissive().unwrap();

        // All IPs should be allowed
        assert!(backend.is_ip_allowed("8.8.8.8".parse().unwrap()));
        assert!(backend.is_ip_allowed("1.2.3.4".parse().unwrap()));
    }

    #[tokio::test]
    async fn test_who_is_generates_identity() {
        let backend = StandaloneSshBackend::permissive().unwrap();

        let identity = backend.who_is("10.0.0.1".parse().unwrap()).await.unwrap();
        assert!(identity.is_some());

        let identity = identity.unwrap();
        assert_eq!(identity.node.virtual_ip, "10.0.0.1");
        assert!(identity.node.online);
    }

    #[tokio::test]
    async fn test_who_is_returns_none_for_disallowed() {
        let backend = StandaloneSshBackend::localhost_only().unwrap();

        // Public IP should return None
        let identity = backend.who_is("8.8.8.8".parse().unwrap()).await.unwrap();
        assert!(identity.is_none());

        // Localhost should work
        let identity = backend.who_is("127.0.0.1".parse().unwrap()).await.unwrap();
        assert!(identity.is_some());
    }

    #[tokio::test]
    async fn test_add_and_resolve_peer() {
        let backend = StandaloneSshBackend::permissive().unwrap();

        backend
            .add_peer(PeerInfo {
                name: "test-server".to_string(),
                vpn_ip: "10.0.0.5".parse().unwrap(),
                online: true,
                device_id: Some("dev-123".to_string()),
            })
            .await;

        let resolved = backend.resolve_peer_name("test-server").await.unwrap();
        assert_eq!(resolved, Some("10.0.0.5".parse().unwrap()));

        // Partial match
        let resolved = backend.resolve_peer_name("test").await.unwrap();
        assert_eq!(resolved, Some("10.0.0.5".parse().unwrap()));
    }

    #[tokio::test]
    async fn test_policy_generation() {
        let backend = StandaloneSshBackend::new(
            StandaloneConfig::default()
                .with_user_mapping("admin", "root")
                .with_default_user("nobody"),
        )
        .unwrap();

        let policy = backend.get_ssh_policy().await.unwrap();
        assert_eq!(policy.rules.len(), 1);

        let rule = &policy.rules[0];
        assert!(rule.action.accept);
        assert!(rule.ssh_users.contains_key("admin"));
        assert_eq!(rule.ssh_users.get("admin"), Some(&"root".to_string()));
    }

    #[test]
    fn test_builder() {
        let builder = StandaloneSshServerBuilder::new()
            .permissive()
            .device_id("my-server")
            .network_id("my-network")
            .map_user("admin", "root")
            .default_user("guest")
            .log_events(false);

        let backend = builder.build_backend().unwrap();
        assert_eq!(backend.device_id(), "my-server");
        assert_eq!(backend.network_id(), "my-network");
    }
}
