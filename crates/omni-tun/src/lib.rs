use anyhow::Result;
use omninervous::wg::{UserspaceWgControl, WgInterface};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;

#[derive(Clone)]
pub struct OmniTun {
    interface: WgInterface,
}

impl OmniTun {
    pub fn new_userspace(ifname: &str) -> Self {
        Self {
            interface: WgInterface::Userspace(UserspaceWgControl::new(ifname)),
        }
    }

    pub async fn setup(&mut self, vip: &str, port: u16, private_key: &str) -> anyhow::Result<()> {
        let res: Result<(), String> = self.interface.setup_interface(vip, port, private_key).await;
        res.map_err(|e| anyhow::anyhow!("TUN Setup failed: {}", e))
    }

    pub async fn add_peer(
        &mut self,
        public_key: &str,
        endpoint: Option<SocketAddr>,
        allowed_ips: &[String],
    ) -> anyhow::Result<()> {
        let res: Result<(), String> = self
            .interface
            .set_peer(public_key, endpoint, allowed_ips, Some(25))
            .await;
        res.map_err(|e| anyhow::anyhow!("Set peer failed: {}", e))
    }

    pub async fn start_loop(&mut self, socket: Arc<UdpSocket>) -> anyhow::Result<()> {
        let res: Result<(), String> = self.interface.start_loop(socket).await;
        res.map_err(|e| anyhow::anyhow!("Packet loop failed: {}", e))
    }

    pub async fn handle_packet(
        &mut self,
        buf: &[u8],
        src: SocketAddr,
        socket: &UdpSocket,
    ) -> anyhow::Result<()> {
        let res: Result<(), String> = self
            .interface
            .handle_incoming_packet(buf, src, socket)
            .await;
        res.map_err(|e| anyhow::anyhow!("WireGuard packet handling failed: {}", e))
    }

    pub async fn get_peer_stats(&self, public_key: &str) -> Option<omninervous::wg::PeerStats> {
        self.interface.get_peer_stats(public_key).await
    }

    /// Shutdown the TUN interface and release resources.
    /// This must be called before dropping OmniTun to properly release the TUN device
    /// on macOS (where utun interfaces are tied to the file descriptor).
    pub async fn shutdown(&self) {
        self.interface.shutdown().await
    }
}
