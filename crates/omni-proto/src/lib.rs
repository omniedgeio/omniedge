use anyhow::{Context, Result};
use omninervous::signaling::NucleusClient;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use tokio::net::UdpSocket;

// Re-export nucleus server components for dual mode
pub use omninervous::signaling::{handle_nucleus_message, NucleusState};

// Re-export RuntimeState for shared state management (v0.3.1)
pub use omninervous::RuntimeState;

// ============================================================================
// NAT Traversal Types (OmniNervous v0.3.0)
// ============================================================================

// NAT Detection
pub use omninervous::{NatReport, NatType};

// Relay Infrastructure
pub use omninervous::{RelayConfig, RelayStats};

// Port Mapping
pub use omninervous::{PortMapCapabilities, PortMapProtocol};

// Endpoint Management
pub use omninervous::{EndpointInfo, EndpointState, PathType};

// Socket Utilities
pub use omninervous::DualStackAddr;

// Configuration
pub use omninervous::NetworkConfig as NervousNetworkConfig;

pub struct PeerInfo {
    pub vip: Ipv4Addr,
    /// IPv6 virtual IP address (dual-stack support)
    pub vip_v6: Option<Ipv6Addr>,
    pub endpoint: Option<SocketAddr>,
    pub public_key: [u8; 32],
}

pub struct PeerUpdate {
    pub peers: Vec<PeerInfo>,
}

pub struct OmniProto {
    client: NucleusClient,
    nucleus_host: String,
    /// Local IPv6 virtual IP (dual-stack support)
    vip_v6: Option<Ipv6Addr>,
}

impl OmniProto {
    /// Create a new OmniProto instance
    pub async fn new(
        nucleus_host: &str,
        cluster: String,
        secret_key: String,
        virtual_ip: Ipv4Addr,
        virtual_ip_v6: Option<Ipv6Addr>,
        listen_port: u16,
        public_key: [u8; 32],
    ) -> Result<Self> {
        // Map legacy secret_key to psk
        let psk = if secret_key.is_empty() {
            None
        } else {
            Some(secret_key)
        };

        let client = NucleusClient::new(
            nucleus_host,
            cluster,
            public_key,
            virtual_ip,
            listen_port,
            psk,
        )
        .await?;

        Ok(Self {
            client,
            nucleus_host: nucleus_host.to_string(),
            vip_v6: virtual_ip_v6,
        })
    }

    pub fn get_nucleus_host(&self) -> &str {
        &self.nucleus_host
    }

    pub async fn register(&self, socket: &UdpSocket) -> Result<()> {
        self.client.register(socket).await
    }

    pub async fn heartbeat(&self, socket: &UdpSocket, known_peer_count: u32) -> Result<()> {
        self.client.heartbeat(socket, known_peer_count).await
    }

    pub fn handle_packet(&self, buf: &[u8], secret: Option<&str>) -> Result<Option<PeerUpdate>> {
        use omninervous::signaling::{
            get_signaling_type, parse_heartbeat_ack, parse_register_ack, SIGNALING_HEARTBEAT_ACK,
            SIGNALING_REGISTER_ACK,
        };

        let msg_type = get_signaling_type(buf).context("Empty signaling packet")?;

        let mut peers = Vec::new();

        match msg_type {
            SIGNALING_REGISTER_ACK => {
                let ack = parse_register_ack(buf, secret)?;
                for p in ack.recent_peers {
                    peers.push(PeerInfo {
                        vip: p.vip,
                        // TODO: Parse vip_v6 from signaling when omninervous supports it
                        vip_v6: None,
                        endpoint: p.endpoint.parse().ok(),
                        public_key: p.public_key,
                    });
                }
            }
            SIGNALING_HEARTBEAT_ACK => {
                let ack = parse_heartbeat_ack(buf, secret)?;
                for p in ack.new_peers {
                    peers.push(PeerInfo {
                        vip: p.vip,
                        // TODO: Parse vip_v6 from signaling when omninervous supports it
                        vip_v6: None,
                        endpoint: p.endpoint.parse().ok(),
                        public_key: p.public_key,
                    });
                }
            }
            _ => return Ok(None),
        }

        Ok(Some(PeerUpdate { peers }))
    }

    pub fn cluster(&self) -> &str {
        self.client.cluster()
    }

    pub fn vip(&self) -> Ipv4Addr {
        self.client.vip()
    }

    /// Get the IPv6 virtual IP address (if dual-stack is enabled)
    pub fn vip_v6(&self) -> Option<Ipv6Addr> {
        self.vip_v6
    }

    // ========================================================================
    // NAT Traversal Status (v0.3.0 Integration)
    // ========================================================================

    /// Get the detected NAT type for this client
    /// Returns None if NAT detection hasn't completed yet
    pub fn get_nat_type(&self) -> Option<NatType> {
        self.client.nat_type()
    }

    /// Check if NAT type detection is complete
    pub fn is_nat_detected(&self) -> bool {
        self.client.nat_type().is_some()
    }

    /// Get human-readable NAT type description
    pub fn get_nat_description(&self) -> String {
        match self.get_nat_type() {
            Some(NatType::Unknown) => "Unknown (detection in progress)".to_string(),
            Some(NatType::Open) => "Open (no NAT, direct connectivity)".to_string(),
            Some(NatType::FullCone) => "Full Cone (easiest NAT type for P2P)".to_string(),
            Some(NatType::RestrictedCone) => "Restricted Cone (moderate NAT)".to_string(),
            Some(NatType::PortRestrictedCone) => "Port-Restricted Cone (strict NAT)".to_string(),
            Some(NatType::Symmetric) => "Symmetric (hardest NAT, relay required)".to_string(),
            None => "Not yet detected".to_string(),
        }
    }

    // ========================================================================
    // Runtime State Queries (v0.3.1 Integration)
    // ========================================================================

    /// Get relay statistics from the runtime state
    ///
    /// Returns current relay session count, active sessions, and bytes relayed.
    /// Returns None if relay is not active or stats haven't been updated.
    pub async fn get_relay_stats(&self) -> Option<RelayStats> {
        self.client.relay_stats().await
    }

    /// Get port mapping status from the runtime state
    ///
    /// Returns detected port mapping capabilities (NAT-PMP, UPnP, PCP support).
    /// Returns None if port mapping is not active or status hasn't been updated.
    pub async fn get_portmap_status(&self) -> Option<PortMapCapabilities> {
        self.client.portmap_status().await
    }

    /// Check if relay is currently being used for any peer connection
    pub async fn is_using_relay(&self) -> bool {
        self.client.is_using_relay().await
    }

    /// Check if relay functionality is enabled in configuration
    pub async fn is_relay_enabled(&self) -> bool {
        self.client.is_relay_enabled().await
    }

    /// Check if port mapping is enabled in configuration
    pub async fn is_portmap_enabled(&self) -> bool {
        self.client.is_portmap_enabled().await
    }

    // ========================================================================
    // Runtime State Updates (called by daemon/manager)
    // ========================================================================

    /// Update relay statistics (called by the connection manager)
    pub async fn update_relay_stats(&self, stats: Option<RelayStats>) {
        self.client.update_relay_stats(stats).await;
    }

    /// Update port mapping status (called by the connection manager)
    pub async fn update_portmap_status(&self, status: Option<PortMapCapabilities>) {
        self.client.update_portmap_status(status).await;
    }

    /// Update whether relay is being used (called by the connection manager)
    pub async fn update_using_relay(&self, using: bool) {
        self.client.update_using_relay(using).await;
    }

    /// Update whether relay is enabled (called on config change)
    pub async fn update_relay_enabled(&self, enabled: bool) {
        self.client.update_relay_enabled(enabled).await;
    }

    /// Update whether port mapping is enabled (called on config change)
    pub async fn update_portmap_enabled(&self, enabled: bool) {
        self.client.update_portmap_enabled(enabled).await;
    }

    // ========================================================================
    // IPv6 Configuration (v2.1.0+ Dual-Stack Support)
    // ========================================================================

    /// Check if IPv6 is enabled for this protocol instance
    /// Returns true if a VIPv6 was provided during construction
    pub fn is_ipv6_enabled(&self) -> bool {
        self.vip_v6.is_some()
    }

    /// Get IPv6 configuration summary for display
    pub fn get_ipv6_summary(&self) -> String {
        match self.vip_v6 {
            Some(v6) => format!("IPv6 Enabled ({})", v6),
            None => "IPv6 Disabled".to_string(),
        }
    }

    // NOTE: The following methods are placeholders for future OmniNervous integration.
    // Once OmniNervous supports IPv6 configuration, these will call the underlying client.

    /// Set IPv6 enabled state (placeholder for future OmniNervous integration)
    /// Currently a no-op as OmniNervous doesn't yet support runtime IPv6 config
    #[allow(unused_variables)]
    pub fn set_ipv6_enabled(&self, enabled: bool) {
        // TODO (v0.3.1): Call self.client.set_ipv6_enabled(enabled) when available
        // For now, IPv6 is configured at construction time via vip_v6 parameter
    }

    /// Set IPv6 preference settings (placeholder for future OmniNervous integration)
    /// Currently a no-op as OmniNervous doesn't yet support runtime IPv6 preference config
    #[allow(unused_variables)]
    pub fn set_ipv6_preference(&self, prefer_ipv6: bool, threshold_ms: u32) {
        // TODO (v0.3.1): Call self.client.set_ipv6_preference(prefer_ipv6, threshold_ms) when available
        // These settings will control Happy Eyeballs behavior when available
    }
}
