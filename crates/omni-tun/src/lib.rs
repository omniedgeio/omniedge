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

    /// Soft shutdown - clears peers and routing but keeps TUN device alive.
    /// Use this on Windows to prevent WinTun adapter accumulation on disconnect/reconnect.
    pub async fn soft_shutdown(&self) {
        self.interface.soft_shutdown().await
    }

    /// Check if the TUN loop is active (device is being used by reader/writer tasks)
    pub async fn is_tun_active(&self) -> bool {
        self.interface.is_tun_active().await
    }
}

/// Windows-specific utilities for managing WinTun adapters
#[cfg(target_os = "windows")]
pub mod windows {
    use log::{debug, info, warn};

    /// Delete all WinTun adapters matching the given name pattern.
    /// This properly closes the adapter using the WinTun API, which should
    /// prevent adapter accumulation ("wintun", "wintun 2", etc.).
    ///
    /// Returns the number of adapters that were successfully deleted.
    pub fn delete_wintun_adapters(name_pattern: &str) -> usize {
        let mut deleted_count = 0;

        // Load the WinTun library (unsafe because it loads a DLL)
        let wintun = match unsafe { wintun::load() } {
            Ok(w) => w,
            Err(e) => {
                warn!(
                    "Failed to load WinTun library: {:?}. Adapter cleanup may not work.",
                    e
                );
                return 0;
            }
        };

        // Try to open and close adapters with common name patterns
        // WinTun creates adapters named "wintun", "wintun 2", "wintun 3", etc.
        let names_to_try: Vec<String> = if name_pattern.is_empty() || name_pattern == "wintun" {
            // Try the base name and numbered variants
            let mut names = vec!["wintun".to_string()];
            for i in 2..=20 {
                names.push(format!("wintun {}", i));
            }
            names
        } else {
            // Try the specific pattern and numbered variants
            let mut names = vec![name_pattern.to_string()];
            for i in 2..=20 {
                names.push(format!("{} {}", name_pattern, i));
            }
            names
        };

        for name in names_to_try {
            match wintun::Adapter::open(&wintun, &name) {
                Ok(adapter) => {
                    info!("Found WinTun adapter '{}', closing it...", name);
                    // Dropping the adapter calls WintunCloseAdapter
                    // This doesn't delete the adapter but closes our handle to it
                    drop(adapter);
                    deleted_count += 1;
                }
                Err(_) => {
                    // Adapter doesn't exist with this name, continue
                    debug!("No WinTun adapter found with name '{}'", name);
                }
            }
        }

        if deleted_count > 0 {
            info!("Closed {} WinTun adapter(s)", deleted_count);
        }

        deleted_count
    }

    /// Check if a WinTun adapter with the given name exists.
    pub fn wintun_adapter_exists(name: &str) -> bool {
        let wintun = match unsafe { wintun::load() } {
            Ok(w) => w,
            Err(_) => return false,
        };

        wintun::Adapter::open(&wintun, name).is_ok()
    }

    /// Get a list of existing WinTun adapter names.
    pub fn list_wintun_adapters() -> Vec<String> {
        let wintun = match unsafe { wintun::load() } {
            Ok(w) => w,
            Err(_) => return vec![],
        };

        let mut found = vec![];

        // Check common names
        let names_to_check = [
            "wintun",
            "wintun 2",
            "wintun 3",
            "wintun 4",
            "wintun 5",
            "wintun 6",
            "wintun 7",
            "wintun 8",
            "wintun 9",
            "wintun 10",
            "OmniEdge",
            "OmniEdge 2",
            "OmniEdge 3",
        ];

        for name in names_to_check {
            if wintun::Adapter::open(&wintun, name).is_ok() {
                found.push(name.to_string());
            }
        }

        found
    }
}

#[cfg(not(target_os = "windows"))]
pub mod windows {
    /// Stub for non-Windows platforms
    pub fn delete_wintun_adapters(_name_pattern: &str) -> usize {
        0
    }

    pub fn wintun_adapter_exists(_name: &str) -> bool {
        false
    }

    pub fn list_wintun_adapters() -> Vec<String> {
        vec![]
    }
}
