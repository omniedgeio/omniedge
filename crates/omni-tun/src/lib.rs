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
    /// Cached actual interface name (detected after setup on macOS)
    #[allow(dead_code)]
    actual_ifname: Option<String>,
    /// Virtual IP address (used to detect the actual utun interface on macOS)
    vip: Option<String>,
}

impl OmniTun {
    pub fn new_userspace(ifname: &str) -> Self {
        Self {
            interface: WgInterface::Userspace(UserspaceWgControl::new(ifname)),
            ifname: ifname.to_string(),
            actual_ifname: None,
            vip: None,
        }
    }

    pub async fn setup(&mut self, vip: &str, port: u16, private_key: &str) -> anyhow::Result<()> {
        // Store the VIP for later interface detection (needed on macOS)
        self.vip = Some(vip.to_string());
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
        // We need to detect it by matching the VIP address on network interfaces
        #[cfg(target_os = "macos")]
        {
            if self.ifname.is_empty() {
                // Auto-assigned - detect by VIP
                if let Some(ref vip) = self.vip {
                    if let Some(name) = self.find_interface_by_ip(vip) {
                        debug!("Detected macOS utun interface '{}' for VIP {}", name, vip);
                        return name;
                    }
                }
                // Fallback: scan for any utun with a 100.x.x.x address (OmniEdge VIP range)
                if let Some(name) = self.find_utun_with_omniedge_ip() {
                    warn!(
                        "Using fallback utun detection: found '{}' with OmniEdge IP range",
                        name
                    );
                    return name;
                }
                // Last resort fallback
                warn!("Could not detect utun interface name, using fallback 'utun0'");
                return "utun0".to_string();
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

    /// Find a network interface by its IPv4 address
    #[cfg(target_os = "macos")]
    fn find_interface_by_ip(&self, target_ip: &str) -> Option<String> {
        use std::process::Command;

        // Use ifconfig to find the interface with this IP
        // Output format: "utunN: flags=... \n inet <ip> ..."
        let output = Command::new("sh")
            .args([
                "-c",
                &format!(
                    "ifconfig | grep -B5 'inet {}' | grep -E '^utun[0-9]+:' | head -1 | cut -d: -f1",
                    target_ip
                ),
            ])
            .output()
            .ok()?;

        if output.status.success() {
            let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !name.is_empty() && name.starts_with("utun") {
                return Some(name);
            }
        }

        None
    }

    /// Find any utun interface with an OmniEdge IP (100.x.x.x range)
    #[cfg(target_os = "macos")]
    fn find_utun_with_omniedge_ip(&self) -> Option<String> {
        use std::process::Command;

        // Find utun interfaces with 100.x.x.x addresses
        let output = Command::new("sh")
            .args([
                "-c",
                "ifconfig | grep -B1 'inet 100\\.' | grep -E '^utun[0-9]+:' | head -1 | cut -d: -f1",
            ])
            .output()
            .ok()?;

        if output.status.success() {
            let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !name.is_empty() && name.starts_with("utun") {
                return Some(name);
            }
        }

        None
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

/// L2 VPN (TAP) support module - Linux only
///
/// This module provides Layer 2 Ethernet bridging capabilities using TAP devices.
/// L2 mode allows bridging Ethernet frames between peers, enabling:
/// - Non-IP protocols (ARP, DHCP relay, NetBIOS, etc.)
/// - MAC address visibility across the mesh
/// - True Layer 2 LAN bridging
///
/// # Requirements
/// - Linux only (TAP devices require Linux kernel support)
/// - Compile with `--features l2-vpn`
/// - OmniNervous v0.5.0+ with L2 module
///
/// # Example
/// ```ignore
/// use omni_tun::l2::OmniTapTun;
///
/// let mut tap = OmniTapTun::new("omniedge-tap0")?;
/// tap.setup("10.0.0.1", 51820, &private_key).await?;
/// ```
#[cfg(all(feature = "l2-vpn", target_os = "linux"))]
pub mod l2 {
    use anyhow::Result;
    use log::info;

    /// OmniTapTun provides Layer 2 TAP-based Ethernet bridging.
    ///
    /// Unlike the standard OmniTun (TUN/L3), this creates a TAP device that
    /// operates at the Ethernet layer, allowing Ethernet frame forwarding.
    #[derive(Clone)]
    pub struct OmniTapTun {
        /// TAP interface name
        ifname: String,
        /// Virtual IP address for the TAP interface
        vip: Option<String>,
        // TODO: Add OmniNervous L2Transport when available
        // l2_transport: Option<omninervous::l2::L2Transport>,
    }

    impl OmniTapTun {
        /// Create a new L2 TAP interface.
        ///
        /// # Arguments
        /// * `ifname` - Name for the TAP interface (e.g., "omniedge-tap0")
        ///
        /// # Returns
        /// A new OmniTapTun instance (not yet configured)
        pub fn new(ifname: &str) -> Result<Self> {
            info!("Creating L2 TAP interface: {}", ifname);
            Ok(Self {
                ifname: ifname.to_string(),
                vip: None,
            })
        }

        /// Setup the TAP interface with the given configuration.
        ///
        /// This creates the TAP device and configures it with the specified
        /// IP address. The TAP device will be ready to send/receive Ethernet frames.
        ///
        /// # Arguments
        /// * `vip` - Virtual IP address for the interface
        /// * `port` - UDP port for WireGuard-over-L2 encapsulation
        /// * `private_key` - WireGuard private key (hex encoded)
        ///
        /// # Note
        /// This is a stub implementation. Full L2 support requires OmniNervous
        /// L2 module integration which is available in OmniNervous v0.5.0+.
        pub async fn setup(&mut self, vip: &str, _port: u16, _private_key: &str) -> Result<()> {
            self.vip = Some(vip.to_string());

            // TODO: Implement actual TAP creation using OmniNervous L2 module
            // When OmniNervous L2 module is available:
            // 1. Create TAP device via omninervous::l2::L2Transport::new()
            // 2. Configure IP address
            // 3. Set up L2Fragmenter for MTU handling
            // 4. Initialize L2Metrics for monitoring

            info!(
                "L2 TAP interface {} configured with VIP {} (stub - full L2 pending OmniNervous integration)",
                self.ifname, vip
            );

            // For now, return error indicating L2 is not fully implemented
            Err(anyhow::anyhow!(
                "L2 TAP mode is not yet fully implemented. \
                OmniNervous L2 module integration is pending. \
                Please use L3 mode (--transport-mode l3) for now."
            ))
        }

        /// Get the interface name
        pub fn interface_name(&self) -> &str {
            &self.ifname
        }

        /// Get the configured VIP (if any)
        pub fn vip(&self) -> Option<&str> {
            self.vip.as_deref()
        }

        /// Shutdown the TAP interface
        pub async fn shutdown(&self) {
            info!("Shutting down L2 TAP interface: {}", self.ifname);
            // TODO: Cleanup TAP device
        }
    }
}

/// Stub L2 module for non-Linux platforms or when l2-vpn feature is disabled
#[cfg(not(all(feature = "l2-vpn", target_os = "linux")))]
pub mod l2 {
    use anyhow::Result;

    /// Stub OmniTapTun for platforms that don't support L2 mode.
    ///
    /// L2 TAP mode is only supported on Linux. On other platforms,
    /// this stub will return an error when attempting to use L2 mode.
    #[derive(Clone)]
    pub struct OmniTapTun {
        _ifname: String,
    }

    impl OmniTapTun {
        /// Create a new L2 TAP interface (stub - always fails on non-Linux).
        pub fn new(ifname: &str) -> Result<Self> {
            #[cfg(not(target_os = "linux"))]
            {
                let _ = ifname; // Suppress unused warning
                return Err(anyhow::anyhow!(
                    "L2 TAP mode is only supported on Linux. \
                    TAP devices require Linux kernel support. \
                    Please use L3 mode (--transport-mode l3) on this platform."
                ));
            }

            #[cfg(all(target_os = "linux", not(feature = "l2-vpn")))]
            {
                let _ = ifname; // Suppress unused warning
                return Err(anyhow::anyhow!(
                    "L2 TAP mode requires the 'l2-vpn' feature. \
                    Rebuild with: cargo build --features l2-vpn"
                ));
            }

            #[allow(unreachable_code)]
            Ok(Self {
                _ifname: ifname.to_string(),
            })
        }

        /// Setup stub (always fails on non-Linux)
        pub async fn setup(&mut self, _vip: &str, _port: u16, _private_key: &str) -> Result<()> {
            Err(anyhow::anyhow!(
                "L2 TAP mode is not available on this platform"
            ))
        }

        /// Shutdown stub
        pub async fn shutdown(&self) {}
    }
}
