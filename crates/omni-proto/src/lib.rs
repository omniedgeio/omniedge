use anyhow::{Context, Result};
use omninervous::signaling::NucleusClient;
use std::net::{Ipv4Addr, SocketAddr};
use tokio::net::UdpSocket;

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
    supernode_host: String,
}

impl OmniProto {
    /// Create a new OmniProto instance using legacy OmniEdge naming
    pub async fn new(
        supernode_host: &str,
        community_name: String,
        secret_key: String,
        virtual_ip: Ipv4Addr,
        listen_port: u16,
        public_key: [u8; 32],
    ) -> Result<Self> {
        // Map legacy n2n names to OmniNervous Nucleus names
        let cluster = community_name;
        let psk = if secret_key.is_empty() {
            None
        } else {
            Some(secret_key)
        };

        let client = NucleusClient::new(
            supernode_host,
            cluster,
            public_key,
            virtual_ip,
            listen_port,
            psk,
        )
        .await?;

        Ok(Self {
            client,
            supernode_host: supernode_host.to_string(),
        })
    }

    pub fn get_server_host(&self) -> &str {
        &self.supernode_host
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
}
