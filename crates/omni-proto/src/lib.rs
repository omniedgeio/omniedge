use anyhow::{Context, Result};
use omninervous::signaling::NucleusClient;
use std::net::{Ipv4Addr, SocketAddr};
use tokio::net::UdpSocket;

// Re-export nucleus server components for dual mode
pub use omninervous::signaling::{handle_nucleus_message, NucleusState};

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
    pub endpoint: Option<SocketAddr>,
    pub public_key: [u8; 32],
}

pub struct PeerUpdate {
    pub peers: Vec<PeerInfo>,
}

pub struct OmniProto {
    client: NucleusClient,
    nucleus_host: String,
}

impl OmniProto {
    /// Create a new OmniProto instance
    pub async fn new(
        nucleus_host: &str,
        cluster: String,
        secret_key: String,
        virtual_ip: Ipv4Addr,
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

    // Future methods (stubs for now, will be implemented when OmniNervous exposes state)

    /// Get relay statistics (stub - returns None until OmniNervous exposes this)
    pub fn get_relay_stats(&self) -> Option<RelayStats> {
        // TODO: Once OmniNervous adds NucleusClient::relay_stats() method
        None
    }

    /// Get port mapping status (stub - returns None until OmniNervous exposes this)
    pub fn get_portmap_status(&self) -> Option<PortMapCapabilities> {
        // TODO: Once OmniNervous adds NucleusClient::portmap_status() method
        None
    }

    /// Check if currently using relay for any peer (stub)
    pub fn is_using_relay(&self) -> bool {
        // TODO: Once OmniNervous exposes relay state
        false
    }
}
