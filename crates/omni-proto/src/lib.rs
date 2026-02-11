use anyhow::{Context, Result};
use log::{debug, info, warn};
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
pub use omninervous::{RelayClient, RelayClientState, RelayConfig, RelayStats, SessionId};

// Relay Protocol Message Types and Encoding
pub use omninervous::relay::{
    encode_relay_bind, encode_relay_bind_ack, encode_relay_data, encode_relay_unbind,
    is_relay_message, parse_relay_bind, parse_relay_bind_ack, parse_relay_data, parse_relay_unbind,
    RelayBindAck, RelayBindRequest, MSG_RELAY_BIND, MSG_RELAY_BIND_ACK, MSG_RELAY_DATA,
    MSG_RELAY_KEEPALIVE, MSG_RELAY_UNBIND,
};

// Port Mapping
pub use omninervous::{PortMapCapabilities, PortMapProtocol, PortMapper, PortMapping};

// Endpoint Management
pub use omninervous::{EndpointInfo, EndpointSource, EndpointState, PathType};

// Socket Utilities
pub use omninervous::DualStackAddr;

// Configuration
pub use omninervous::NetworkConfig as NervousNetworkConfig;

// ============================================================================
// P2P Connection State Tracking (v0.3.2 - NAT Traversal Fix)
// ============================================================================

// Connection state management for disco protocol
pub use omninervous::{ConnectionState as PeerConnectionState, EndpointSet, PeerConnection};

// Disco ping/pong protocol
pub use omninervous::signaling::{
    encode_disco_ping, encode_disco_pong, parse_disco_ping, parse_disco_pong, DiscoPing, DiscoPong,
    SIGNALING_DISCO_PING, SIGNALING_DISCO_PONG,
};

// Disco configuration and results
pub use omninervous::{DiscoConfig, DiscoResult, PendingPing};

pub struct PeerInfo {
    pub vip: Ipv4Addr,
    /// IPv6 virtual IP address (dual-stack support)
    pub vip_v6: Option<Ipv6Addr>,
    pub endpoint: Option<SocketAddr>,
    pub public_key: [u8; 32],
    /// NAT type of this peer (for connection strategy selection)
    pub nat_type: Option<NatType>,
    /// Port-mapped endpoint (from NAT-PMP/UPnP/PCP, may be more reachable)
    pub mapped_endpoint: Option<SocketAddr>,
}

pub struct PeerUpdate {
    pub peers: Vec<PeerInfo>,
    pub removed_vips: Vec<Ipv4Addr>,
}

pub struct OmniProto {
    client: tokio::sync::RwLock<NucleusClient>,
    nucleus_host: String,
    /// Local IPv6 virtual IP (dual-stack support)
    vip_v6: Option<Ipv6Addr>,
    /// Encryption context for signaling (v0.3.5)
    encryption: tokio::sync::RwLock<omninervous::signaling::SignalingEncryption>,
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
        private_key: [u8; 32],
    ) -> Result<Self> {
        // Map legacy secret_key to psk
        let psk = if secret_key.is_empty() {
            None
        } else {
            Some(secret_key.clone())
        };

        // Use with_ipv6() constructor to properly pass IPv6 VIP to signaling
        let client = NucleusClient::with_ipv6(
            nucleus_host,
            cluster,
            public_key,
            virtual_ip,
            virtual_ip_v6,
            listen_port,
            psk,
        )
        .await?;

        // Initialize signaling encryption using our identity private key
        let encryption = if !secret_key.is_empty() {
            // Enable signaling encryption if secret_key (cluster PSK) is present
            // We use our individual identity private key for the NaCl box
            omninervous::signaling::SignalingEncryption::from_secret_key(private_key, true)
        } else {
            omninervous::signaling::SignalingEncryption::new(false)
        };

        Ok(Self {
            client: tokio::sync::RwLock::new(client),
            nucleus_host: nucleus_host.to_string(),
            vip_v6: virtual_ip_v6,
            encryption: tokio::sync::RwLock::new(encryption),
        })
    }

    pub fn get_nucleus_host(&self) -> &str {
        &self.nucleus_host
    }

    pub async fn register(&self, socket: &UdpSocket) -> Result<()> {
        let client = self.client.read().await;
        let ext_port = client.external_port();
        let ext_addr = client.external_addr();
        info!(
            "Sending REGISTER to Nucleus: vip={}, listen_port={}, external_port={:?}, external_addr={:?}",
            client.vip(),
            socket.local_addr().map(|a| a.port()).unwrap_or(0),
            ext_port,
            ext_addr
        );
        client.register(socket).await
    }

    pub async fn heartbeat(&self, socket: &UdpSocket, known_peer_count: u32) -> Result<()> {
        debug!("Sending HEARTBEAT to Nucleus: known_peer_count={}", known_peer_count);
        self.client.read().await.heartbeat(socket, known_peer_count).await
    }

    pub async fn handle_packet(&self, buf: &[u8], secret: Option<&str>) -> Result<Option<PeerUpdate>> {
        use omninervous::signaling::{
            get_signaling_type, parse_heartbeat_ack, parse_register_ack, parse_peer_info,
            parse_peer_notify,
            SIGNALING_HEARTBEAT_ACK, SIGNALING_REGISTER_ACK, MSG_ENCRYPTED, SIGNALING_PEER_INFO,
            SIGNALING_PEER_NOTIFY,
        };

        let mut decrypted_buf = None;
        let mut msg_type = get_signaling_type(buf).context("Empty signaling packet")?;

        debug!(
            "Signaling packet received: len={}, type=0x{:02x} (PEER_NOTIFY=0x{:02x}, HEARTBEAT_ACK=0x{:02x})",
            buf.len(), msg_type, SIGNALING_PEER_NOTIFY, SIGNALING_HEARTBEAT_ACK
        );

        // Handle encrypted signaling packets
        if msg_type == MSG_ENCRYPTED {
            let mut enc = self.encryption.write().await;
            match enc.decrypt(buf) {
                Ok(plaintext) => {
                    debug!("Signaling decryption successful, payload len={}", plaintext.len());
                    decrypted_buf = Some(plaintext);
                    if let Some(p) = decrypted_buf.as_ref() {
                        msg_type = get_signaling_type(p).context("Empty signaling packet after decryption")?;
                        debug!("Decrypted signaling type: 0x{:02x}", msg_type);
                    }
                }
                Err(e) => {
                    anyhow::bail!("Failed to decrypt signaling message: {}", e);
                }
            }
        }

        let effective_buf = decrypted_buf.as_deref().unwrap_or(buf);

        let mut peers = Vec::new();
        let mut removed_vips = Vec::new();

        match msg_type {
            SIGNALING_REGISTER_ACK => {
                let ack = parse_register_ack(effective_buf, secret)?;
                info!(
                    "Received REGISTER_ACK from Nucleus: success={}, {} recent peers",
                    ack.success, ack.recent_peers.len()
                );
                for p in ack.recent_peers {
                    info!(
                        "  Peer from REGISTER_ACK: vip={}, endpoint={}, mapped_endpoint={:?}, nat_type={:?}",
                        p.vip, p.endpoint, p.mapped_endpoint, p.nat_type
                    );
                    peers.push(PeerInfo {
                        vip: p.vip,
                        vip_v6: p.vip_v6,
                        endpoint: p.endpoint.parse().ok(),
                        public_key: p.public_key,
                        nat_type: p.nat_type,
                        mapped_endpoint: p.mapped_endpoint.as_ref().and_then(|s| s.parse().ok()),
                    });
                }
            }
            SIGNALING_HEARTBEAT_ACK => {
                let ack = parse_heartbeat_ack(effective_buf, secret)?;
                if !ack.new_peers.is_empty() || !ack.removed_vips.is_empty() {
                    info!(
                        "Received HEARTBEAT_ACK: {} new peers, {} removed",
                        ack.new_peers.len(), ack.removed_vips.len()
                    );
                }
                for p in ack.new_peers {
                    info!(
                        "  New peer from HEARTBEAT_ACK: vip={}, endpoint={}, mapped_endpoint={:?}, nat_type={:?}",
                        p.vip, p.endpoint, p.mapped_endpoint, p.nat_type
                    );
                    peers.push(PeerInfo {
                        vip: p.vip,
                        vip_v6: p.vip_v6,
                        endpoint: p.endpoint.parse().ok(),
                        public_key: p.public_key,
                        nat_type: p.nat_type,
                        mapped_endpoint: p.mapped_endpoint.as_ref().and_then(|s| s.parse().ok()),
                    });
                }
                for vip in &ack.removed_vips {
                    info!("  Removed peer from HEARTBEAT_ACK: vip={}", vip);
                }
                removed_vips.extend(ack.removed_vips.iter().copied());
            }
            SIGNALING_PEER_INFO => {
                let info = parse_peer_info(effective_buf, secret)?;
                if info.found {
                    if let Some(p) = info.peer {
                        info!(
                            "Received PEER_INFO from Nucleus: vip={}, endpoint={}, mapped_endpoint={:?}, nat_type={:?}",
                            p.vip, p.endpoint, p.mapped_endpoint, p.nat_type
                        );
                        peers.push(PeerInfo {
                            vip: p.vip,
                            vip_v6: p.vip_v6,
                            endpoint: p.endpoint.parse().ok(),
                            public_key: p.public_key,
                            nat_type: p.nat_type,
                            mapped_endpoint: p.mapped_endpoint.as_ref().and_then(|s| s.parse().ok()),
                        });
                    }
                }
            }
            SIGNALING_PEER_NOTIFY => {
                match parse_peer_notify(effective_buf, secret) {
                    Ok(notify) => {
                        info!(
                            "Received PEER_NOTIFY from Nucleus: new peer vip={}, endpoint={}, mapped_endpoint={:?}, nat_type={:?}",
                            notify.peer.vip, notify.peer.endpoint, notify.peer.mapped_endpoint, notify.peer.nat_type
                        );
                        peers.push(PeerInfo {
                            vip: notify.peer.vip,
                            vip_v6: notify.peer.vip_v6,
                            endpoint: notify.peer.endpoint.parse().ok(),
                            public_key: notify.peer.public_key,
                            nat_type: notify.peer.nat_type,
                            mapped_endpoint: notify.peer.mapped_endpoint.as_ref().and_then(|s| s.parse().ok()),
                        });
                    }
                    Err(e) => {
                        warn!("Failed to parse PEER_NOTIFY: {}", e);
                        return Ok(None);
                    }
                }
            }
            _ => return Ok(None),
        }

        Ok(Some(PeerUpdate { peers, removed_vips }))
    }

    pub async fn cluster(&self) -> String {
        self.client.read().await.cluster().to_string()
    }

    pub async fn vip(&self) -> Ipv4Addr {
        self.client.read().await.vip()
    }

    /// Get the IPv6 virtual IP address (if dual-stack is enabled)
    pub async fn vip_v6(&self) -> Option<Ipv6Addr> {
        // Prefer the client's vip_v6 as it may have been updated at runtime
        self.client.read().await.vip_v6().or(self.vip_v6)
    }

    // ========================================================================
    // NAT Traversal Status (v0.3.0 Integration)
    // ========================================================================

    /// Get the detected NAT type for this client
    /// Returns None if NAT detection hasn't completed yet
    pub async fn get_nat_type(&self) -> Option<NatType> {
        self.client.read().await.nat_type()
    }

    /// Check if NAT type detection is complete
    pub async fn is_nat_detected(&self) -> bool {
        self.client.read().await.nat_type().is_some()
    }

    /// Get human-readable NAT type description
    pub async fn get_nat_description(&self) -> String {
        match self.get_nat_type().await {
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
        self.client.read().await.relay_stats().await
    }

    /// Get port mapping status from the runtime state
    ///
    /// Returns detected port mapping capabilities (NAT-PMP, UPnP, PCP support).
    /// Returns None if port mapping is not active or status hasn't been updated.
    pub async fn get_portmap_status(&self) -> Option<PortMapCapabilities> {
        self.client.read().await.portmap_status().await
    }

    /// Check if relay is currently being used for any peer connection
    pub async fn is_using_relay(&self) -> bool {
        self.client.read().await.is_using_relay().await
    }

    /// Check if relay functionality is enabled in configuration
    pub async fn is_relay_enabled(&self) -> bool {
        self.client.read().await.is_relay_enabled().await
    }

    /// Check if port mapping is enabled in configuration
    pub async fn is_portmap_enabled(&self) -> bool {
        self.client.read().await.is_portmap_enabled().await
    }

    // ========================================================================
    // Runtime State Updates (called by daemon/manager)
    // ========================================================================

    /// Update relay statistics (called by the connection manager)
    pub async fn update_relay_stats(&self, stats: Option<RelayStats>) {
        self.client.read().await.update_relay_stats(stats).await;
    }

    /// Update port mapping status (called by the connection manager)
    pub async fn update_portmap_status(&self, status: Option<PortMapCapabilities>) {
        self.client.read().await.update_portmap_status(status).await;
    }

    /// Update whether relay is being used (called by the connection manager)
    pub async fn update_using_relay(&self, using: bool) {
        self.client.read().await.update_using_relay(using).await;
    }

    /// Update whether relay is enabled (called on config change)
    pub async fn update_relay_enabled(&self, enabled: bool) {
        self.client.read().await.update_relay_enabled(enabled).await;
    }

    /// Update whether port mapping is enabled (called on config change)
    pub async fn update_portmap_enabled(&self, enabled: bool) {
        self.client.read().await.update_portmap_enabled(enabled).await;
    }

    // ========================================================================
    // Port Mapping / External Endpoint (NAT Traversal Fix v0.3.5)
    // ========================================================================

    /// Set the external port from port mapping (NAT-PMP/UPnP/PCP)
    /// 
    /// This port will be advertised to the Nucleus server in subsequent
    /// REGISTER/HEARTBEAT messages, allowing peers to reach us through NAT.
    pub async fn set_external_port(&self, port: u16) {
        self.client.write().await.set_external_port(port);
        debug!("Set external port to {}", port);
    }

    /// Set the external address from port mapping
    /// 
    /// Format should be "ip:port" (e.g., "203.0.113.1:51820")
    pub async fn set_external_addr(&self, addr: String) {
        self.client.write().await.set_external_addr(addr.clone());
        debug!("Set external address to {}", addr);
    }

    /// Get the currently configured external port (if any)
    pub async fn get_external_port(&self) -> Option<u16> {
        self.client.read().await.external_port()
    }

    /// Get the currently configured external address (if any)
    pub async fn get_external_addr(&self) -> Option<String> {
        self.client.read().await.external_addr().map(|s| s.to_string())
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

    // NOTE: The following methods are for runtime IPv6 configuration.
    // OmniNervous supports IPv6 through NucleusClient::with_ipv6() at construction time.
    // These methods provide a way to track IPv6 state changes but don't affect the
    // underlying signaling since NucleusClient's vip_v6 is set at construction.

    /// Check/log IPv6 enabled state
    /// Note: The actual IPv6 VIP is set at construction time via with_ipv6().
    /// This method is for informational/logging purposes.
    pub fn set_ipv6_enabled(&self, enabled: bool) {
        if enabled && self.vip_v6.is_some() {
            debug!("IPv6 is enabled with VIP {:?}", self.vip_v6);
        } else if enabled {
            debug!("IPv6 enabled flag set but no VIP v6 configured - set via constructor");
        } else {
            debug!("IPv6 disabled - local vip_v6 will be ignored");
        }
    }

    /// Set IPv6 preference settings
    /// Note: OmniNervous supports prefer_ipv6 and happy_eyeballs_delay_ms in NetworkConfig,
    /// which are loaded from config file at startup.
    pub fn set_ipv6_preference(&self, prefer_ipv6: bool, threshold_ms: u32) {
        debug!(
            "IPv6 preference: prefer={}, threshold={}ms (configured via daemon config file)",
            prefer_ipv6, threshold_ms
        );
    }
}
