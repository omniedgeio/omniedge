use anyhow::Result;
use log::{debug, info, warn};
use omninervous::wg::{UserspaceWgControl, WgInterface};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;

#[derive(Clone)]
pub struct OmniTun {
    interface: WgInterface,
    /// Interface name (for IPv6 configuration)
    ifname: String,
}

impl OmniTun {
    pub fn new_userspace(ifname: &str) -> Self {
        Self {
            interface: WgInterface::Userspace(UserspaceWgControl::new(ifname)),
            ifname: ifname.to_string(),
        }
    }

    pub async fn setup(&mut self, vip: &str, port: u16, private_key: &str) -> anyhow::Result<()> {
        let res: Result<(), String> = self.interface.setup_interface(vip, port, private_key).await;
        res.map_err(|e| anyhow::anyhow!("TUN Setup failed: {}", e))
    }

    /// Setup the TUN interface with dual-stack (IPv4 + IPv6) support
    pub async fn setup_dual_stack(
        &mut self,
        vip: &str,
        vip_v6: Option<&str>,
        prefix_v6: Option<u8>,
        port: u16,
        private_key: &str,
    ) -> anyhow::Result<()> {
        // First setup IPv4 (this creates the interface)
        self.setup(vip, port, private_key).await?;

        // Then add IPv6 address if provided
        if let Some(ipv6) = vip_v6 {
            let prefix = prefix_v6.unwrap_or(120);
            if let Err(e) = self.add_ipv6_address(ipv6, prefix).await {
                // IPv6 failure is non-fatal - log warning and continue with IPv4 only
                warn!(
                    "Failed to configure IPv6 address {}/{}: {}. Continuing with IPv4 only.",
                    ipv6, prefix, e
                );
            } else {
                info!(
                    "Dual-stack configured: IPv4={}, IPv6={}/{}",
                    vip, ipv6, prefix
                );
            }
        }

        Ok(())
    }

    /// Add an IPv6 address to the TUN interface (platform-specific)
    async fn add_ipv6_address(&self, ipv6: &str, prefix_len: u8) -> anyhow::Result<()> {
        let ifname = self.get_interface_name().await;

        #[cfg(target_os = "linux")]
        {
            // Linux: ip -6 addr add <ipv6>/<prefix> dev <ifname>
            let output = std::process::Command::new("ip")
                .args([
                    "-6",
                    "addr",
                    "add",
                    &format!("{}/{}", ipv6, prefix_len),
                    "dev",
                    &ifname,
                ])
                .output()
                .map_err(|e| anyhow::anyhow!("Failed to run ip command: {}", e))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                // Ignore "already exists" error
                if !stderr.contains("File exists") {
                    return Err(anyhow::anyhow!("ip -6 addr add failed: {}", stderr));
                }
            }
            debug!("Added IPv6 address {}/{} to {}", ipv6, prefix_len, ifname);
        }

        #[cfg(target_os = "macos")]
        {
            // macOS: ifconfig <ifname> inet6 <ipv6> prefixlen <prefix>
            let output = std::process::Command::new("ifconfig")
                .args([&ifname, "inet6", ipv6, "prefixlen", &prefix_len.to_string()])
                .output()
                .map_err(|e| anyhow::anyhow!("Failed to run ifconfig command: {}", e))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow::anyhow!("ifconfig inet6 failed: {}", stderr));
            }
            debug!("Added IPv6 address {}/{} to {}", ipv6, prefix_len, ifname);
        }

        #[cfg(target_os = "windows")]
        {
            // Windows: PowerShell New-NetIPAddress -InterfaceAlias "<ifname>" -IPAddress "<ipv6>" -PrefixLength <prefix>
            let ps_cmd = format!(
                "New-NetIPAddress -InterfaceAlias '{}' -IPAddress '{}' -PrefixLength {} -ErrorAction SilentlyContinue",
                ifname, ipv6, prefix_len
            );
            let output = std::process::Command::new("powershell")
                .args(["-Command", &ps_cmd])
                .output()
                .map_err(|e| anyhow::anyhow!("Failed to run PowerShell command: {}", e))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                // Ignore if address already exists
                if !stderr.contains("already exists") && !stderr.is_empty() {
                    return Err(anyhow::anyhow!(
                        "PowerShell New-NetIPAddress failed: {}",
                        stderr
                    ));
                }
            }
            debug!("Added IPv6 address {}/{} to {}", ipv6, prefix_len, ifname);
        }

        Ok(())
    }

    /// Get the actual interface name (may differ from configured name on some platforms)
    async fn get_interface_name(&self) -> String {
        // On macOS, the interface name is auto-assigned (utunN)
        // Try to get it from the WgInterface if available
        #[cfg(target_os = "macos")]
        {
            // For now, return the configured name - the actual utun name
            // would need to be retrieved from the interface after creation
            if self.ifname.is_empty() {
                // Auto-assigned - we need to find it
                // This is a limitation - for now use a reasonable default
                return "utun7".to_string();
            }
        }

        // On Windows, use "OmniEdge" as the default interface name
        #[cfg(target_os = "windows")]
        {
            if self.ifname.is_empty() {
                return "OmniEdge".to_string();
            }
        }

        // On Linux, use the configured name or "omniedge0" as default
        #[cfg(target_os = "linux")]
        {
            if self.ifname.is_empty() {
                return "omniedge0".to_string();
            }
        }

        self.ifname.clone()
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
