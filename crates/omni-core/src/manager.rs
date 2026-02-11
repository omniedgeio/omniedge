use crate::config::{CliConfig, NetworkConfig};
use crate::state::ConnectionState;
use anyhow::{Context, Result};
use log::{debug, error, info, trace, warn};
use omni_api::{types::*, ApiClient, AuthService, DeviceService, NetworkService};
use omni_proto::{
    encode_disco_ping,
    encode_disco_pong,
    // Relay protocol
    encode_relay_bind,
    handle_nucleus_message,
    is_relay_message,
    parse_disco_ping,
    parse_disco_pong,
    parse_relay_bind_ack,
    parse_relay_data,
    DiscoPing,
    DiscoPong,
    EndpointInfo,
    // Multi-endpoint support
    EndpointSet,
    EndpointSource,
    // NAT type detection
    NatType,
    NucleusState,
    OmniProto,
    PeerConnectionState,
    PortMapCapabilities,
    // Port mapping
    PortMapper,
    PortMapping,
    RelayClient,
    MSG_RELAY_BIND_ACK,
    MSG_RELAY_DATA,
    SIGNALING_DISCO_PING,
    SIGNALING_DISCO_PONG,
};
use omni_tun::{OmniTun, WgMode};
use omninervous::Identity;
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};
use tokio::task::JoinHandle;

// ============================================================================
// P2P Connection State Tracking (NAT Traversal Fix v0.3.2)
// ============================================================================

/// Connection strategy based on NAT type detection
///
/// This determines how we attempt to establish a connection with a peer,
/// based on both our NAT type and the peer's NAT type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConnectionStrategy {
    /// Standard direct connection attempt via disco ping/pong
    /// Used when at least one side has favorable NAT (Open, FullCone, or RestrictedCone)
    #[default]
    Direct,
    /// Both sides send disco pings simultaneously
    /// Used for Restricted-Restricted or PortRestricted-PortRestricted NAT pairs
    SimultaneousOpen,
    /// Attempt port prediction for symmetric NAT
    /// Used when one side is Symmetric but the other is not
    PortPrediction,
    /// Skip direct connection attempts, go straight to relay
    /// Used for Symmetric-Symmetric NAT pairs where direct P2P is nearly impossible
    RelayOnly,
}

impl ConnectionStrategy {
    /// Get a human-readable description of the strategy
    pub fn description(&self) -> &'static str {
        match self {
            Self::Direct => "Direct (standard disco)",
            Self::SimultaneousOpen => "Simultaneous Open (both sides ping)",
            Self::PortPrediction => "Port Prediction (symmetric NAT workaround)",
            Self::RelayOnly => "Relay Only (skip direct attempts)",
        }
    }
}

/// Select the best connection strategy based on NAT types
///
/// Strategy matrix:
/// | Our NAT          | Peer NAT         | Strategy           |
/// |------------------|------------------|--------------------|
/// | Open/FullCone    | Any              | Direct             |
/// | Any              | Open/FullCone    | Direct             |
/// | Restricted       | Restricted       | SimultaneousOpen   |
/// | PortRestricted   | PortRestricted   | SimultaneousOpen   |
/// | Symmetric        | Symmetric        | RelayOnly          |
/// | Symmetric        | Other            | PortPrediction     |
/// | Other            | Symmetric        | PortPrediction     |
/// | Unknown          | Unknown          | Direct (optimistic)|
pub fn select_connection_strategy(
    our_nat: Option<NatType>,
    peer_nat: Option<NatType>,
) -> ConnectionStrategy {
    match (our_nat, peer_nat) {
        // If either side has no NAT or easy NAT, direct connection works
        (Some(NatType::Open), _) | (_, Some(NatType::Open)) => ConnectionStrategy::Direct,
        (Some(NatType::FullCone), _) | (_, Some(NatType::FullCone)) => ConnectionStrategy::Direct,

        // Both sides are symmetric - relay is the only reliable option
        (Some(NatType::Symmetric), Some(NatType::Symmetric)) => ConnectionStrategy::RelayOnly,

        // One side is symmetric - try port prediction
        (Some(NatType::Symmetric), _) | (_, Some(NatType::Symmetric)) => {
            ConnectionStrategy::PortPrediction
        }

        // Both sides are restricted - simultaneous open can work
        (Some(NatType::RestrictedCone), Some(NatType::RestrictedCone)) => {
            ConnectionStrategy::SimultaneousOpen
        }
        (Some(NatType::PortRestrictedCone), Some(NatType::PortRestrictedCone)) => {
            ConnectionStrategy::SimultaneousOpen
        }

        // Mixed restricted types - simultaneous open is still worth trying
        (Some(NatType::RestrictedCone), Some(NatType::PortRestrictedCone))
        | (Some(NatType::PortRestrictedCone), Some(NatType::RestrictedCone)) => {
            ConnectionStrategy::SimultaneousOpen
        }

        // Unknown NAT types - be optimistic and try direct
        _ => ConnectionStrategy::Direct,
    }
}

/// Tracks the state of a pending disco ping
#[derive(Debug, Clone)]
pub struct PendingDiscoPing {
    /// Transaction ID (12 bytes)
    pub tx_id: [u8; 12],
    /// Target endpoint we sent the ping to
    pub target: std::net::SocketAddr,
    /// Target VIP
    pub target_vip: Ipv4Addr,
    /// When the ping was sent
    pub sent_at: Instant,
    /// Number of retries attempted
    pub retries: u32,
    /// Maximum retries before relay fallback
    pub max_retries: u32,
}

/// Tracks the state of a peer connection during establishment
#[derive(Debug, Clone)]
pub struct PeerState {
    /// Peer's virtual IP
    pub vip: Ipv4Addr,
    /// Peer's IPv6 virtual IP (if dual-stack)
    pub vip_v6: Option<std::net::Ipv6Addr>,
    /// Peer's public key
    pub public_key: [u8; 32],
    /// All known endpoints for this peer (multi-path support)
    pub endpoints: EndpointSet,
    /// Connection state
    pub state: PeerConnectionState,
    /// When the peer was discovered
    pub discovered_at: Instant,
    /// Last successful connectivity check
    pub last_seen: Option<Instant>,
    /// Whether we're using relay for this peer
    pub using_relay: bool,
    /// Peer's detected NAT type (if known from signaling)
    pub peer_nat_type: Option<NatType>,
    /// Connection strategy based on NAT types
    pub connection_strategy: ConnectionStrategy,
}

impl PeerState {
    pub fn new(
        vip: Ipv4Addr,
        vip_v6: Option<std::net::Ipv6Addr>,
        public_key: [u8; 32],
        endpoint: Option<std::net::SocketAddr>,
    ) -> Self {
        let mut endpoints = EndpointSet::new();
        if let Some(addr) = endpoint {
            endpoints.upsert(addr, EndpointSource::Nucleus);
        }
        Self {
            vip,
            vip_v6,
            public_key,
            endpoints,
            state: PeerConnectionState::Init,
            discovered_at: Instant::now(),
            last_seen: None,
            using_relay: false,
            peer_nat_type: None,
            connection_strategy: ConnectionStrategy::Direct,
        }
    }

    /// Create a new PeerState with NAT-aware strategy selection
    ///
    /// This constructor calculates the optimal connection strategy based on
    /// our NAT type and the peer's NAT type (if known).
    pub fn with_nat_strategy(
        vip: Ipv4Addr,
        vip_v6: Option<std::net::Ipv6Addr>,
        public_key: [u8; 32],
        endpoint: Option<std::net::SocketAddr>,
        our_nat_type: Option<NatType>,
        peer_nat_type: Option<NatType>,
    ) -> Self {
        let strategy = select_connection_strategy(our_nat_type, peer_nat_type);
        let mut endpoints = EndpointSet::new();
        if let Some(addr) = endpoint {
            endpoints.upsert(addr, EndpointSource::Nucleus);
        }
        Self {
            vip,
            vip_v6,
            public_key,
            endpoints,
            state: PeerConnectionState::Init,
            discovered_at: Instant::now(),
            last_seen: None,
            using_relay: false,
            peer_nat_type,
            connection_strategy: strategy,
        }
    }

    /// Update connection strategy based on new NAT information
    pub fn update_strategy(
        &mut self,
        our_nat_type: Option<NatType>,
        peer_nat_type: Option<NatType>,
    ) {
        self.peer_nat_type = peer_nat_type;
        self.connection_strategy = select_connection_strategy(our_nat_type, peer_nat_type);
    }

    /// Add an endpoint from a specific source
    pub fn add_endpoint(&mut self, addr: std::net::SocketAddr, source: EndpointSource) {
        self.endpoints.upsert(addr, source);
    }

    /// Record a successful pong response and update best endpoint
    pub fn record_pong(&mut self, addr: std::net::SocketAddr, rtt: Duration) {
        self.endpoints.record_pong(addr, rtt);
        self.last_seen = Some(Instant::now());

        // Update connection state based on endpoint state
        if self.endpoints.has_working_connection() {
            if self
                .endpoints
                .best()
                .map(|e| e.source == EndpointSource::Relay)
                .unwrap_or(false)
            {
                self.state = PeerConnectionState::RelayOk;
                self.using_relay = true;
            } else {
                self.state = PeerConnectionState::DirectOk;
                self.using_relay = false;
            }
        }
    }

    /// Mark an endpoint as failed
    pub fn mark_endpoint_failed(&mut self, addr: std::net::SocketAddr) {
        self.endpoints.mark_failed(addr);

        // Check if we need to fall back to relay
        if self.endpoints.needs_relay() {
            self.state = PeerConnectionState::RelayTry;
        }
    }

    /// Mark peer as using relay (legacy compatibility)
    pub fn mark_relayed(&mut self) {
        self.state = PeerConnectionState::RelayOk;
        self.using_relay = true;
        self.last_seen = Some(Instant::now());
    }

    /// Get the best endpoint to use
    pub fn best_endpoint(&self) -> Option<std::net::SocketAddr> {
        self.endpoints.best_addr()
    }

    /// Get the best endpoint info (for latency display)
    pub fn best_endpoint_info(&self) -> Option<&EndpointInfo> {
        self.endpoints.best()
    }

    /// Get all endpoints that need probing
    pub fn endpoints_needing_probe(&self, interval: Duration) -> Vec<std::net::SocketAddr> {
        self.endpoints.endpoints_needing_probe(interval)
    }

    /// Mark an endpoint as being probed
    pub fn mark_probing(&mut self, addr: std::net::SocketAddr) {
        self.endpoints.mark_probing(addr);
    }

    /// Get endpoint count
    pub fn endpoint_count(&self) -> usize {
        self.endpoints.endpoints.len()
    }

    /// Check if peer has any working connection
    pub fn has_working_connection(&self) -> bool {
        self.endpoints.has_working_connection()
    }
}

/// Configuration for disco probing
#[derive(Debug, Clone)]
pub struct LocalDiscoConfig {
    /// Timeout for each ping attempt
    pub ping_timeout: Duration,
    /// Number of retries before giving up
    pub max_retries: u32,
    /// Whether to use relay fallback
    pub relay_enabled: bool,
}

impl Default for LocalDiscoConfig {
    fn default() -> Self {
        Self {
            ping_timeout: Duration::from_secs(5),
            max_retries: 3,
            relay_enabled: true,
        }
    }
}

/// Debug info for a single peer connection
#[derive(Debug, Clone, serde::Serialize)]
pub struct PeerDebugInfo {
    pub vip: String,
    pub vip_v6: Option<String>,
    pub state: String,
    /// Best endpoint address
    pub best_endpoint: Option<String>,
    /// Best endpoint latency in ms
    pub best_latency_ms: Option<u64>,
    /// Total number of known endpoints
    pub endpoint_count: usize,
    /// Number of working endpoints
    pub working_endpoint_count: usize,
    pub using_relay: bool,
    pub last_seen_ago_secs: Option<u64>,
    /// Peer's NAT type (if known)
    pub peer_nat_type: Option<String>,
    /// Connection strategy being used
    pub connection_strategy: String,
}

/// Comprehensive connection debug information
#[derive(Debug, Clone, serde::Serialize)]
pub struct ConnectionDebugInfo {
    pub total_peers: usize,
    pub connected_peers: usize,
    pub probing_peers: usize,
    pub failed_peers: usize,
    pub relayed_peers: usize,
    pub pending_pings: usize,
    pub peers: Vec<PeerDebugInfo>,
}

pub struct ConnectionManager {
    state: Arc<RwLock<ConnectionState>>,
    api_client: Option<ApiClient>,
    proto: Option<Arc<OmniProto>>,
    tun: Option<OmniTun>,
    identity: Identity,
    base_url: String,
    is_nucleus: bool,
    nucleus_state: Option<Arc<Mutex<NucleusState>>>,
    nucleus_port: u16,
    as_exit_node: Arc<AtomicBool>,
    exit_node_ip: Option<String>,
    /// IPv6 address of the selected exit node (dual-stack support)
    exit_node_ip_v6: Option<String>,
    cluster_secret: Option<String>,
    device_id: Option<String>,
    /// Hardware ID used for heartbeats (differs from device_id which is API-assigned UUID)
    hardware_id: Option<String>,
    current_network_id: Arc<RwLock<Option<String>>>,
    virtual_ip: Arc<RwLock<Option<String>>>,
    /// IPv6 virtual IP address (dual-stack support)
    virtual_ip_v6: Arc<RwLock<Option<String>>>,
    heartbeat_tx: Option<mpsc::Sender<()>>,
    shutdown_tx: Option<broadcast::Sender<()>>,
    task_handles: Vec<JoinHandle<()>>,
    /// Network configuration for NAT traversal (v0.3.0+)
    network_config: NetworkConfig,
    // ========================================================================
    // P2P Connection State Tracking (NAT Traversal Fix v0.3.2)
    // ========================================================================
    /// Peer connection states by VIP
    peer_states: Arc<RwLock<HashMap<Ipv4Addr, PeerState>>>,
    /// Pending disco pings awaiting pong responses (keyed by transaction ID)
    pending_pings: Arc<RwLock<HashMap<[u8; 12], PendingDiscoPing>>>,
    /// Disco configuration
    disco_config: LocalDiscoConfig,
    // ========================================================================
    // Relay Fallback (NAT Traversal Fix v0.3.3)
    // ========================================================================
    /// Relay client for fallback when disco fails
    relay_client: Arc<RwLock<Option<omni_proto::RelayClient>>>,
    /// Active relay sessions by peer public key
    relay_sessions: Arc<RwLock<HashMap<[u8; 32], omni_proto::SessionId>>>,
    // ========================================================================
    // Port Mapping (NAT Traversal Fix v0.3.4)
    // ========================================================================
    /// Port mapper for NAT-PMP/UPnP/PCP
    port_mapper: Arc<RwLock<Option<PortMapper>>>,
    /// Current port mapping (if active)
    port_mapping: Arc<RwLock<Option<PortMapping>>>,
}

impl ConnectionManager {
    pub fn new(base_url: String, private_key: Option<[u8; 32]>) -> Self {
        let identity = if let Some(pk) = private_key {
            Identity::from_private_key(pk)
        } else {
            Identity::generate()
        };

        // Load network config from CLI config, with defaults if not available
        let (is_exit_node, network_config) = CliConfig::load()
            .map(|c| (c.is_exit_node, c.network_config))
            .unwrap_or_else(|_| (false, NetworkConfig::default()));

        // Initialize disco config from network config
        let disco_config = LocalDiscoConfig {
            relay_enabled: network_config.relay_enabled,
            ..LocalDiscoConfig::default()
        };

        Self {
            state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            api_client: None,
            proto: None,
            tun: None,
            identity,
            base_url,
            is_nucleus: false,
            nucleus_state: None,
            nucleus_port: 51820, // Default nucleus signaling port
            as_exit_node: Arc::new(AtomicBool::new(is_exit_node)),
            exit_node_ip: None,
            exit_node_ip_v6: None,
            cluster_secret: None,
            device_id: None,
            hardware_id: None,
            current_network_id: Arc::new(RwLock::new(None)),
            virtual_ip: Arc::new(RwLock::new(None)),
            virtual_ip_v6: Arc::new(RwLock::new(None)),
            heartbeat_tx: None,
            shutdown_tx: None,
            task_handles: Vec::new(),
            network_config,
            // P2P Connection State Tracking
            peer_states: Arc::new(RwLock::new(HashMap::new())),
            pending_pings: Arc::new(RwLock::new(HashMap::new())),
            disco_config,
            // Relay Fallback
            relay_client: Arc::new(RwLock::new(None)),
            relay_sessions: Arc::new(RwLock::new(HashMap::new())),
            // Port Mapping
            port_mapper: Arc::new(RwLock::new(None)),
            port_mapping: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn get_state(&self) -> ConnectionState {
        self.state.read().await.clone()
    }

    pub fn get_state_handle(&self) -> Arc<RwLock<ConnectionState>> {
        self.state.clone()
    }

    pub fn get_network_id_handle(&self) -> Arc<RwLock<Option<String>>> {
        self.current_network_id.clone()
    }

    pub fn get_virtual_ip_handle(&self) -> Arc<RwLock<Option<String>>> {
        self.virtual_ip.clone()
    }

    /// Get the IPv6 virtual IP handle (dual-stack support)
    pub fn get_virtual_ip_v6_handle(&self) -> Arc<RwLock<Option<String>>> {
        self.virtual_ip_v6.clone()
    }

    pub fn get_as_exit_node_handle(&self) -> Arc<AtomicBool> {
        self.as_exit_node.clone()
    }

    /// Get the network configuration for NAT traversal
    pub fn get_network_config(&self) -> &NetworkConfig {
        &self.network_config
    }

    /// Get a mutable reference to the network configuration
    pub fn get_network_config_mut(&mut self) -> &mut NetworkConfig {
        &mut self.network_config
    }

    /// Get access to the underlying OmniProto instance (if connected)
    pub fn get_proto(&self) -> Option<Arc<OmniProto>> {
        self.proto.clone()
    }

    // ========================================================================
    // P2P Connection State Queries (NAT Traversal Fix v0.3.2)
    // ========================================================================

    /// Get all peer connection states
    pub async fn get_peer_states(&self) -> Vec<PeerState> {
        let peers = self.peer_states.read().await;
        peers.values().cloned().collect()
    }

    /// Get connection state for a specific peer by VIP
    pub async fn get_peer_state(&self, vip: Ipv4Addr) -> Option<PeerState> {
        let peers = self.peer_states.read().await;
        peers.get(&vip).cloned()
    }

    /// Get count of connected peers (successful disco handshake)
    pub async fn get_connected_peer_count(&self) -> usize {
        let peers = self.peer_states.read().await;
        peers
            .values()
            .filter(|p| p.state == PeerConnectionState::DirectOk)
            .count()
    }

    /// Get count of peers using relay
    pub async fn get_relayed_peer_count(&self) -> usize {
        let peers = self.peer_states.read().await;
        peers.values().filter(|p| p.using_relay).count()
    }

    /// Get pending disco ping count
    pub async fn get_pending_ping_count(&self) -> usize {
        let pings = self.pending_pings.read().await;
        pings.len()
    }

    /// Get detailed connection status for debugging
    pub async fn get_connection_debug_info(&self) -> ConnectionDebugInfo {
        let peers = self.peer_states.read().await;
        let pings = self.pending_pings.read().await;

        let mut peer_info: Vec<PeerDebugInfo> = peers
            .values()
            .map(|p| {
                let best_info = p.best_endpoint_info();
                let responsive_timeout = Duration::from_secs(30);
                PeerDebugInfo {
                    vip: p.vip.to_string(),
                    vip_v6: p.vip_v6.map(|v| v.to_string()),
                    state: format!("{:?}", p.state),
                    best_endpoint: p.best_endpoint().map(|e| e.to_string()),
                    best_latency_ms: best_info
                        .and_then(|e| e.latency.map(|l| l.as_millis() as u64)),
                    endpoint_count: p.endpoint_count(),
                    working_endpoint_count: p.endpoints.responsive_count(responsive_timeout),
                    using_relay: p.using_relay,
                    last_seen_ago_secs: p.last_seen.map(|t| t.elapsed().as_secs()),
                    peer_nat_type: p.peer_nat_type.map(|n| format!("{:?}", n)),
                    connection_strategy: p.connection_strategy.description().to_string(),
                }
            })
            .collect();

        // Sort by VIP for consistent output
        peer_info.sort_by(|a, b| a.vip.cmp(&b.vip));

        ConnectionDebugInfo {
            total_peers: peers.len(),
            connected_peers: peers
                .values()
                .filter(|p| p.state == PeerConnectionState::DirectOk)
                .count(),
            probing_peers: peers
                .values()
                .filter(|p| p.state == PeerConnectionState::DirectTry)
                .count(),
            failed_peers: peers
                .values()
                .filter(|p| p.state == PeerConnectionState::Failed)
                .count(),
            relayed_peers: peers.values().filter(|p| p.using_relay).count(),
            pending_pings: pings.len(),
            peers: peer_info,
        }
    }

    // ========================================================================
    // Relay State Queries (NAT Traversal Fix v0.3.3)
    // ========================================================================

    /// Check if relay is enabled in configuration
    pub fn is_relay_enabled(&self) -> bool {
        self.disco_config.relay_enabled
    }

    /// Get count of active relay sessions
    pub async fn get_relay_session_count(&self) -> usize {
        let sessions = self.relay_sessions.read().await;
        sessions.len()
    }

    /// Check if a specific peer is using relay
    pub async fn is_peer_using_relay(&self, vip: Ipv4Addr) -> bool {
        let peers = self.peer_states.read().await;
        peers.get(&vip).map(|p| p.using_relay).unwrap_or(false)
    }

    // ========================================================================
    // Port Mapping (NAT Traversal Fix v0.3.4)
    // ========================================================================

    /// Check if port mapping is enabled in configuration
    pub fn is_portmap_enabled(&self) -> bool {
        self.network_config.portmap_enabled
    }

    /// Get current port mapping status
    pub async fn get_port_mapping(&self) -> Option<PortMapping> {
        self.port_mapping.read().await.clone()
    }

    /// Get the external port if we have an active mapping
    pub async fn get_external_port(&self) -> Option<u16> {
        self.port_mapping
            .read()
            .await
            .as_ref()
            .map(|m| m.external_port)
    }

    /// Initialize port mapper and probe for capabilities
    ///
    /// This should be called after binding the UDP socket to know the internal port.
    /// Returns the capabilities found (NAT-PMP, UPnP, PCP support).
    pub async fn init_port_mapper(&self, internal_port: u16) -> Result<PortMapCapabilities> {
        if !self.network_config.portmap_enabled {
            return Ok(PortMapCapabilities::default());
        }

        info!(
            "Initializing port mapper for internal port {}",
            internal_port
        );

        let mut mapper = PortMapper::new(internal_port);
        let caps = mapper.probe().await?;

        info!(
            "Port mapping capabilities: NAT-PMP={}, UPnP={}, PCP={}, gateway={:?}, external={:?}",
            caps.nat_pmp, caps.upnp, caps.pcp, caps.gateway_addr, caps.external_addr
        );

        // Store the mapper
        let mut pm = self.port_mapper.write().await;
        *pm = Some(mapper);

        Ok(caps)
    }

    /// Request a port mapping using the best available protocol
    ///
    /// Returns the external port if successful.
    /// The mapping is stored and can be refreshed/released later.
    pub async fn request_port_mapping(&self, lifetime_secs: u32) -> Result<u16> {
        let mut mapper_guard = self.port_mapper.write().await;
        let mapper = mapper_guard
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Port mapper not initialized"))?;

        let external_port = mapper.request_mapping(lifetime_secs).await?;

        // Store the mapping
        if let Some(mapping) = mapper.current_mapping() {
            info!(
                "Port mapping established: internal {} -> external {} (gateway: {}, lifetime: {}s)",
                mapping.internal_port, mapping.external_port, mapping.gateway, lifetime_secs
            );
            let mut pm = self.port_mapping.write().await;
            *pm = Some(mapping.clone());
        }

        Ok(external_port)
    }

    /// Check and refresh port mapping if needed (at 50% of lifetime)
    pub async fn check_and_refresh_port_mapping(&self) -> Result<bool> {
        let mut mapper_guard = self.port_mapper.write().await;
        if let Some(mapper) = mapper_guard.as_mut() {
            let refreshed = mapper.check_and_refresh().await?;
            if refreshed {
                if let Some(mapping) = mapper.current_mapping() {
                    info!(
                        "Port mapping refreshed: external port {}",
                        mapping.external_port
                    );
                    let mut pm = self.port_mapping.write().await;
                    *pm = Some(mapping.clone());
                }
            }
            Ok(refreshed)
        } else {
            Ok(false)
        }
    }

    /// Release the current port mapping
    pub async fn release_port_mapping(&self) -> Result<()> {
        let mut mapper_guard = self.port_mapper.write().await;
        if let Some(mapper) = mapper_guard.as_mut() {
            mapper.release().await?;
            info!("Port mapping released");
            let mut pm = self.port_mapping.write().await;
            *pm = None;
        }
        Ok(())
    }

    /// Update network configuration and persist to disk
    pub fn set_network_config(&mut self, config: NetworkConfig) -> Result<()> {
        // Validate before applying
        config.validate()?;

        self.network_config = config.clone();

        // Persist to disk
        if let Ok(mut cli_config) = CliConfig::load() {
            cli_config.network_config = config;
            cli_config.save()?;
        }

        Ok(())
    }

    pub async fn sync_state(
        &mut self,
        state: ConnectionState,
        network_id: Option<String>,
        virtual_ip: Option<String>,
        virtual_ip_v6: Option<String>,
    ) {
        self.set_state(state).await;
        let mut nid = self.current_network_id.write().await;
        *nid = network_id;
        let mut vip = self.virtual_ip.write().await;
        *vip = virtual_ip;
        let mut vip_v6 = self.virtual_ip_v6.write().await;
        *vip_v6 = virtual_ip_v6;
    }

    async fn set_state(&self, new_state: ConnectionState) {
        let mut state = self.state.write().await;
        info!(
            "Connection state transition: {:?} -> {:?}",
            *state, new_state
        );
        *state = new_state;
    }

    /// Logout - clear authentication state and stored tokens
    pub async fn logout(&mut self) -> Result<()> {
        info!("Logging out - clearing authentication state...");

        // Clear API client
        self.api_client = None;

        // Clear stored tokens from config
        if let Ok(mut config) = crate::config::CliConfig::load() {
            config.auth_response = None;
            if let Err(e) = config.save() {
                warn!("Failed to clear saved auth tokens: {}", e);
            } else {
                info!("Cleared saved authentication tokens");
            }
        }

        // Set state to disconnected
        self.set_state(ConnectionState::Disconnected).await;

        Ok(())
    }

    pub async fn try_auto_login(&mut self) -> Result<bool> {
        // Check if we have saved auth credentials
        let config = crate::config::CliConfig::load()?;

        // If already authenticated with valid API client, skip
        if self.api_client.is_some() {
            let current_state = self.get_state().await;
            if current_state != ConnectionState::Disconnected {
                info!(
                    "Already authenticated with API client (state: {:?}), skipping auto-login",
                    current_state
                );
                return Ok(true);
            }
        }

        info!("Attempting auto-login...");
        if let Some(auth) = config.auth_response.clone() {
            // Try using the saved token
            self.api_client = Some(ApiClient::new(
                self.base_url.clone(),
                Some(auth.effective_token().to_string()),
            ));
            if let Ok(_profile) = self.get_profile().await {
                // Update state if needed
                let current_state = self.get_state().await;
                if current_state == ConnectionState::Disconnected {
                    self.set_state(ConnectionState::Authenticated).await;
                }
                Ok(true)
            } else {
                // Token invalid, clear the client
                self.api_client = None;
                Ok(false)
            }
        } else {
            Ok(false)
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn connect_with_token(
        &mut self,
        token: String,
        network_id: &str,
        device_id: &str,
        hardware_id: &str,
        is_nucleus: bool,
        as_exit_node: bool,
        exit_node_ip: Option<String>,
        exit_node_ip_v6: Option<String>,
    ) -> Result<JoinVirtualNetworkResponse> {
        // Disconnect any existing connection first to prevent duplicate TUN interfaces
        if self.is_connected() {
            info!("Already connected in connect_with_token, disconnecting first...");
            let _ = self.disconnect().await;
        }

        self.set_state(ConnectionState::Authenticated).await;
        self.is_nucleus = is_nucleus;
        self.as_exit_node.store(as_exit_node, Ordering::SeqCst);
        self.exit_node_ip = exit_node_ip;
        self.exit_node_ip_v6 = exit_node_ip_v6;

        // Initialize nucleus state if running in nucleus mode
        if is_nucleus {
            info!("Initializing Nucleus signaling server state...");
            self.nucleus_state = Some(Arc::new(Mutex::new(NucleusState::new())));
        }

        let client = ApiClient::new(self.base_url.clone(), Some(token));
        self.api_client = Some(client);

        match self.perform_join(network_id, device_id, hardware_id).await {
            Ok(join_resp) => Ok(join_resp),
            Err(e) => {
                // If join fails, reset state back to Authenticated so we can try again
                self.set_state(ConnectionState::Authenticated).await;
                Err(e)
            }
        }
    }

    pub async fn perform_join(
        &mut self,
        network_id: &str,
        device_id: &str,
        hardware_id: &str,
    ) -> Result<JoinVirtualNetworkResponse> {
        info!(
            "Starting perform_join for network: {}, device_id: {}, hardware_id: {}",
            network_id, device_id, hardware_id
        );
        info!("Using API base URL: {}", self.base_url);

        // Redundant disconnect check removed - connect_with_token already handles this


        self.set_state(ConnectionState::Joining).await;
        self.device_id = Some(device_id.to_string());
        // Store hardware_id for heartbeats - this is different from device_id (API UUID)
        self.hardware_id = Some(hardware_id.to_string());
        {
            let mut nid = self.current_network_id.write().await;
            *nid = Some(network_id.to_string());
        }

        // On Windows, skip cleanup if we have an existing TUN we want to reuse
        // This prevents destroying the adapter on reconnect
        #[cfg(target_os = "windows")]
        let should_cleanup = self.tun.is_none();
        #[cfg(not(target_os = "windows"))]
        let should_cleanup = true;

        if should_cleanup {
            let _ = self.cleanup_adapters();
            // Give the OS time to fully release TUN resources
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
        } else {
            info!("Windows: Skipping adapter cleanup - reusing existing TUN");
        }

        let client = self.api_client.as_ref().context("Not authenticated")?;
        let dev_service = DeviceService::new(client);
        let net_service = NetworkService::new(client);

        // 0. Register/Update Device
        let os = std::env::consts::OS;
        let hostname =
            ::whoami::fallible::hostname().unwrap_or_else(|_| "OmniEdge Device".to_string());
        info!(
            "Registering/Updating device: {} (OS: {}, hardware_id: {})",
            hostname, os, hardware_id
        );
        let device_resp = dev_service.register(&hostname, hardware_id, os).await;

        let effective_device_id = if let Ok(ref resp) = device_resp {
            info!("Device registered/updated successfully. ID: {}", resp.id);
            // Update self.device_id with the actual device UUID from the API
            self.device_id = Some(resp.id.clone());
            &resp.id
        } else {
            let e = device_resp.as_ref().unwrap_err();
            warn!(
                "Device registration failed: {}. Proceeding with hardware_id: {}",
                e, hardware_id
            );
            hardware_id
        };

        // 1. Join Network
        info!(
            "Attempting to join virtual network: {} as effective_device_id: {}",
            network_id, effective_device_id
        );
        let join_resp = match net_service.join(network_id, effective_device_id).await {
            Ok(resp) => resp,
            Err(e) => {
                let err_msg = format!("Join failed for network {}: {}", network_id, e);
                error!("{}", err_msg);
                self.set_state(ConnectionState::Authenticated).await;
                return Err(anyhow::anyhow!(err_msg));
            }
        };

        info!(
            "Join successful. VIP: {}, Cluster: {}, Nucleus: {}",
            join_resp.virtual_ip, join_resp.cluster, join_resp.server.host
        );
        debug!("Full Join response: {:?}", join_resp);

        self.set_state(ConnectionState::Connecting).await;

        // 2. Initialize Proto & Tun
        let vip_addr: std::net::Ipv4Addr = join_resp.virtual_ip.parse()?;

        // Parse IPv6 address if provided (dual-stack support)
        let vip_v6_addr: Option<std::net::Ipv6Addr> = join_resp
            .virtual_ip_v6
            .as_ref()
            .and_then(|v| v.parse().ok());

        if let Some(ref v6) = vip_v6_addr {
            info!("Dual-stack enabled. VIP: {}, VIPv6: {}", vip_addr, v6);
        }

        self.cluster_secret = Some(join_resp.secret_key.clone());

        // ====================================================================
        // 2. Create UDP Socket FIRST (needed for port info)
        // ====================================================================
        // Create dual-stack UDP socket for IPv6 support
        // Try binding to [::]:0 first (dual-stack), fall back to 0.0.0.0:0 (IPv4-only)
        let socket: Arc<UdpSocket> =
            Arc::new(Self::create_dual_stack_socket().await.unwrap_or_else(|e| {
                warn!(
                    "Failed to create dual-stack socket: {}. Falling back to IPv4-only.",
                    e
                );
                // This should not fail, but handle it gracefully
                futures::executor::block_on(UdpSocket::bind("0.0.0.0:0"))
                    .expect("Failed to bind IPv4 socket")
            }));
        let port = socket.local_addr()?.port();
        debug!("Bound UDP socket to port: {}", port);

        // ====================================================================
        // 3. Port Mapping (NAT Traversal Fix v0.3.5)
        // ====================================================================
        // Try to establish a port mapping via NAT-PMP/UPnP/PCP for better connectivity.
        // This allows peers behind compatible NAT gateways to receive incoming connections.
        let mut external_port: Option<u16> = None;
        let mut external_addr: Option<String> = None;
        
        if self.network_config.portmap_enabled {
            info!("Initializing port mapping for local port {}...", port);
            match self.init_port_mapper(port).await {
                Ok(caps) => {
                    if caps.nat_pmp || caps.upnp || caps.pcp {
                        // Request a mapping with 2-hour lifetime (will be refreshed periodically)
                        match self.request_port_mapping(7200).await {
                            Ok(ext_port) => {
                                external_port = Some(ext_port);
                                if let Some(ext_addr) = caps.external_addr {
                                    let full_addr = format!("{}:{}", ext_addr, ext_port);
                                    external_addr = Some(full_addr.clone());
                                    info!(
                                        "Port mapping established: {}:{} -> {}",
                                        ext_addr, port, full_addr
                                    );
                                } else {
                                    info!(
                                        "Port mapping established: internal:{} -> external:{}",
                                        port, ext_port
                                    );
                                }
                            }
                            Err(e) => {
                                warn!("Failed to request port mapping: {}. Continuing without mapping.", e);
                            }
                        }
                    } else {
                        debug!(
                            "No port mapping protocols available (NAT-PMP={}, UPnP={}, PCP={})",
                            caps.nat_pmp, caps.upnp, caps.pcp
                        );
                    }
                }
                Err(e) => {
                    warn!("Failed to probe port mapping capabilities: {}. Continuing without mapping.", e);
                }
            }
        } else {
            debug!("Port mapping disabled in configuration");
        }

        // ====================================================================
        // 4. Initialize OmniProto with correct port info
        // ====================================================================
        info!("Initializing OmniProto for VIP: {} (listen_port: {})", vip_addr, port);

        let proto = Arc::new(
            OmniProto::new(
                &join_resp.server.host,
                join_resp.cluster.clone(),
                join_resp.secret_key.clone(),
                vip_addr,
                vip_v6_addr,
                port,  // Use actual socket port now!
                self.identity.public_key_bytes(),
                self.identity.private_key_bytes(),
            )
            .await?,
        );

        // Set external port/addr if port mapping succeeded
        if let Some(ext_port) = external_port {
            proto.set_external_port(ext_port).await;
            info!("Advertising external port {} to Nucleus", ext_port);
        }
        if let Some(ref ext_addr) = external_addr {
            proto.set_external_addr(ext_addr.clone()).await;
            info!("Advertising external endpoint {} to Nucleus for NAT traversal", ext_addr);
        }

        // ====================================================================
        // CRITICAL: Send initial REGISTER with external port/addr to Nucleus
        // ====================================================================
        // This must happen AFTER setting external_port/external_addr so Nucleus
        // knows our port-mapped endpoint immediately. Otherwise peers won't know
        // our reachable address until the first heartbeat (up to 30 seconds).
        info!("Sending initial REGISTER to Nucleus with port mapping info...");
        if let Err(e) = proto.register(&socket).await {
            warn!("Initial REGISTER failed: {}. Will retry on heartbeat.", e);
        }

        // Pass IPv6 configuration to protocol layer
        // Note: These are currently no-ops until OmniNervous adds full IPv6 support,
        // but setting them now ensures they'll work when the underlying library is updated.
        if self.network_config.ipv6_enabled {
            proto.set_ipv6_enabled(true);
            proto.set_ipv6_preference(
                self.network_config.prefer_ipv6,
                self.network_config.ipv6_preference_threshold_ms,
            );
            info!(
                "IPv6 configuration: enabled={}, prefer={}, threshold={}ms",
                self.network_config.ipv6_enabled,
                self.network_config.prefer_ipv6,
                self.network_config.ipv6_preference_threshold_ms
            );
        }

        // Pass NetworkConfig relay/portmap settings to OmniNervous runtime state
        proto
            .update_relay_enabled(self.network_config.relay_enabled)
            .await;
        proto
            .update_portmap_enabled(self.network_config.portmap_enabled)
            .await;
        // Note: relay_server and encrypt_signaling are set via OmniNervous config file,
        // not through runtime API. See OmniNervous crates/daemon/src/config.rs

        // ====================================================================
        // 5. Setup TUN
        // ====================================================================
        // First, check if an interface with this IP already exists
        // On Windows, skip this check if we're reusing our existing TUN
        #[cfg(target_os = "windows")]
        let skip_interface_check = self.tun.is_some();
        #[cfg(not(target_os = "windows"))]
        let skip_interface_check = false;

        if !skip_interface_check {
            if let Some(existing_iface) = Self::find_interface_with_ip(&join_resp.virtual_ip) {
                warn!(
                    "Interface {} already exists with IP {}. Cleaning up before creating new TUN.",
                    existing_iface, join_resp.virtual_ip
                );
                let _ = self.cleanup_adapters();
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            }
        }

        #[allow(unused_assignments)]
        let mut tun_instance: Option<OmniTun> = None;
        // Note: 'port' is already defined from socket.local_addr() above
        #[allow(unused_mut)]
        let mut tun_loop_already_active = false;

        #[cfg(target_os = "windows")]
        {
            // On Windows, check if we already have a TUN from a previous connection.
            // WinTun adapters persist and create "wintun 2", "wintun 3" etc. if we
            // create new ones. Reuse the existing adapter when possible.
            if let Some(ref existing_tun) = self.tun {
                // Check if the TUN loop is still active from previous connection
                if existing_tun.is_tun_active().await {
                    info!("Windows: TUN loop is still active, reusing for reconnect");
                    tun_loop_already_active = true;
                    tun_instance = self.tun.take();
                    // Reconfigure will happen via add_peer calls
                    info!("Reconfiguring existing TUN for new connection...");
                } else {
                    info!("Windows: TUN exists but loop is not active, will restart loop");
                    tun_instance = self.tun.take();
                }
            } else {
                // No existing TUN, need to create one
                // First, clean up any orphaned WinTun adapters from previous runs
                if Self::windows_adapter_exists("wintun")
                    || Self::windows_adapter_exists("OmniEdge")
                {
                    info!("Found orphaned WinTun/OmniEdge adapter(s), cleaning up...");
                    let _ = self.cleanup_adapters();
                    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

                    if Self::windows_adapter_exists("wintun")
                        || Self::windows_adapter_exists("OmniEdge")
                    {
                        warn!("WinTun adapter still exists after cleanup, attempting force removal...");
                        Self::windows_force_remove_adapter("wintun");
                        Self::windows_force_remove_adapter("OmniEdge");
                        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                    }
                }

                let if_names = ["OmniEdge"];
                let mut setup_success = false;
                let mut last_err = String::new();
                let max_retries = 3;

                for retry in 0..max_retries {
                    if retry > 0 {
                        info!("TUN setup retry attempt {} of {}", retry + 1, max_retries);
                        let _ = self.cleanup_adapters();
                        tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
                    }

                    for ifname in if_names {
                        debug!("Attempting TUN setup with interface name: {}", ifname);
                        // Convert WireGuardMode to WgMode for OmniTun
                        let wg_mode = match self.network_config.wireguard_mode {
                            crate::config::WireGuardMode::Auto => WgMode::Auto,
                            crate::config::WireGuardMode::Kernel => WgMode::Kernel,
                            crate::config::WireGuardMode::Userspace => WgMode::Userspace,
                        };
                        let mut tun = OmniTun::new_with_mode(ifname, wg_mode);

                        match tun
                            .setup_dual_stack(
                                &join_resp.virtual_ip,
                                Some(join_resp.subnet_mask.as_str()),
                                join_resp.virtual_ip_v6.as_deref(),
                                join_resp.subnet_prefix_v6,
                                port,
                                &::hex::encode(self.identity.private_key_bytes()),
                                self.network_config.effective_mtu(),
                            )
                            .await
                        {
                            Ok(_) => {
                                info!("TUN setup completed successfully using name: {}", ifname);
                                setup_success = true;
                                tun_instance = Some(tun);
                                break;
                            }
                            Err(e) => {
                                last_err = e.to_string();
                                warn!("TUN setup failed for name {}: {}", ifname, e);
                            }
                        }
                    }

                    if setup_success {
                        break;
                    }
                }

                if !setup_success {
                    let err_msg = format!("Failed to create TUN device after {} attempts. Please ensure you are running OmniEdge as Administrator and no other VPN is conflicting. Error: {}", max_retries, last_err);
                    error!("CRITICAL: {}", err_msg);
                    return Err(anyhow::anyhow!(err_msg));
                }
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            // On macOS, TUN interfaces must be named utunN - we pass empty string
            // to let the system assign the next available utun interface.
            // On Linux, we can use custom names like "omniedge0"
            #[cfg(target_os = "macos")]
            let ifname = ""; // macOS will auto-assign utunN
            #[cfg(target_os = "linux")]
            let ifname = "omniedge0";

            info!(
                "Creating TUN: {} (WireGuard mode: {:?})",
                if ifname.is_empty() {
                    "(auto-assign)"
                } else {
                    ifname
                },
                self.network_config.wireguard_mode
            );
            // Convert WireGuardMode to WgMode for OmniTun
            let wg_mode = match self.network_config.wireguard_mode {
                crate::config::WireGuardMode::Auto => WgMode::Auto,
                crate::config::WireGuardMode::Kernel => WgMode::Kernel,
                crate::config::WireGuardMode::Userspace => WgMode::Userspace,
            };
            let mut tun = OmniTun::new_with_mode(ifname, wg_mode);
            tun.setup_dual_stack(
                &join_resp.virtual_ip,
                Some(join_resp.subnet_mask.as_str()),
                join_resp.virtual_ip_v6.as_deref(),
                join_resp.subnet_prefix_v6,
                port,
                &::hex::encode(self.identity.private_key_bytes()),
                self.network_config.effective_mtu(),
            )
            .await?;
            tun_instance = Some(tun);
        }

        let tun = tun_instance.context("TUN instance not created")?;

        self.proto = Some(proto.clone());
        self.tun = Some(tun.clone());
        {
            let mut vip = self.virtual_ip.write().await;
            *vip = Some(join_resp.virtual_ip.clone());
        }
        {
            let mut vip_v6 = self.virtual_ip_v6.write().await;
            *vip_v6 = join_resp.virtual_ip_v6.clone();
        }

        // 4. Start Background Loops
        info!("Starting background loops...");
        let (shutdown_tx, _) = broadcast::channel(1);
        self.shutdown_tx = Some(shutdown_tx.clone());

        let nucleus_state = self.nucleus_state.clone();
        let nucleus_port = self.nucleus_port;
        let is_nucleus = self.is_nucleus;

        self.start_loops(
            socket,
            proto,
            tun,
            effective_device_id.to_string(),
            hardware_id.to_string(),
            shutdown_tx,
            nucleus_state,
            nucleus_port,
            is_nucleus,
            tun_loop_already_active,
        )
        .await;

        self.set_state(ConnectionState::Connected).await;

        // 5. Setup Exit Node Routing if requested
        if let Some(ref exit_ip) = self.exit_node_ip {
            info!(
                "Configuring system to use exit node: {} (v6: {:?})",
                exit_ip, self.exit_node_ip_v6
            );
            let nucleus_host = &join_resp.server.host;
            if let Err(e) = crate::routing::RoutingManager::setup_exit_node(
                exit_ip,
                self.exit_node_ip_v6.as_deref(),
                nucleus_host,
            ) {
                error!("Failed to setup exit node routing: {}", e);
            }
        }

        Ok(join_resp)
    }

    #[allow(clippy::too_many_arguments)]
    async fn start_loops(
        &mut self,
        socket: Arc<UdpSocket>,
        proto: Arc<OmniProto>,
        tun: OmniTun,
        _device_id: String,
        hardware_id: String,
        shutdown_tx: broadcast::Sender<()>,
        nucleus_state: Option<Arc<Mutex<NucleusState>>>,
        nucleus_port: u16,
        is_nucleus: bool,
        skip_tun_loop: bool,
    ) {
        let (hb_tx, mut hb_rx) = mpsc::channel(1);
        self.heartbeat_tx = Some(hb_tx);
        let mut tun_ctrl = tun.clone();
        let proto_ctrl = proto.clone();
        let socket_inner = socket.clone();
        let secret = self.cluster_secret.clone();

        // Clone peer state tracking for dispatcher
        let peer_states = self.peer_states.clone();
        let pending_pings = self.pending_pings.clone();
        let our_public_key = self.identity.public_key_bytes();
        let our_vip_str = self.virtual_ip.read().await.clone();
        let our_vip: Option<Ipv4Addr> = our_vip_str.as_ref().and_then(|s| s.parse().ok());
        let our_vip_v6_str = self.virtual_ip_v6.read().await.clone();
        let our_vip_v6: Option<std::net::Ipv6Addr> =
            our_vip_v6_str.as_ref().and_then(|s| s.parse().ok());
        let disco_config = self.disco_config.clone();

        // Clone relay state for dispatcher
        let relay_client = self.relay_client.clone();
        let relay_sessions = self.relay_sessions.clone();

        // Track WireGuard mode and listen port for kernel mode support
        // In kernel mode, we need to tell peers which port to send WG packets to
        // and use transparent relay (no RELAY_DATA header encapsulation)
        let is_kernel_wg = tun.is_kernel_mode();
        let wg_listen_port: Option<u16> = if is_kernel_wg {
            // In kernel mode, WG listens on the same port as our signaling socket
            socket.local_addr().ok().map(|a| a.port())
        } else {
            None // Userspace mode - same port for everything, no need to specify
        };
        if is_kernel_wg {
            info!(
                "Kernel WireGuard mode active, WG listen port: {:?}",
                wg_listen_port
            );
        }

        // Get relay server address: prefer custom relay_server from config, fall back to nucleus
        let relay_server_addr: Option<std::net::SocketAddr> = self
            .network_config
            .relay_server
            .as_ref()
            .and_then(|s| s.parse().ok())
            .or_else(|| proto.get_nucleus_host().parse().ok());

        if disco_config.relay_enabled {
            if let Some(addr) = relay_server_addr {
                info!("Relay server configured: {}", addr);
            } else {
                warn!("Relay enabled but no valid relay server address");
            }
        }

        // Clear any existing task handles
        self.task_handles.clear();

        // Nucleus Signaling Server Loop (only when running in nucleus mode)
        if is_nucleus {
            if let Some(nucleus_state) = nucleus_state.clone() {
                let secret_clone = secret.clone();
                let mut shutdown_rx_nucleus = shutdown_tx.subscribe();

                // Bind nucleus signaling socket on fixed port
                let nucleus_socket =
                    match UdpSocket::bind(format!("0.0.0.0:{}", nucleus_port)).await {
                        Ok(s) => {
                            info!(
                                "Nucleus signaling server listening on UDP port {}",
                                nucleus_port
                            );
                            Arc::new(s)
                        }
                        Err(e) => {
                            error!(
                            "Failed to bind nucleus signaling port {}: {}. Nucleus mode disabled.",
                            nucleus_port, e
                        );
                            // Continue without nucleus mode
                            Arc::new(UdpSocket::bind("0.0.0.0:0").await.unwrap())
                        }
                    };

                tokio::spawn(async move {
                    let mut buf = [0u8; 4096];
                    let mut cleanup_interval =
                        tokio::time::interval(tokio::time::Duration::from_secs(60));

                    loop {
                        tokio::select! {
                            res = nucleus_socket.recv_from(&mut buf) => {
                                match res {
                                    Ok((len, src)) => {
                                        let pkt = &buf[..len];
                                        if pkt.is_empty() || pkt[0] < 0x11 {
                                            continue;
                                        }

                                        // Handle nucleus signaling request
                                        let mut state = nucleus_state.lock().await;
                                        let result = handle_nucleus_message(
                                            &mut state,
                                            pkt,
                                            src,
                                            secret_clone.as_deref(),
                                        );
                                        if let Some(response) = result.response {
                                            if let Err(e) = nucleus_socket.send_to(&response, src).await {
                                                warn!("Failed to send nucleus response to {}: {}", src, e);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        error!("Nucleus socket error: {}", e);
                                    }
                                }
                            }
                            _ = cleanup_interval.tick() => {
                                // Periodic cleanup of stale peers
                                let mut state = nucleus_state.lock().await;
                                state.cleanup();
                                debug!("Nucleus state cleanup complete. {} peers registered.", state.peer_count());
                            }
                            _ = shutdown_rx_nucleus.recv() => {
                                info!("Nucleus Signaling Server shutting down");
                                break;
                            }
                        }
                    }
                });
            }
        }

        // Master Dispatcher Loop - handles signaling, disco, relay, and WireGuard packets
        let mut shutdown_rx1 = shutdown_tx.subscribe();
        let socket_for_disco = socket_inner.clone();

        // Initialize relay client if relay is enabled and we have a relay server address
        if disco_config.relay_enabled {
            if let Some(relay_addr) = relay_server_addr {
                if let Some(vip) = our_vip {
                    let client = RelayClient::new(our_public_key, vip, relay_addr);
                    *relay_client.write().await = Some(client);
                    info!("Relay client initialized, server: {}", relay_addr);
                }
            } else {
                warn!("Relay enabled but no relay server address available");
            }
        }

        let dispatcher_handle = tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            // Disco timeout check interval
            let mut disco_check_interval =
                tokio::time::interval(tokio::time::Duration::from_secs(1));

            loop {
                tokio::select! {
                    res = socket_inner.recv_from(&mut buf) => {
                         match res {
                            Ok((len, src)) => {
                                let pkt = &buf[..len];
                                if pkt.is_empty() {
                                    continue;
                                }

                                let first_byte = pkt[0];
                                trace!("UDP packet received: len={}, from={}, first_byte=0x{:02x}", len, src, first_byte);

                                // Handle DISCO_PING (0x1B)
                                if first_byte == SIGNALING_DISCO_PING {
                                    if let Ok(ping) = parse_disco_ping(pkt) {
                                        info!(
                                            "Received disco ping from {} (VIP: {}, tx: {:02x?})",
                                            src, ping.sender_vip, &ping.tx_id[..4]
                                        );

                                        // Create and send pong response
                                        // In kernel mode, include wg_port so peer knows where to send WG packets
                                        let pong = DiscoPong {
                                            tx_id: ping.tx_id,
                                            observed_addr: src.to_string(),
                                            responder_key: our_public_key,
                                            responder_vip_v6: our_vip_v6,
                                            wg_port: wg_listen_port,
                                        };

                                        if let Ok(pong_data) = encode_disco_pong(&pong) {
                                            if let Err(e) = socket_inner.send_to(&pong_data, src).await {
                                                warn!("Failed to send disco pong to {}: {}", src, e);
                                            } else {
                                                info!("Sent disco pong to {} (VIP: {}, tx: {:02x?})", src, ping.sender_vip, &pong.tx_id[..4]);
                                            }
                                        }

                                        // Update peer endpoint if we know this peer
                                        let mut peers = peer_states.write().await;
                                        if let Some(peer_state) = peers.get_mut(&ping.sender_vip) {
                                            // SECURITY: Verify sender's public key matches expected peer
                                            // This prevents an attacker from spoofing disco pings to
                                            // inject malicious endpoints into our peer state
                                            if ping.sender_key != peer_state.public_key {
                                                warn!(
                                                    "Disco ping key mismatch for VIP {}: expected {:02x?}..., got {:02x?}...",
                                                    ping.sender_vip,
                                                    &peer_state.public_key[..8],
                                                    &ping.sender_key[..8]
                                                );
                                                continue;  // Reject the ping - possible spoofing attempt
                                            }

                                            // Add endpoint from direct probe (highest priority source)
                                            peer_state.add_endpoint(src, EndpointSource::DirectProbe);
                                            peer_state.last_seen = Some(Instant::now());
                                            info!(
                                                "Peer {} endpoint added via disco ping: {} (total: {} endpoints)",
                                                ping.sender_vip, src, peer_state.endpoint_count()
                                            );
                                        }
                                    }
                                    continue;
                                }

                                // Handle DISCO_PONG (0x1C)
                                if first_byte == SIGNALING_DISCO_PONG {
                                    if let Ok(pong) = parse_disco_pong(pkt) {
                                        // Look up pending ping
                                        let pending = {
                                            let mut pings = pending_pings.write().await;
                                            pings.remove(&pong.tx_id)
                                        };

                                        if let Some(pending) = pending {
                                            let rtt = pending.sent_at.elapsed();
                                            info!(
                                                "Disco pong received from {} (VIP: {}, RTT: {:?}, observed: {})",
                                                src, pending.target_vip, rtt, pong.observed_addr
                                            );

                                            // Record the pong and update best endpoint
                                            let mut peers = peer_states.write().await;
                                            if let Some(peer_state) = peers.get_mut(&pending.target_vip) {
                                                // SECURITY: Verify responder's public key matches expected peer
                                                // This prevents an attacker from impersonating a peer by
                                                // responding to disco pings with a fake public key
                                                if pong.responder_key != peer_state.public_key {
                                                    warn!(
                                                        "Disco pong key mismatch for VIP {}: expected {:02x?}..., got {:02x?}...",
                                                        pending.target_vip,
                                                        &peer_state.public_key[..8],
                                                        &pong.responder_key[..8]
                                                    );
                                                    continue;  // Reject the pong - possible impersonation attempt
                                                }

                                                // Add endpoint from direct probe and record latency
                                                peer_state.add_endpoint(src, EndpointSource::DirectProbe);
                                                peer_state.record_pong(src, rtt);

                                                // Get the best endpoint (may have changed)
                                                if let Some(best_ep) = peer_state.best_endpoint() {
                                                    let latency = peer_state.best_endpoint_info()
                                                        .and_then(|e| e.latency)
                                                        .map(|l| format!("{:?}", l))
                                                        .unwrap_or_else(|| "unknown".to_string());

                                                    // Configure WireGuard with the best endpoint
                                                    let pubkey = ::hex::encode(peer_state.public_key);
                                                    let mut allowed_ips = vec![format!("{}/32", peer_state.vip)];
                                                    if let Some(vip_v6) = peer_state.vip_v6 {
                                                        allowed_ips.push(format!("{}/128", vip_v6));
                                                    }

                                                    info!(
                                                        "Configuring WireGuard peer {} at {} (latency: {}, {} endpoints)",
                                                        peer_state.vip, best_ep, latency, peer_state.endpoint_count()
                                                    );
                                                    let _ = tun_ctrl
                                                        .add_peer(&pubkey, Some(best_ep), &allowed_ips)
                                                        .await;
                                                }
                                            }
                                        } else {
                                            debug!(
                                                "Received disco pong with unknown tx_id {:02x?} from {} (late/duplicate)",
                                                &pong.tx_id[..4], src
                                            );
                                        }
                                    }
                                    continue;
                                }

                                // Handle Relay messages (0x20-0x24)
                                if is_relay_message(pkt) {
                                    // Handle RELAY_BIND_ACK (0x21)
                                    if first_byte == MSG_RELAY_BIND_ACK {
                                        if let Ok(ack) = parse_relay_bind_ack(pkt) {
                                            if ack.success {
                                                if let Some(session_id) = ack.session_id {
                                                    info!(
                                                        "Relay session established: {:02x?}",
                                                        &session_id[..4]
                                                    );

                                                    // SECURITY: Use target_key to identify the correct peer
                                                    // This prevents session hijacking by associating the session
                                                    // with the correct peer based on cryptographic identity
                                                    let target_key = match ack.target_key {
                                                        Some(key) => key,
                                                        None => {
                                                            // Legacy server without target_key - fall back to finding
                                                            // first peer in RelayTry state (less secure)
                                                            warn!(
                                                                "Relay ACK missing target_key - using legacy association"
                                                            );
                                                            let peers = peer_states.read().await;
                                                            let legacy_key = peers.values()
                                                                .find(|p| p.state == PeerConnectionState::RelayTry)
                                                                .map(|p| p.public_key);
                                                            match legacy_key {
                                                                Some(k) => k,
                                                                None => {
                                                                    warn!("No peer in RelayTry state for relay ACK");
                                                                    continue;
                                                                }
                                                            }
                                                        }
                                                    };

                                                    // Find peer by target_key and update
                                                    let mut peers = peer_states.write().await;
                                                    let peer_entry = peers.values_mut()
                                                        .find(|p| p.public_key == target_key);

                                                    if let Some(peer_state) = peer_entry {
                                                        // Verify peer is actually waiting for relay
                                                        if peer_state.state != PeerConnectionState::RelayTry {
                                                            warn!(
                                                                "Received relay ACK for peer {} not in RelayTry state (state: {:?})",
                                                                peer_state.vip, peer_state.state
                                                            );
                                                            continue;
                                                        }

                                                        peer_state.mark_relayed();

                                                        // Store session ID
                                                        relay_sessions.write().await.insert(
                                                            peer_state.public_key,
                                                            session_id,
                                                        );

                                                        // Configure WireGuard with relay endpoint
                                                        let pubkey = ::hex::encode(peer_state.public_key);
                                                        let mut allowed_ips = vec![format!("{}/32", peer_state.vip)];
                                                        if let Some(v6) = peer_state.vip_v6 {
                                                            allowed_ips.push(format!("{}/128", v6));
                                                        }

                                                        // Use relay server as endpoint
                                                        // For transparent relay, use `src` (actual relay server address)
                                                        // instead of `ack.relay_endpoint` which may be 0.0.0.0
                                                        // from the relay server's local_addr()
                                                        let endpoint = if ack.transparent {
                                                            // Transparent relay: use source address of ACK
                                                            Some(src)
                                                        } else {
                                                            // Standard relay: use relay_endpoint from ACK or fallback
                                                            ack.relay_endpoint
                                                                .as_ref()
                                                                .and_then(|s| s.parse().ok())
                                                                .or(relay_server_addr)
                                                        };

                                                        if let Some(ep) = endpoint {
                                                            info!(
                                                                "Configuring WireGuard peer {} via relay {} (transparent: {})",
                                                                peer_state.vip, ep, ack.transparent
                                                            );
                                                            let _ = tun_ctrl
                                                                .add_peer(&pubkey, Some(ep), &allowed_ips)
                                                                .await;
                                                        }
                                                    } else {
                                                        warn!(
                                                            "Received relay ACK for unknown peer key {:02x?}...",
                                                            &target_key[..8]
                                                        );
                                                    }
                                                }
                                            } else {
                                                warn!("Relay bind failed: {:?}", ack.error);
                                                // SECURITY: Use target_key to mark the correct peer as failed
                                                if let Some(target_key) = ack.target_key {
                                                    let mut peers = peer_states.write().await;
                                                    if let Some(peer_state) = peers.values_mut()
                                                        .find(|p| p.public_key == target_key)
                                                    {
                                                        if peer_state.state == PeerConnectionState::RelayTry {
                                                            peer_state.state = PeerConnectionState::Failed;
                                                        }
                                                    }
                                                } else {
                                                    // Legacy fallback - mark first RelayTry peer as failed
                                                    let mut peers = peer_states.write().await;
                                                    for peer_state in peers.values_mut() {
                                                        if peer_state.state == PeerConnectionState::RelayTry {
                                                            peer_state.state = PeerConnectionState::Failed;
                                                            break;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        continue;
                                    }

                                    // Handle RELAY_DATA (0x22) - Forward relayed packets to WireGuard
                                    if first_byte == MSG_RELAY_DATA {
                                        if let Ok((session_id, wg_packet)) = parse_relay_data(pkt) {
                                            debug!(
                                                "Received relayed packet: session {:02x?}, {} bytes",
                                                &session_id[..4], wg_packet.len()
                                            );
                                            // Forward to WireGuard handler
                                            let _ = tun_ctrl.handle_packet(wg_packet, src, &socket_inner).await;
                                        }
                                        continue;
                                    }

                                    // Other relay messages (UNBIND, KEEPALIVE) - log and continue
                                    debug!("Received relay message type 0x{:02x} from {}", first_byte, src);
                                    continue;
                                }

                                // Handle other signaling messages (0x11-0x1F, excluding disco)
                                if first_byte >= 0x11 && first_byte != SIGNALING_DISCO_PING && first_byte != SIGNALING_DISCO_PONG {
                                    match proto_ctrl.handle_packet(pkt, secret.as_deref()).await {
                                        Ok(Some(update)) => {
                                            // Get our NAT type for strategy selection
                                            let our_nat_type = proto_ctrl.get_nat_type().await;
                                            info!("Our NAT type: {:?} ({} peers in update)", our_nat_type, update.peers.len());

                                        for peer in update.peers {
                                            let vip = peer.vip;
                                            let vip_v6 = peer.vip_v6;

                                            // Create peer state with NAT-aware strategy
                                            // Extract peer NAT type from signaling (Phase 6)
                                            let peer_nat_type = peer.nat_type;

                                            // DEBUG: Log all peer info received from Nucleus
                                            info!(
                                                "Peer info from Nucleus: VIP={}, endpoint={:?}, mapped_endpoint={:?}, nat_type={:?}, pubkey={:02x?}...",
                                                vip, peer.endpoint, peer.mapped_endpoint, peer_nat_type, &peer.public_key[..8]
                                            );

                                            let mut peers = peer_states.write().await;
                                            let peer_state = peers.entry(vip).or_insert_with(|| {
                                                let strategy = select_connection_strategy(our_nat_type, peer_nat_type);
                                                info!(
                                                    "Discovered new peer {} (v6: {:?}, nat: {:?}) - strategy: {}",
                                                    vip, vip_v6, peer_nat_type, strategy.description()
                                                );
                                                PeerState::with_nat_strategy(
                                                    vip,
                                                    vip_v6,
                                                    peer.public_key,
                                                    peer.endpoint,
                                                    our_nat_type,
                                                    peer_nat_type,
                                                )
                                            });

                                            // Update peer info if already exists - add endpoint from signaling
                                            if let Some(endpoint) = peer.endpoint {
                                                peer_state.add_endpoint(endpoint, EndpointSource::Nucleus);
                                                info!(
                                                    "Added Nucleus endpoint {} for peer {} (total: {} endpoints)",
                                                    endpoint, vip, peer_state.endpoint_count()
                                                );
                                            } else {
                                                warn!(
                                                    "No endpoint received from Nucleus for peer {} - cannot send disco pings",
                                                    vip
                                                );
                                            }

                                            // Add port-mapped endpoint if available (higher priority)
                                            if let Some(mapped_ep) = peer.mapped_endpoint {
                                                peer_state.add_endpoint(mapped_ep, EndpointSource::PortMap);
                                                debug!(
                                                    "Added port-mapped endpoint {} for peer {}",
                                                    mapped_ep, vip
                                                );
                                            }
                                            peer_state.vip_v6 = vip_v6;

                                            // Update strategy if NAT types changed
                                            if peer_state.peer_nat_type != peer_nat_type {
                                                peer_state.update_strategy(our_nat_type, peer_nat_type);
                                                debug!(
                                                    "Updated peer {} strategy: {}",
                                                    vip, peer_state.connection_strategy.description()
                                                );
                                            }

                                            // If peer already has working connection, just update WireGuard with best
                                            if peer_state.has_working_connection() {
                                                if let Some(endpoint) = peer_state.best_endpoint() {
                                                    let pubkey = ::hex::encode(peer_state.public_key);
                                                    let mut allowed_ips = vec![format!("{}/32", vip)];
                                                    if let Some(v6) = vip_v6 {
                                                        allowed_ips.push(format!("{}/128", v6));
                                                    }
                                                    let _ = tun_ctrl
                                                        .add_peer(&pubkey, Some(endpoint), &allowed_ips)
                                                        .await;
                                                }
                                                continue;
                                            }

                                            // Check connection strategy before attempting disco
                                            match peer_state.connection_strategy {
                                                ConnectionStrategy::RelayOnly => {
                                                    // Skip disco, go straight to relay
                                                    info!(
                                                        "Peer {} using RelayOnly strategy (both symmetric NAT) - skipping disco",
                                                        vip
                                                    );
                                                    peer_state.state = PeerConnectionState::RelayTry;
                                                    // Relay bind will be sent in the disco timeout handler
                                                }
                                                ConnectionStrategy::PortPrediction => {
                                                    // Try port prediction with multiple endpoints
                                                    // For now, fall back to standard disco with relay fallback
                                                    info!(
                                                        "Peer {} using PortPrediction strategy - trying disco with relay fallback",
                                                        vip
                                                    );
                                                    // Send disco pings to ALL known endpoints
                                                    let endpoints: Vec<_> = peer_state.endpoints.endpoints.iter()
                                                        .map(|e| e.addr)
                                                        .collect();

                                                    // CRITICAL FIX: Pre-configure WireGuard with first endpoint
                                                    // This allows traffic to flow while disco is in progress
                                                    if let Some(&first_endpoint) = endpoints.first() {
                                                        let pubkey = ::hex::encode(peer_state.public_key);
                                                        let mut allowed_ips = vec![format!("{}/32", vip)];
                                                        if let Some(v6) = vip_v6 {
                                                            allowed_ips.push(format!("{}/128", v6));
                                                        }
                                                        info!(
                                                            "Pre-configuring WireGuard peer {} with endpoint {} (PortPrediction, disco in progress)",
                                                            vip, first_endpoint
                                                        );
                                                        let _ = tun_ctrl
                                                            .add_peer(&pubkey, Some(first_endpoint), &allowed_ips)
                                                            .await;
                                                    }

                                                    for endpoint in endpoints {
                                                        let tx_id: [u8; 12] = rand::random();
                                                        let ping = DiscoPing {
                                                            tx_id,
                                                            sender_key: our_public_key,
                                                            sender_vip: our_vip.unwrap_or(Ipv4Addr::UNSPECIFIED),
                                                            sender_vip_v6: our_vip_v6,
                                                            wg_port: wg_listen_port,
                                                        };

                                                        if let Ok(ping_data) = encode_disco_ping(&ping) {
                                                            if let Err(e) = socket_for_disco.send_to(&ping_data, endpoint).await {
                                                                warn!("Failed to send disco ping to {}: {}", endpoint, e);
                                                            } else {
                                                                info!(
                                                                    "Sent disco ping to {} (VIP: {}, PortPrediction) tx: {:02x?}",
                                                                    endpoint, vip, &tx_id[..4]
                                                                );
                                                                peer_state.mark_probing(endpoint);
                                                                let pending = PendingDiscoPing {
                                                                    tx_id,
                                                                    target: endpoint,
                                                                    target_vip: vip,
                                                                    sent_at: Instant::now(),
                                                                    retries: 0,
                                                                    max_retries: disco_config.max_retries,
                                                                };
                                                                pending_pings.write().await.insert(tx_id, pending);
                                                            }
                                                        }
                                                    }
                                                    peer_state.state = PeerConnectionState::DirectTry;
                                                }
                                                ConnectionStrategy::SimultaneousOpen => {
                                                    // Both sides should send pings simultaneously
                                                    info!(
                                                        "Peer {} using SimultaneousOpen strategy - sending disco pings to {} endpoints",
                                                        vip, peer_state.endpoint_count()
                                                    );
                                                    // Send disco pings to ALL known endpoints
                                                    let endpoints: Vec<_> = peer_state.endpoints.endpoints.iter()
                                                        .map(|e| e.addr)
                                                        .collect();

                                                    // CRITICAL FIX: Pre-configure WireGuard with first endpoint
                                                    // For SimultaneousOpen, both sides configure each other proactively
                                                    if let Some(&first_endpoint) = endpoints.first() {
                                                        let pubkey = ::hex::encode(peer_state.public_key);
                                                        let mut allowed_ips = vec![format!("{}/32", vip)];
                                                        if let Some(v6) = vip_v6 {
                                                            allowed_ips.push(format!("{}/128", v6));
                                                        }
                                                        info!(
                                                            "Pre-configuring WireGuard peer {} with endpoint {} (SimultaneousOpen, disco in progress)",
                                                            vip, first_endpoint
                                                        );
                                                        let _ = tun_ctrl
                                                            .add_peer(&pubkey, Some(first_endpoint), &allowed_ips)
                                                            .await;
                                                    }

                                                    for endpoint in endpoints {
                                                        let tx_id: [u8; 12] = rand::random();
                                                        let ping = DiscoPing {
                                                            tx_id,
                                                            sender_key: our_public_key,
                                                            sender_vip: our_vip.unwrap_or(Ipv4Addr::UNSPECIFIED),
                                                            sender_vip_v6: our_vip_v6,
                                                            wg_port: wg_listen_port,
                                                        };

                                                        if let Ok(ping_data) = encode_disco_ping(&ping) {
                                                            if let Err(e) = socket_for_disco.send_to(&ping_data, endpoint).await {
                                                                warn!("Failed to send disco ping to {}: {}", endpoint, e);
                                                            } else {
                                                                info!(
                                                                    "Sent disco ping to {} (VIP: {}, SimultaneousOpen) tx: {:02x?}",
                                                                    endpoint, vip, &tx_id[..4]
                                                                );
                                                                peer_state.mark_probing(endpoint);
                                                                let pending = PendingDiscoPing {
                                                                    tx_id,
                                                                    target: endpoint,
                                                                    target_vip: vip,
                                                                    sent_at: Instant::now(),
                                                                    retries: 0,
                                                                    max_retries: disco_config.max_retries,
                                                                };
                                                                pending_pings.write().await.insert(tx_id, pending);
                                                            }
                                                        }
                                                    }
                                                    peer_state.state = PeerConnectionState::DirectTry;
                                                }
                                                ConnectionStrategy::Direct => {
                                                    // Standard disco ping - send to all known endpoints
                                                    let endpoints: Vec<_> = peer_state.endpoints.endpoints.iter()
                                                        .map(|e| e.addr)
                                                        .collect();

                                                    if endpoints.is_empty() {
                                                        // No endpoints, configure WireGuard without endpoint
                                                        // (peer may initiate connection to us)
                                                        let pubkey = ::hex::encode(peer_state.public_key);
                                                        let mut allowed_ips = vec![format!("{}/32", vip)];
                                                        if let Some(v6) = vip_v6 {
                                                            allowed_ips.push(format!("{}/128", v6));
                                                        }
                                                        let _ = tun_ctrl
                                                            .add_peer(&pubkey, None, &allowed_ips)
                                                            .await;
                                                    } else {
                                                        info!(
                                                            "Peer {} using Direct strategy - sending disco pings to {} endpoints",
                                                            vip, endpoints.len()
                                                        );
                                                        
                                                        // CRITICAL FIX: Proactively configure WireGuard with the first
                                                        // endpoint BEFORE waiting for disco pong. This ensures:
                                                        // 1. If peer can reach us but we can't reach them (asymmetric NAT),
                                                        //    at least incoming traffic will work
                                                        // 2. WireGuard handshake can begin immediately, potentially
                                                        //    establishing the tunnel before disco completes
                                                        // 3. The peer will be updated with a better endpoint if disco
                                                        //    pong confirms connectivity with lower latency
                                                        let first_endpoint = endpoints.first().copied();
                                                        {
                                                            let pubkey = ::hex::encode(peer_state.public_key);
                                                            let mut allowed_ips = vec![format!("{}/32", vip)];
                                                            if let Some(v6) = vip_v6 {
                                                                allowed_ips.push(format!("{}/128", v6));
                                                            }
                                                            info!(
                                                                "Pre-configuring WireGuard peer {} with endpoint {:?} (disco in progress)",
                                                                vip, first_endpoint
                                                            );
                                                            let _ = tun_ctrl
                                                                .add_peer(&pubkey, first_endpoint, &allowed_ips)
                                                                .await;
                                                        }
                                                        
                                                    for endpoint in endpoints {
                                                        let tx_id: [u8; 12] = rand::random();
                                                        let ping = DiscoPing {
                                                            tx_id,
                                                            sender_key: our_public_key,
                                                            sender_vip: our_vip.unwrap_or(Ipv4Addr::UNSPECIFIED),
                                                            sender_vip_v6: our_vip_v6,
                                                            wg_port: wg_listen_port,
                                                        };

                                                            if let Ok(ping_data) = encode_disco_ping(&ping) {
                                                                if let Err(e) = socket_for_disco.send_to(&ping_data, endpoint).await {
                                                                    warn!("Failed to send disco ping to {}: {}", endpoint, e);
                                                                } else {
                                                                    info!(
                                                                        "Sent disco ping to {} (VIP: {}) tx: {:02x?}",
                                                                        endpoint, vip, &tx_id[..4]
                                                                    );
                                                                    peer_state.mark_probing(endpoint);
                                                                    let pending = PendingDiscoPing {
                                                                        tx_id,
                                                                        target: endpoint,
                                                                        target_vip: vip,
                                                                        sent_at: Instant::now(),
                                                                        retries: 0,
                                                                        max_retries: disco_config.max_retries,
                                                                    };
                                                                    pending_pings.write().await.insert(tx_id, pending);
                                                                }
                                                            }
                                                        }
                                                        peer_state.state = PeerConnectionState::DirectTry;
                                                    }
                                                }
                                            }
                                        }
                                        // Handle removed peers from heartbeat ACK
                                        if !update.removed_vips.is_empty() {
                                            let mut peers = peer_states.write().await;
                                            for vip in &update.removed_vips {
                                                if peers.remove(vip).is_some() {
                                                    info!("Removed peer {} (departed from cluster)", vip);
                                                } else {
                                                    debug!("Ignoring removal of unknown peer {}", vip);
                                                }
                                            }
                                        }
                                    }
                                    Ok(None) => {
                                        warn!(
                                            "Received unhandled signaling message type 0x{:02x} from {}",
                                            first_byte, src
                                        );
                                    }
                                    Err(e) => {
                                        warn!(
                                            "Failed to handle signaling message 0x{:02x} from {}: {}",
                                            first_byte, src, e
                                        );
                                    }
                                }
                                continue;
                            }
                                if (0x01..=0x04).contains(&first_byte) {
                                    // WireGuard packet type-specific logging (matching OmniNervous wg.rs)
                                    let pkt_len = pkt.len();
                                    match first_byte {
                                        0x01 => info!("[WG-RX] HandshakeInit ({} bytes) from {}", pkt_len, src),
                                        0x02 => info!("[WG-RX] HandshakeResponse ({} bytes) from {}", pkt_len, src),
                                        0x03 => debug!("[WG-RX] CookieReply ({} bytes) from {}", pkt_len, src),
                                        0x04 => trace!("[WG-RX] Data ({} bytes) from {}", pkt_len, src),
                                        _ => {}
                                    }
                                    let _ = tun_ctrl.handle_packet(pkt, src, &socket_inner).await;
                                } else {
                                    debug!("Ignored unknown packet type {} from {}", first_byte, src);
                                }
                            }
                            Err(e) => {
                                error!("Master Dispatcher socket error: {}", e);
                            }
                         }
                    }
                    // Periodic check for disco timeouts and retries
                    _ = disco_check_interval.tick() => {
                        let now = Instant::now();
                        let mut pings = pending_pings.write().await;
                        let mut peers = peer_states.write().await;
                        let mut timed_out: Vec<[u8; 12]> = Vec::new();

                        for (tx_id, pending) in pings.iter_mut() {
                            if now.duration_since(pending.sent_at) > disco_config.ping_timeout {
                                if pending.retries < pending.max_retries {
                                    // Retry the ping
                                    pending.retries += 1;
                                    pending.sent_at = now;

                                    let ping = DiscoPing {
                                        tx_id: *tx_id,
                                        sender_key: our_public_key,
                                        sender_vip: our_vip.unwrap_or(Ipv4Addr::UNSPECIFIED),
                                        sender_vip_v6: our_vip_v6,
                                        wg_port: wg_listen_port,
                                    };

                                    if let Ok(ping_data) = encode_disco_ping(&ping) {
                                        if let Err(e) = socket_for_disco.try_send_to(&ping_data, pending.target) {
                                            warn!("Failed to retry disco ping to {}: {}", pending.target, e);
                                        } else {
                                            info!(
                                                "Retrying disco ping to {} ({}) attempt {}/{}",
                                                pending.target_vip, pending.target,
                                                pending.retries, pending.max_retries
                                            );
                                        }
                                    }
                                } else {
                                // Max retries exceeded - mark for removal
                                    timed_out.push(*tx_id);

                                    // Mark endpoint as failed and check if we need relay
                                    if let Some(peer_state) = peers.get_mut(&pending.target_vip) {
                                        warn!(
                                            "Disco ping to {} ({}) timed out after {} retries",
                                            pending.target_vip, pending.target, pending.max_retries
                                        );

                                        // Mark this specific endpoint as failed
                                        peer_state.mark_endpoint_failed(pending.target);

                                        // Check if all endpoints have failed and we need relay
                                        if peer_state.endpoints.needs_relay() {
                                            if disco_config.relay_enabled {
                                                peer_state.state = PeerConnectionState::RelayTry;
                                                info!(
                                                    "All {} endpoints failed for peer {} - initiating relay fallback",
                                                    peer_state.endpoint_count(), pending.target_vip
                                                );
                                            } else {
                                                peer_state.state = PeerConnectionState::Failed;
                                                // Configure WireGuard with best available endpoint as last resort
                                                // This is critical - even without disco confirmation, we should
                                                // attempt to configure WireGuard so the tunnel can be established
                                                // The peer may be able to reach us even if our disco pings failed
                                                let pubkey = ::hex::encode(peer_state.public_key);
                                                let mut allowed_ips = vec![format!("{}/32", peer_state.vip)];
                                                if let Some(v6) = peer_state.vip_v6 {
                                                    allowed_ips.push(format!("{}/128", v6));
                                                }
                                                
                                                // Use best known endpoint, or None if all failed
                                                let endpoint = peer_state.best_endpoint();
                                                info!(
                                                    "Configuring WireGuard peer {} with endpoint {:?} (disco failed, relay disabled)",
                                                    peer_state.vip, endpoint
                                                );
                                                let _ = tun_ctrl
                                                    .add_peer(&pubkey, endpoint, &allowed_ips)
                                                    .await;
                                            }
                                        } else {
                                            debug!(
                                                "Endpoint {} failed for peer {}, but {} other endpoints available",
                                                pending.target, pending.target_vip,
                                                peer_state.endpoint_count() - 1
                                            );
                                        }
                                    }
                                }
                            }
                        }

                        // Remove timed out pings
                        for tx_id in timed_out {
                            pings.remove(&tx_id);
                        }

                        // Collect peers that need relay (outside the borrow scope)
                        let peers_needing_relay: Vec<(Ipv4Addr, [u8; 32])> = peers
                            .iter()
                            .filter(|(_, p)| p.state == PeerConnectionState::RelayTry)
                            .map(|(vip, p)| (*vip, p.public_key))
                            .collect();

                        // Drop the locks before async operations
                        drop(pings);
                        drop(peers);

                        // Send RELAY_BIND requests for peers needing relay
                        if !peers_needing_relay.is_empty() {
                            if let Some(relay_addr) = relay_server_addr {
                                let mut relay = relay_client.write().await;
                                if let Some(client) = relay.as_mut() {
                                    for (target_vip, target_key) in peers_needing_relay {
                                        // Always use transparent relay for omniedge clients
                                        // Both kernel and userspace WireGuard send raw WG packets,
                                        // so transparent relay (which forwards raw packets by source address)
                                        // is the correct mode. Standard relay expects RELAY_DATA encapsulation
                                        // which neither kernel nor BoringTun userspace WG provides.
                                        info!(
                                            "Requesting TRANSPARENT relay for peer {} (port: {:?})",
                                            target_vip, wg_listen_port
                                        );
                                        let bind_req = client.create_bind_request_with_mode(
                                            target_key,
                                            target_vip,
                                            true,  // transparent mode - raw WG packet forwarding
                                            wg_listen_port,
                                        );

                                        if let Ok(bind_data) = encode_relay_bind(&bind_req) {
                                            if let Err(e) = socket_for_disco.send_to(&bind_data, relay_addr).await {
                                                warn!("Failed to send RELAY_BIND for {} to {}: {}", target_vip, relay_addr, e);
                                            } else {
                                                info!(
                                                    "Sent RELAY_BIND for peer {} to relay server {} (transparent: true)",
                                                    target_vip, relay_addr
                                                );
                                            }
                                        }
                                    }
                                } else {
                                    warn!("Relay client not initialized, cannot send RELAY_BIND");
                                }
                            }
                        }
                    }
                    _ = shutdown_rx1.recv() => {
                        info!("Master Dispatcher Loop shutting down");
                        break;
                    }
                }
            }
        });
        self.task_handles.push(dispatcher_handle);

        // TUN Transmission Loop (TUN -> network) remains necessary for outgoing traffic
        // Skip if the TUN loop is already active from a previous connection (Windows reconnect)
        if !skip_tun_loop {
            let mut tun_tx = tun.clone();
            let socket_tx = socket.clone();
            let mut shutdown_rx2 = shutdown_tx.subscribe();
            let tun_handle = tokio::spawn(async move {
                tokio::select! {
                    _ = tun_tx.start_loop(socket_tx) => {}
                    _ = shutdown_rx2.recv() => {
                        info!("TUN Transmission Loop shutting down");
                    }
                }
            });
            self.task_handles.push(tun_handle);
        } else {
            info!("Skipping TUN loop spawn - already active from previous connection");
        }

        let api_client = self.api_client.as_ref().cloned();
        let proto_hb = proto.clone();
        let socket_hb = socket.clone();
        let is_nucleus_hb = self.is_nucleus;
        let as_exit_node_hb = self.as_exit_node.clone();
        // Use hardware_id for API heartbeats, not device_id (API UUID)
        let hardware_id_hb = hardware_id.clone();
        let peer_states_hb = self.peer_states.clone();

        // Heartbeat/Poll/Role Loop
        let mut shutdown_rx3 = shutdown_tx.subscribe();
        let heartbeat_handle = tokio::spawn(async move {
            let mut api_interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
            let mut proto_interval = tokio::time::interval(tokio::time::Duration::from_secs(30));

            if is_nucleus_hb {
                info!("Running in DUAL MODE: Edge client + Nucleus signaling server active.");
            }

            loop {
                tokio::select! {
                    _ = api_interval.tick() => {
                        if let Some(ref client) = api_client {
                            let ds = DeviceService::new(client);
                            let is_exit = as_exit_node_hb.load(std::sync::atomic::Ordering::SeqCst);
                            let _ = ds.heartbeat(&hardware_id_hb, is_exit).await;
                        }
                    }
                    _ = hb_rx.recv() => {
                        if let Some(ref client) = api_client {
                            info!("Triggering immediate heartbeat...");
                            let ds = DeviceService::new(client);
                            let is_exit = as_exit_node_hb.load(std::sync::atomic::Ordering::SeqCst);
                            let _ = ds.heartbeat(&hardware_id_hb, is_exit).await;
                        }
                    }
                    _ = proto_interval.tick() => {
                        // Pass actual peer count to Nucleus for better peer discovery
                        let peer_count = peer_states_hb.read().await.len() as u32;
                        let _ = proto_hb.heartbeat(&socket_hb, peer_count).await;
                    }
                    _ = shutdown_rx3.recv() => {
                        info!("Heartbeat Loop shutting down");
                        break;
                    }
                }
            }
        });
        self.task_handles.push(heartbeat_handle);

        // ====================================================================
        // Port Mapping Refresh Loop (NAT Traversal Fix v0.3.4)
        // ====================================================================
        // Periodically refresh port mappings before they expire.
        // Check every 30 minutes; actual refresh happens at 50% of mapping lifetime.
        // CRITICAL: When external port changes, update proto and send REGISTER to Nucleus!
        if self.network_config.portmap_enabled {
            let port_mapper = self.port_mapper.clone();
            let port_mapping = self.port_mapping.clone();
            let proto_pm = proto.clone();
            let socket_pm = socket.clone();
            let mut shutdown_rx4 = shutdown_tx.subscribe();

            let portmap_handle = tokio::spawn(async move {
                // Check every 30 minutes (1800 seconds)
                let mut refresh_interval =
                    tokio::time::interval(tokio::time::Duration::from_secs(1800));
                // Skip the first immediate tick
                refresh_interval.tick().await;

                loop {
                    tokio::select! {
                        _ = refresh_interval.tick() => {
                            let mut mapper_guard = port_mapper.write().await;
                            if let Some(mapper) = mapper_guard.as_mut() {
                                // Get old external port before refresh
                                let old_ext_port = proto_pm.get_external_port().await;

                                match mapper.check_and_refresh().await {
                                    Ok(refreshed) => {
                                        if refreshed {
                                            if let Some(mapping) = mapper.current_mapping() {
                                                info!(
                                                    "Port mapping refreshed: external port {} (gateway: {})",
                                                    mapping.external_port, mapping.gateway
                                                );
                                                let mut pm = port_mapping.write().await;
                                                *pm = Some(mapping.clone());

                                                // Check if external port changed
                                                if old_ext_port != Some(mapping.external_port) {
                                                    info!(
                                                        "External port changed: {:?} -> {}, updating Nucleus",
                                                        old_ext_port, mapping.external_port
                                                    );

                                                    // Update proto with new external port/addr
                                                    proto_pm.set_external_port(mapping.external_port).await;
                                                    let ext_addr = format!("{}:{}", mapping.gateway, mapping.external_port);
                                                    proto_pm.set_external_addr(ext_addr.clone()).await;

                                                    // Send REGISTER to notify Nucleus of new endpoint
                                                    if let Err(e) = proto_pm.register(&socket_pm).await {
                                                        warn!("Failed to send REGISTER after port mapping refresh: {}", e);
                                                    } else {
                                                        info!("Sent REGISTER with new external endpoint: {}", ext_addr);
                                                    }
                                                }
                                            }
                                        } else {
                                            debug!("Port mapping still valid, no refresh needed");
                                        }
                                    }
                                    Err(e) => {
                                        warn!("Port mapping refresh failed: {}", e);
                                        // Clear the mapping since it may have expired
                                        let mut pm = port_mapping.write().await;
                                        *pm = None;

                                        // Also clear external port/addr in proto so Nucleus knows
                                        // we no longer have a port mapping
                                        let old_port = proto_pm.get_external_port().await;
                                        if old_port.is_some() {
                                            warn!("Port mapping lost, clearing external endpoint from Nucleus");
                                            proto_pm.set_external_port(0).await;
                                            proto_pm.set_external_addr(String::new()).await;
                                            // Notify Nucleus
                                            let _ = proto_pm.register(&socket_pm).await;
                                        }
                                    }
                                }
                            }
                        }
                        _ = shutdown_rx4.recv() => {
                            info!("Port Mapping Refresh Loop shutting down");
                            break;
                        }
                    }
                }
            });
            self.task_handles.push(portmap_handle);
            debug!("Port mapping refresh loop started (30 min interval)");
        }
    }

    pub async fn login_with_password(&mut self, email: &str, password: &str) -> Result<AuthResp> {
        self.set_state(ConnectionState::Authenticating).await;
        let client = ApiClient::new(self.base_url.clone(), None);
        let auth = AuthService::new(&client);
        let resp = auth.login_with_password(email, password).await?;

        self.api_client = Some(ApiClient::new(
            self.base_url.clone(),
            Some(resp.effective_token().to_string()),
        ));
        self.set_state(ConnectionState::Authenticated).await;
        Ok(resp)
    }

    pub async fn start_device_flow(&self) -> Result<DeviceCodeResp> {
        let client = ApiClient::new(self.base_url.clone(), None);
        let auth = AuthService::new(&client);
        auth.device_flow_init("omniedge-cli", "openid profile email offline_access")
            .await
    }

    pub async fn poll_device_flow(&mut self, device_code: &str) -> Result<AuthResp> {
        let client = ApiClient::new(self.base_url.clone(), None);
        let auth = AuthService::new(&client);
        let resp = auth.device_flow_token("omniedge-cli", device_code).await?;

        self.api_client = Some(ApiClient::new(
            self.base_url.clone(),
            Some(resp.effective_token().to_string()),
        ));
        self.set_state(ConnectionState::Authenticated).await;
        Ok(resp)
    }

    pub async fn start_session_login(&self) -> Result<SessionResponse> {
        let client = ApiClient::new(self.base_url.clone(), None);
        let auth = AuthService::new(&client);
        auth.generate_session().await
    }

    pub async fn handle_login_token(
        &mut self,
        token_resp: WebSocketTokenResponse,
    ) -> Result<AuthResp> {
        info!("Handling login token from WebSocket...");
        let auth_resp = AuthResp {
            token: token_resp.token.clone(),
            refresh_token: token_resp.refresh_token.clone(),
            access_token: token_resp.token.clone(),
            id_token: "".to_string(),
            expires_in: 3600,
            email: None,
            user_id: None,
        };

        self.api_client = Some(ApiClient::new(
            self.base_url.clone(),
            Some(auth_resp.effective_token().to_string()),
        ));

        // Save to config immediately
        if let Ok(mut config) = CliConfig::load() {
            info!("Saving login tokens to native storage...");
            config.auth_response = Some(auth_resp.clone());
            let _ = config.save();
        }

        self.set_state(ConnectionState::Authenticated).await;
        info!("Authentication successful via session login.");
        Ok(auth_resp)
    }

    pub async fn wait_for_session_login(
        base_url: &str,
        session_id: &str,
        mut cancel_rx: tokio::sync::oneshot::Receiver<()>,
    ) -> Result<WebSocketTokenResponse> {
        use futures_util::{SinkExt, StreamExt};
        use tokio::time::{timeout, Duration};
        use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

        let ws_url = if base_url.contains("localhost") || base_url.contains("127.0.0.1") {
            format!("ws://127.0.0.1:8080/auth/login/session/{}", session_id)
        } else {
            let client = ApiClient::new(base_url.to_string(), None);
            client.ws_url(&format!("/auth/login/session/{}", session_id))
        };

        info!(
            "Connecting to WebSocket for session login (ID: {}). URL: {}",
            session_id, ws_url
        );

        use tokio_tungstenite::tungstenite::client::IntoClientRequest;
        let mut request = ws_url.into_client_request()?;
        let headers = request.headers_mut();
        headers.insert("User-Agent", "OmniEdge/2.0.0".parse().unwrap());
        headers.insert("Origin", "https://connect.omniedge.io".parse().unwrap());

        let connect_future = connect_async(request);
        let (ws_stream, _) = timeout(Duration::from_secs(15), connect_future)
            .await
            .context("WebSocket connection timed out during handshake")?
            .context("Failed to connect to login WebSocket")?;

        info!("WebSocket connection established for session login. Waiting for browser login...");

        let (mut write, mut read) = ws_stream.split();

        // Create a cancellation token for the ping task (internal)
        let (ping_cancel_tx, mut ping_cancel_rx) = tokio::sync::oneshot::channel::<()>();

        // Ping loop to keep connection alive (will be cancelled when login completes)
        let ping_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Err(e) = write.send(Message::Ping(vec![])).await {
                            debug!("WebSocket ping loop stopping: {}", e);
                            break;
                        }
                        debug!("Sent WebSocket ping");
                    }
                    _ = &mut ping_cancel_rx => {
                        debug!("WebSocket ping loop cancelled");
                        // Try to close the WebSocket gracefully
                        let _ = write.send(Message::Close(None)).await;
                        break;
                    }
                }
            }
        });

        // Wait for token message with timeout
        let wait_future = async {
            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        info!("Received WebSocket Text message: {}", text);
                        match serde_json::from_str::<WebSocketTokenResponse>(&text) {
                            Ok(tokens) => {
                                if !tokens.token.is_empty() {
                                    info!("Successfully received tokens via WebSocket.");
                                    return Ok(tokens);
                                } else {
                                    warn!(
                                        "Received WebSocket message but token field was empty: {}",
                                        text
                                    );
                                }
                            }
                            Err(e) => {
                                info!(
                                    "Could not parse WebSocket message as tokens: {} (Raw: {})",
                                    e, text
                                );
                            }
                        }
                    }
                    Ok(Message::Binary(bin)) => {
                        let text = String::from_utf8_lossy(&bin);
                        info!("Received WebSocket Binary message: {}", text);
                        match serde_json::from_str::<WebSocketTokenResponse>(&text) {
                            Ok(tokens) => {
                                if !tokens.token.is_empty() {
                                    info!("Successfully received tokens via WebSocket.");
                                    return Ok(tokens);
                                }
                            }
                            Err(e) => {
                                info!("Could not parse binary WebSocket message as tokens: {}", e);
                            }
                        }
                    }
                    Ok(Message::Close(frame)) => {
                        info!("WebSocket closed by server: {:?}", frame);
                        return Err(anyhow::anyhow!("WebSocket closed by server: {:?}", frame));
                    }
                    Ok(Message::Pong(_)) => {
                        // Pong is expected response to our ping, no need to log at info level
                        debug!("Received WebSocket Pong");
                    }
                    Err(e) => {
                        error!("WebSocket error: {}", e);
                        return Err(anyhow::anyhow!("WebSocket error: {}", e));
                    }
                    msg => {
                        debug!("Received other WebSocket message: {:?}", msg);
                    }
                }
            }
            Err(anyhow::anyhow!("WebSocket closed without receiving tokens"))
        };

        // 15 minutes timeout to match Go implementation, with cancellation support
        let result = tokio::select! {
            res = timeout(Duration::from_secs(900), wait_future) => {
                match res {
                    Ok(r) => r,
                    Err(_) => {
                        error!(
                            "Login session timed out after 15 minutes for session {}",
                            session_id
                        );
                        Err(anyhow::anyhow!(
                            "Login session timed out after 15 minutes. Please try again."
                        ))
                    }
                }
            }
            _ = &mut cancel_rx => {
                info!("Login session cancelled by user for session {}", session_id);
                Err(anyhow::anyhow!("Login cancelled by user"))
            }
        };

        // Cancel the ping task
        let _ = ping_cancel_tx.send(());
        ping_handle.abort();

        result
    }

    pub async fn get_networks(&self) -> Result<Vec<VirtualNetworkResponse>> {
        let client = self.api_client.as_ref().context("Not authenticated")?;
        let net_service = NetworkService::new(client);
        net_service.list_all().await
    }

    pub async fn get_profile(&self) -> Result<ProfileResponse> {
        let client = self.api_client.as_ref().context("Not authenticated")?;
        let auth_service = AuthService::new(client);
        auth_service.me().await
    }

    pub async fn get_network_devices(
        &self,
        network_id: &str,
    ) -> Result<Vec<VirtualNetworkDeviceResponse>> {
        let client = self.api_client.as_ref().context("Not authenticated")?;
        let net_service = NetworkService::new(client);
        net_service.get_devices(network_id).await
    }

    pub async fn set_exit_node(
        &mut self,
        network_id: &str,
        exit_node_id: &str,
        exit_node_ip: Option<&str>,
        exit_node_ip_v6: Option<&str>,
    ) -> Result<()> {
        let client = self.api_client.as_ref().context("Not authenticated")?;
        let net_service = NetworkService::new(client);
        let device_id = self.device_id.as_deref().context("Device ID not set")?;

        let node_id = if exit_node_id.is_empty() {
            None
        } else {
            Some(exit_node_id)
        };
        net_service
            .select_exit_node(network_id, device_id, node_id)
            .await?;

        // Update local state
        self.exit_node_ip = exit_node_ip.map(|s| s.to_string());
        self.exit_node_ip_v6 = exit_node_ip_v6.map(|s| s.to_string());

        // Refresh routing if connected
        if let ConnectionState::Connected = *self.state.read().await {
            if let Some(ip) = exit_node_ip {
                info!(
                    "Enabling exit node routing to: {} (v6: {:?})",
                    ip, exit_node_ip_v6
                );
                // We need the nucleus host to add a persistent route to it
                // For simplicity, we can try to get it from the current proto if available
                if let Some(ref proto) = self.proto {
                    let _ = crate::routing::RoutingManager::setup_exit_node(
                        ip,
                        exit_node_ip_v6,
                        proto.get_nucleus_host(),
                    );
                }
            } else {
                info!("Restoring original routing (no exit node)");
                let _ = crate::routing::RoutingManager::restore_exit_node();
            }
        }
        Ok(())
    }

    pub async fn set_as_exit_node(&mut self, enabled: bool) -> Result<()> {
        info!("Setting as_exit_node to: {}", enabled);
        self.as_exit_node.store(enabled, Ordering::SeqCst);

        // Persist to config
        if let Ok(mut config) = CliConfig::load() {
            config.is_exit_node = enabled;
            let _ = config.save();
        }

        // Sync with backend if connected
        // IMPORTANT: Must send heartbeat FIRST to update device's is_exit_node status,
        // then call update_device() to allow it in the network
        let current_net_id = self.current_network_id.read().await.clone();
        if let (Some(client), Some(net_id), Some(dev_id), Some(hw_id)) = (
            &self.api_client,
            &current_net_id,
            &self.device_id,
            &self.hardware_id,
        ) {
            // Step 1: Send heartbeat with new is_exit_node status and wait for it
            // Use hardware_id for heartbeat, not device_id (API UUID)
            let dev_service = DeviceService::new(client);
            match dev_service.heartbeat(hw_id, enabled).await {
                Ok(_) => {
                    info!("Heartbeat sent with is_exit_node={}", enabled);
                }
                Err(e) => {
                    error!("Failed to send heartbeat with exit node status: {}", e);
                    // Continue anyway, the periodic heartbeat will eventually sync
                }
            }

            // Step 2: Now update the device in the network
            let net_service = NetworkService::new(client);
            if let Err(e) = net_service.update_device(net_id, dev_id, enabled).await {
                error!("Failed to sync exit node status to backend: {}", e);
                // We continue because local state is updated, but this indicates a sync issue
            } else {
                info!("Successfully synced exit node status to backend.");
            }
        }

        Ok(())
    }

    pub fn is_exit_node(&self) -> bool {
        self.as_exit_node.load(Ordering::SeqCst)
    }

    pub async fn get_connected_network_id(&self) -> Option<String> {
        self.current_network_id.read().await.clone()
    }

    pub async fn get_devices(&self) -> Result<Vec<DeviceResponse>> {
        let client = self.api_client.as_ref().context("Not authenticated")?;
        let dev_service = DeviceService::new(client);
        dev_service.list_all().await
    }

    pub async fn get_virtual_ip(&self) -> String {
        // First priority: active session IP
        if let Some(ref ip) = *self.virtual_ip.read().await {
            return ip.clone();
        }

        // Fallback to last recorded IP in config
        if let Ok(config) = CliConfig::load() {
            if let Some(info) = config.last_join_info {
                return info.virtual_ip;
            }
        }
        "".to_string()
    }

    /// Get the IPv6 virtual IP address (dual-stack support)
    pub async fn get_virtual_ip_v6(&self) -> Option<String> {
        self.virtual_ip_v6.read().await.clone()
    }

    pub fn get_identity_private_key(&self) -> [u8; 32] {
        self.identity.private_key_bytes()
    }

    pub fn get_base_url(&self) -> &str {
        &self.base_url
    }

    /// Configure nucleus settings for dual mode operation
    pub fn set_nucleus_config(&mut self, port: u16, secret: Option<String>) {
        self.nucleus_port = port;
        self.cluster_secret = secret;
    }

    pub async fn disconnect(&mut self) -> Result<()> {
        self.set_state(ConnectionState::Stopping).await;

        // First, send shutdown signal to all background loops
        if let Some(tx) = self.shutdown_tx.take() {
            info!("Sending shutdown signal to background loops...");
            let _ = tx.send(());
        }

        // Wait for all background tasks to complete (with timeout)
        // This ensures they drop their TUN references
        if !self.task_handles.is_empty() {
            info!(
                "Waiting for {} background tasks to complete...",
                self.task_handles.len()
            );
            let handles = std::mem::take(&mut self.task_handles);
            
            // Wait for all tasks in parallel with a single 3-second timeout
            // This is much faster than waiting for each serially
            let _ = tokio::time::timeout(
                tokio::time::Duration::from_secs(3),
                futures::future::join_all(handles)
            ).await;
            
            info!("All background tasks completed or timed out");
        }

        // On Windows, we keep the TUN adapter alive to prevent accumulation of
        // "wintun", "wintun 2", "wintun 3" etc. on repeated connect/disconnect.
        // WinTun adapters are not automatically deleted when dropped, so we reuse them.
        // On macOS/Linux, we shutdown properly as the kernel handles cleanup.
        #[cfg(not(target_os = "windows"))]
        {
            // Shutdown the TUN properly - this closes the file descriptor
            // and causes macOS to remove the utun interface
            if let Some(ref tun) = self.tun {
                info!("Shutting down TUN interface...");
                tun.shutdown().await;
                info!("TUN interface shutdown complete");
            }
            self.tun = None;
        }

        #[cfg(target_os = "windows")]
        {
            // On Windows, use soft_shutdown to keep the TUN adapter alive.
            // WinTun adapters persist and create "wintun 2", "wintun 3" etc. if we
            // fully shutdown and recreate. Instead, keep the adapter/tasks alive
            // but clear peers so no traffic flows. On reconnect, we'll reconfigure peers.
            if let Some(ref tun) = self.tun {
                info!("Windows: Soft shutdown TUN (keeping adapter alive for reconnect)...");
                tun.soft_shutdown().await;
                info!("Windows: TUN soft shutdown complete - adapter still active");
                // Keep self.tun reference for reconnect
            }
        }

        // Now drop the protocol reference
        self.proto = None;

        {
            let mut nid = self.current_network_id.write().await;
            *nid = None;
        }
        {
            let mut vip = self.virtual_ip.write().await;
            *vip = None;
        }
        {
            let mut vip_v6 = self.virtual_ip_v6.write().await;
            *vip_v6 = None;
        }

        // Clear peer tracking state
        {
            let mut peers = self.peer_states.write().await;
            peers.clear();
        }
        {
            let mut pings = self.pending_pings.write().await;
            pings.clear();
        }

        // ====================================================================
        // Release Port Mapping (NAT Traversal Fix v0.3.4)
        // ====================================================================
        // Release the port mapping to free up resources on the NAT gateway.
        // This is important for NAT-PMP which has a limited number of mappings.
        if let Err(e) = self.release_port_mapping().await {
            warn!("Failed to release port mapping during disconnect: {}", e);
        }
        // Clear relay sessions
        {
            let mut sessions = self.relay_sessions.write().await;
            sessions.clear();
        }
        // Clear relay client
        {
            let mut client = self.relay_client.write().await;
            *client = None;
        }

        // On Windows, don't run cleanup_adapters during normal disconnect
        // Only cleanup on app exit to prevent adapter accumulation
        #[cfg(not(target_os = "windows"))]
        {
            let _ = self.cleanup_adapters();
        }

        if self.exit_node_ip.is_some() {
            let _ = crate::routing::RoutingManager::restore_exit_node();
        }

        self.set_state(ConnectionState::Disconnected).await;
        Ok(())
    }

    pub fn cleanup_adapters(&self) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            info!("Cleaning up all OmniEdge/WinTun network adapters (Windows)...");

            // Method 1: Use WinTun API to properly close adapters
            // This is the most reliable method as it uses the same API that created them
            let closed_omniedge = omni_tun::windows::delete_wintun_adapters("OmniEdge");
            let closed_wintun = omni_tun::windows::delete_wintun_adapters("wintun");
            let total_closed = closed_omniedge + closed_wintun;
            if total_closed > 0 {
                info!("Closed {} adapter(s) via WinTun API", total_closed);
            }

            // Method 2: Try to disable via PowerShell as fallback
            let ps_cmd = "Get-NetAdapter -IncludeHidden | Where-Object { $_.Name -like 'wintun*' -or $_.Name -like 'OmniEdge*' } | ForEach-Object { Disable-NetAdapter -Name $_.Name -Confirm:$false -ErrorAction SilentlyContinue }";
            let _ = std::process::Command::new("powershell")
                .args(["-Command", ps_cmd])
                .output();

            // Method 3: Use pnputil to remove WinTun/OmniEdge device instances
            let pnp_find = r#"Get-PnpDevice -FriendlyName '*wintun*' -ErrorAction SilentlyContinue | ForEach-Object { pnputil /remove-device $_.InstanceId 2>$null }; Get-PnpDevice -FriendlyName '*OmniEdge*' -ErrorAction SilentlyContinue | ForEach-Object { pnputil /remove-device $_.InstanceId 2>$null }"#;
            let _ = std::process::Command::new("powershell")
                .args(["-Command", pnp_find])
                .output();
        }

        #[cfg(target_os = "linux")]
        {
            info!("Cleaning up all OmniEdge network adapters (Linux)...");
            // Find all OmniEdge interfaces and delete them
            let output = std::process::Command::new("sh")
                .arg("-c")
                .arg("ip link show | grep -oE 'omniedge[0-9]*'")
                .output();

            if let Ok(out) = output {
                let list = String::from_utf8_lossy(&out.stdout);
                for iface in list.lines() {
                    let iface = iface.trim();
                    if !iface.is_empty() {
                        debug!("Deleting linux interface: {}", iface);
                        // Already running as root (require_root_privileges called before)
                        let _ = std::process::Command::new("ip")
                            .args(["link", "delete", iface])
                            .output();
                    }
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            info!("Cleaning up all OmniEdge network adapters (macOS)...");
            // macOS utun interfaces are kernel-managed and automatically destroyed when
            // the owning process terminates. However, we can try to identify and bring down
            // any interfaces that have our expected virtual IP.
            //
            // Note: On macOS, we cannot directly delete utun interfaces - they are cleaned up
            // by the kernel when the file descriptor is closed. Killing the daemon process
            // (done in stop_and_cleanup_service) is the proper way to clean up.

            // Try to find and bring down any utun interfaces with 100.x.x.x addresses
            // (OmniEdge virtual network range)
            let output = std::process::Command::new("sh")
                .arg("-c")
                .arg("ifconfig | grep -B1 'inet 100\\.' | grep -E '^utun[0-9]+' | cut -d: -f1")
                .output();

            if let Ok(out) = output {
                let list = String::from_utf8_lossy(&out.stdout);
                for iface in list.lines() {
                    let iface = iface.trim();
                    if !iface.is_empty() {
                        debug!("Bringing down macOS interface: {}", iface);
                        // Bring interface down - this doesn't delete it but stops traffic
                        let _ = std::process::Command::new("ifconfig")
                            .args([iface, "down"])
                            .output();
                    }
                }
            }
        }

        Ok(())
    }

    /// Check if a TUN interface already exists with the given virtual IP
    /// Returns the interface name if found, None otherwise
    pub fn find_interface_with_ip(vip: &str) -> Option<String> {
        #[cfg(target_os = "macos")]
        {
            // On macOS, check for utun interfaces with the given IP
            let output = std::process::Command::new("sh")
                .arg("-c")
                .arg(format!(
                    "ifconfig | grep -B5 'inet {}' | grep -E '^utun[0-9]+' | head -1 | cut -d: -f1",
                    vip
                ))
                .output();

            if let Ok(out) = output {
                let iface = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !iface.is_empty() {
                    info!("Found existing interface {} with IP {}", iface, vip);
                    return Some(iface);
                }
            }
        }

        #[cfg(target_os = "linux")]
        {
            // On Linux, check for omniedge interfaces with the given IP
            let output = std::process::Command::new("sh")
                .arg("-c")
                .arg(format!("ip addr show | grep -B2 'inet {}/24' | grep -E 'omniedge[0-9]*' | head -1 | awk '{{print $2}}' | tr -d ':'", vip))
                .output();

            if let Ok(out) = output {
                let iface = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !iface.is_empty() {
                    info!("Found existing interface {} with IP {}", iface, vip);
                    return Some(iface);
                }
            }
        }

        #[cfg(target_os = "windows")]
        {
            // On Windows, check for OmniEdge adapters with the given IP
            let output = std::process::Command::new("powershell")
                .args(["-Command", &format!(
                    "Get-NetIPAddress -IPAddress '{}' -ErrorAction SilentlyContinue | Select-Object -ExpandProperty InterfaceAlias",
                    vip
                )])
                .output();

            if let Ok(out) = output {
                let iface = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !iface.is_empty() && iface.contains("OmniEdge") {
                    info!("Found existing interface {} with IP {}", iface, vip);
                    return Some(iface);
                }
            }
        }

        None
    }

    /// Check if we're already connected (have an active TUN)
    pub fn is_connected(&self) -> bool {
        self.tun.is_some()
    }

    /// Create a dual-stack UDP socket that supports both IPv4 and IPv6
    ///
    /// This binds to [::]:0 which on most systems accepts both IPv4 and IPv6 traffic.
    /// On Windows, we need to explicitly disable IPV6_V6ONLY to enable dual-stack.
    /// On Linux/macOS, dual-stack is typically the default behavior.
    async fn create_dual_stack_socket() -> Result<UdpSocket> {
        use std::net::{Ipv6Addr, SocketAddrV6};

        // Create a socket bound to IPv6 any address with port 0 (auto-assign)
        let addr = SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, 0, 0, 0);

        // First try using socket2 to set IPV6_V6ONLY = false for true dual-stack
        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
        {
            use socket2::{Domain, Protocol, Socket, Type};

            let socket = Socket::new(Domain::IPV6, Type::DGRAM, Some(Protocol::UDP))
                .context("Failed to create IPv6 UDP socket")?;

            // Disable IPV6_V6ONLY to allow IPv4 connections on IPv6 socket (dual-stack)
            // On Windows this is required; on Linux/macOS it's usually the default
            if let Err(e) = socket.set_only_v6(false) {
                warn!(
                    "Failed to disable IPV6_V6ONLY (dual-stack may not work): {}",
                    e
                );
            }

            // Set non-blocking before binding
            socket
                .set_nonblocking(true)
                .context("Failed to set socket non-blocking")?;

            // Bind to [::]:0
            socket
                .bind(&addr.into())
                .context("Failed to bind dual-stack socket")?;

            // Convert socket2::Socket to tokio::net::UdpSocket
            let std_socket: std::net::UdpSocket = socket.into();
            let tokio_socket =
                UdpSocket::from_std(std_socket).context("Failed to convert to tokio UdpSocket")?;

            let local_addr = tokio_socket.local_addr()?;
            info!("Created dual-stack UDP socket on {}", local_addr);

            Ok(tokio_socket)
        }

        // Fallback for other platforms: just bind to IPv6 and hope for the best
        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            let socket = UdpSocket::bind(addr)
                .await
                .context("Failed to bind IPv6 UDP socket")?;
            info!("Created IPv6 UDP socket on {}", socket.local_addr()?);
            Ok(socket)
        }
    }

    /// Check if a Windows network adapter with the given name pattern exists
    #[cfg(target_os = "windows")]
    pub fn windows_adapter_exists(name_pattern: &str) -> bool {
        let ps_cmd = format!(
            "Get-NetAdapter -IncludeHidden -ErrorAction SilentlyContinue | Where-Object {{ $_.Name -like '{}*' }} | Measure-Object | Select-Object -ExpandProperty Count",
            name_pattern
        );

        let output = std::process::Command::new("powershell")
            .args(["-Command", &ps_cmd])
            .output();

        if let Ok(out) = output {
            let count_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if let Ok(count) = count_str.parse::<i32>() {
                if count > 0 {
                    debug!(
                        "Found {} existing adapter(s) matching pattern '{}'",
                        count, name_pattern
                    );
                    return true;
                }
            }
        }
        false
    }

    /// Force remove a Windows network adapter by name pattern using multiple methods
    /// This is specifically designed for WinTun adapters which persist across sessions
    #[cfg(target_os = "windows")]
    pub fn windows_force_remove_adapter(name_pattern: &str) {
        info!(
            "Force removing Windows adapter(s) matching pattern: {}",
            name_pattern
        );

        // Method 1: Use pnputil to remove the device completely
        // This is the most reliable method for WinTun adapters
        let pnp_cmd = format!(
            r#"
            $devices = Get-PnpDevice -FriendlyName '*{0}*' -ErrorAction SilentlyContinue
            if ($devices) {{
                foreach ($dev in $devices) {{
                    Write-Host "Removing device: $($dev.FriendlyName) ($($dev.InstanceId))"
                    & pnputil /remove-device $dev.InstanceId /force 2>&1 | Out-Null
                }}
            }}
            # Also try by driver description
            $wintunDevices = Get-PnpDevice -Class Net -ErrorAction SilentlyContinue | Where-Object {{ $_.FriendlyName -like '*{0}*' -or $_.FriendlyName -like '*WinTun*' }}
            foreach ($dev in $wintunDevices) {{
                Write-Host "Removing WinTun device: $($dev.FriendlyName)"
                & pnputil /remove-device $dev.InstanceId /force 2>&1 | Out-Null
            }}
            "#,
            name_pattern
        );
        let output = std::process::Command::new("powershell")
            .args(["-Command", &pnp_cmd])
            .output();
        if let Ok(out) = output {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if !stdout.trim().is_empty() {
                debug!("pnputil output: {}", stdout.trim());
            }
        }

        // Method 2: Remove via devcon if available (Windows Driver Kit tool)
        let devcon_cmd = format!(
            r#"
            $devconPath = Get-Command devcon.exe -ErrorAction SilentlyContinue
            if ($devconPath) {{
                & devcon remove "*{0}*" 2>&1 | Out-Null
                & devcon remove "*WINTUN*" 2>&1 | Out-Null
            }}
            "#,
            name_pattern
        );
        let _ = std::process::Command::new("powershell")
            .args(["-Command", &devcon_cmd])
            .output();

        // Method 3: Use SetupAPI to remove device (via PowerShell with .NET)
        // This directly calls Windows Setup API which is what devcon uses
        let setupapi_cmd = r#"
            Add-Type -TypeDefinition @"
            using System;
            using System.Runtime.InteropServices;
            public class DeviceRemover {
                [DllImport("setupapi.dll", SetLastError = true, CharSet = CharSet.Auto)]
                public static extern IntPtr SetupDiGetClassDevs(ref Guid ClassGuid, IntPtr Enumerator, IntPtr hwndParent, int Flags);
                
                [DllImport("setupapi.dll", SetLastError = true)]
                public static extern bool SetupDiDestroyDeviceInfoList(IntPtr DeviceInfoSet);
            }
"@
            # Note: Full implementation would require more P/Invoke code
            # This is a placeholder for the SetupAPI approach
            Write-Host "SetupAPI cleanup attempted"
        "#;
        let _ = std::process::Command::new("powershell")
            .args(["-Command", setupapi_cmd])
            .output();

        // Method 4: Disable and then try to remove from device manager
        let disable_cmd = format!(
            "Get-NetAdapter -IncludeHidden -ErrorAction SilentlyContinue | Where-Object {{ $_.Name -like '{}*' }} | Disable-NetAdapter -Confirm:$false -ErrorAction SilentlyContinue",
            name_pattern
        );
        let _ = std::process::Command::new("powershell")
            .args(["-Command", &disable_cmd])
            .output();

        info!(
            "Completed force removal attempts for adapter pattern: {}",
            name_pattern
        );
    }

    #[cfg(not(target_os = "windows"))]
    pub fn windows_adapter_exists(_name_pattern: &str) -> bool {
        false
    }

    #[cfg(not(target_os = "windows"))]
    pub fn windows_force_remove_adapter(_name_pattern: &str) {
        // No-op on non-Windows platforms
    }
}
