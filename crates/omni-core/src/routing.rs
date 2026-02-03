#[allow(unused_imports)]
use anyhow::{anyhow, Context, Result};
use log::info;
use std::process::Command;

/// Default DNS server to use when system DNS cannot be detected
const FALLBACK_DNS: &str = "8.8.8.8";
/// Default IPv6 DNS server fallback
#[allow(dead_code)]
const FALLBACK_DNS_V6: &str = "2001:4860:4860::8888";

pub struct RoutingManager;

impl RoutingManager {
    /// Get the system's configured DNS server, falling back to Google DNS if detection fails
    fn get_dns_server() -> String {
        Self::detect_system_dns().unwrap_or_else(|| FALLBACK_DNS.to_string())
    }

    /// Attempt to detect the system's DNS server
    fn detect_system_dns() -> Option<String> {
        #[cfg(target_os = "linux")]
        {
            // Try systemd-resolve first
            if let Ok(output) = Self::run_command("sh", &["-c", "resolvectl status 2>/dev/null | grep 'Current DNS Server' | head -1 | awk '{print $NF}'"]) {
                let dns = output.trim();
                if !dns.is_empty() && Self::is_valid_ip(dns) {
                    return Some(dns.to_string());
                }
            }
            // Fall back to resolv.conf
            if let Ok(output) = Self::run_command(
                "sh",
                &[
                    "-c",
                    "grep -m1 '^nameserver' /etc/resolv.conf | awk '{print $2}'",
                ],
            ) {
                let dns = output.trim();
                if !dns.is_empty() && Self::is_valid_ip(dns) {
                    return Some(dns.to_string());
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            if let Ok(output) = Self::run_command(
                "sh",
                &[
                    "-c",
                    "scutil --dns | grep 'nameserver\\[0\\]' | head -1 | awk '{print $3}'",
                ],
            ) {
                let dns = output.trim();
                if !dns.is_empty() && Self::is_valid_ip(dns) {
                    return Some(dns.to_string());
                }
            }
        }

        #[cfg(target_os = "windows")]
        {
            if let Ok(output) = Self::run_command("powershell", &["-Command", "Get-DnsClientServerAddress -AddressFamily IPv4 | Where-Object { $_.ServerAddresses } | Select-Object -First 1 -ExpandProperty ServerAddresses | Select-Object -First 1"]) {
                let dns = output.trim();
                if !dns.is_empty() && Self::is_valid_ip(dns) {
                    return Some(dns.to_string());
                }
            }
        }

        None
    }

    /// Validate that a string is a valid IP address (IPv4 or IPv6)
    fn is_valid_ip(s: &str) -> bool {
        s.parse::<std::net::IpAddr>().is_ok()
    }

    /// Validate that a string is a valid IPv4 address
    #[allow(dead_code)]
    fn is_valid_ipv4(s: &str) -> bool {
        s.parse::<std::net::Ipv4Addr>().is_ok()
    }

    /// Validate that a string is a valid IPv6 address
    #[allow(dead_code)]
    fn is_valid_ipv6(s: &str) -> bool {
        s.parse::<std::net::Ipv6Addr>().is_ok()
    }
}

impl RoutingManager {
    /// Setup exit node routing for both IPv4 and optionally IPv6
    ///
    /// # Arguments
    /// * `exit_node_ip` - The IPv4 address of the exit node
    /// * `exit_node_ip_v6` - Optional IPv6 address of the exit node
    /// * `nucleus_host` - The nucleus server host (for persistent route)
    ///
    /// # Safety
    /// IPv6 routing is optional and failures are logged but don't fail the overall setup.
    /// This ensures backward compatibility with IPv4-only exit nodes.
    pub fn setup_exit_node(
        exit_node_ip: &str,
        exit_node_ip_v6: Option<&str>,
        nucleus_host: &str,
    ) -> Result<()> {
        #[cfg(target_os = "linux")]
        return Self::setup_linux(exit_node_ip, exit_node_ip_v6, nucleus_host);

        #[cfg(target_os = "macos")]
        return Self::setup_macos(exit_node_ip, exit_node_ip_v6, nucleus_host);

        #[cfg(target_os = "windows")]
        return Self::setup_windows(exit_node_ip, exit_node_ip_v6, nucleus_host);

        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        Err(anyhow!("Exit node not supported on this OS"))
    }

    pub fn restore_exit_node() -> Result<()> {
        #[cfg(target_os = "linux")]
        return Self::restore_linux();

        #[cfg(target_os = "macos")]
        return Self::restore_macos();

        #[cfg(target_os = "windows")]
        return Self::restore_windows();

        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        Ok(())
    }

    // --- Linux Implementation Shell ---
    #[cfg(target_os = "linux")]
    fn setup_linux(
        exit_node_ip: &str,
        exit_node_ip_v6: Option<&str>,
        nucleus_host: &str,
    ) -> Result<()> {
        info!(
            "Setting up exit node on Linux: {} (v6: {:?}) via {}",
            exit_node_ip, exit_node_ip_v6, nucleus_host
        );

        // 1. Resolve nucleus IP
        let host = if let Some(pos) = nucleus_host.find(':') {
            &nucleus_host[..pos]
        } else {
            nucleus_host
        };

        let output = Self::run_command("getent", &["ahosts", host])?;
        let nucleus_ip = output
            .lines()
            .next()
            .and_then(|l| l.split_whitespace().next())
            .context("Failed to resolve nucleus host")?;

        // 2. Get primary interface
        let iface = Self::get_primary_interface()?;

        // 3. Get original gateway (IPv4)
        let gateway_output = Self::run_command(
            "sh",
            &[
                "-c",
                &format!(
                    "ip route show dev {} | head -n1 | awk '{{print $3}}'",
                    iface
                ),
            ],
        )?;
        let original_gateway = gateway_output.trim();
        if original_gateway.is_empty() {
            return Err(anyhow!(
                "Could not determine current gateway for interface {}",
                iface
            ));
        }

        // 4. Add route to nucleus via original gateway
        Self::run_command(
            "sudo",
            &[
                "ip",
                "route",
                "add",
                nucleus_ip,
                "via",
                original_gateway,
                "dev",
                &iface,
            ],
        )?;

        // 5. Update default IPv4 gateway to tunnel
        let _ = Self::run_command("sudo", &["ip", "route", "del", "default", "dev", &iface]);
        Self::run_command(
            "sudo",
            &["ip", "route", "add", "default", "via", exit_node_ip],
        )?;

        // 6. Setup IPv6 routing if exit node has IPv6 (safe - failures logged but don't fail overall)
        if let Some(exit_v6) = exit_node_ip_v6 {
            info!("Setting up IPv6 exit node routing via {}", exit_v6);

            // Delete existing IPv6 default route (ignore errors)
            let _ = Self::run_command("sudo", &["ip", "-6", "route", "del", "default"]);

            // Add IPv6 default route via exit node
            match Self::run_command(
                "sudo",
                &["ip", "-6", "route", "add", "default", "via", exit_v6],
            ) {
                Ok(_) => info!("IPv6 exit node routing configured successfully"),
                Err(e) => {
                    // Log warning but don't fail - IPv4 exit node still works
                    log::warn!(
                        "Failed to setup IPv6 exit node routing (IPv4 still active): {}",
                        e
                    );
                }
            }
        }

        Self::setup_dns_linux(&iface)?;

        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn setup_dns_linux(iface: &str) -> Result<()> {
        let dns_server = Self::get_dns_server();
        info!(
            "Setting up DNS on Linux ({}) for interface {}",
            dns_server, iface
        );

        // Try resolvectl first (modern systemd)
        if Self::run_command("sh", &["-c", "command -v resolvectl"]).is_ok() {
            info!("Using resolvectl for DNS configuration");
            let _ = Self::run_command("sudo", &["resolvectl", "dns", iface, &dns_server]);
            let _ = Self::run_command("sudo", &["resolvectl", "domain", iface, "~."]);
            return Ok(());
        }

        // Fallback to resolv.conf only if not using resolvectl
        let check_symlink = Self::run_command("sh", &["-c", "test -L /etc/resolv.conf"]);
        if check_symlink.is_err() {
            let _ = Self::run_command(
                "sudo",
                &["cp", "/etc/resolv.conf", "/etc/resolv.conf.omniedge_bak"],
            );
        }

        Self::run_command(
            "sh",
            &[
                "-c",
                &format!(
                    "echo \"nameserver {}\" | sudo tee /etc/resolv.conf",
                    dns_server
                ),
            ],
        )?;
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn restore_linux() -> Result<()> {
        info!("Restoring original routing on Linux");

        // Restore IPv6 default route (remove tunnel route, let system reconfigure)
        // This is safe - if there was no IPv6 exit route, this just fails silently
        let _ = Self::run_command("sudo", &["ip", "-6", "route", "del", "default"]);
        info!("IPv6 default route cleared (will be restored by DHCP/RA)");

        Self::restore_dns_linux()?;
        // Original gateway restoration logic would go here
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn restore_dns_linux() -> Result<()> {
        if let Ok(iface) = Self::get_primary_interface() {
            if Self::run_command("sh", &["-c", "command -v resolvectl"]).is_ok() {
                info!("Restoring DNS via resolvectl (Linux)");
                let _ = Self::run_command("sudo", &["resolvectl", "revert", &iface]);
                return Ok(());
            }
        }

        let check_bak = Self::run_command("sh", &["-c", "test -f /etc/resolv.conf.omniedge_bak"]);
        if check_bak.is_ok() {
            info!("Restoring DNS from backup (Linux)");
            Self::run_command(
                "sudo",
                &["mv", "/etc/resolv.conf.omniedge_bak", "/etc/resolv.conf"],
            )?;
        }
        Ok(())
    }

    // --- macOS Implementation Shell ---
    #[cfg(target_os = "macos")]
    fn setup_macos(
        exit_node_ip: &str,
        exit_node_ip_v6: Option<&str>,
        nucleus_host: &str,
    ) -> Result<()> {
        info!(
            "Setting up exit node on macOS: {} (v6: {:?}) via {}",
            exit_node_ip, exit_node_ip_v6, nucleus_host
        );

        let iface = Self::get_primary_interface()?;
        let gateway_output = Self::run_command(
            "sh",
            &[
                "-c",
                &format!(
                    "networksetup -getadditionalroutes {} | head -n1 | awk '{{print $3}}'",
                    iface
                ),
            ],
        )
        .or_else(|_| {
            Self::run_command(
                "sh",
                &[
                    "-c",
                    "route -n get default | grep gateway | awk '{print $2}'",
                ],
            )
        })?;

        let original_gateway = gateway_output.trim();
        if original_gateway.is_empty() {
            return Err(anyhow!("Could not determine current gateway"));
        }

        // Add route to nucleus via original gateway
        Self::run_command(
            "sudo",
            &["route", "-n", "add", "-net", nucleus_host, original_gateway],
        )?;

        // Update default IPv4 gateway
        let _ = Self::run_command("sudo", &["route", "delete", "default"]);
        Self::run_command(
            "sudo",
            &["route", "-n", "add", "-net", "0.0.0.0", exit_node_ip],
        )?;

        // Setup IPv6 routing if exit node has IPv6 (safe - failures logged but don't fail overall)
        if let Some(exit_v6) = exit_node_ip_v6 {
            info!("Setting up IPv6 exit node routing via {}", exit_v6);

            // Delete existing IPv6 default route (ignore errors)
            let _ = Self::run_command("sudo", &["route", "-n", "delete", "-inet6", "default"]);

            // Add IPv6 default route via exit node
            match Self::run_command(
                "sudo",
                &["route", "-n", "add", "-inet6", "default", exit_v6],
            ) {
                Ok(_) => info!("IPv6 exit node routing configured successfully"),
                Err(e) => {
                    // Log warning but don't fail - IPv4 exit node still works
                    log::warn!(
                        "Failed to setup IPv6 exit node routing (IPv4 still active): {}",
                        e
                    );
                }
            }
        }

        Self::setup_dns_macos(&iface)?;

        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn setup_dns_macos(iface: &str) -> Result<()> {
        let dns_server = Self::get_dns_server();
        info!(
            "Setting up DNS on macOS ({}) for interface {}",
            dns_server, iface
        );
        // On macOS, networksetup uses service names, not interface names (en0).
        // We need to find the service associated with the interface.
        let service_out = Self::run_command("sh", &["-c", &format!("networksetup -listallhardwareports | grep -B 1 {} | head -n 1 | cut -d ' ' -f 3-", iface)])?;
        let service = service_out.trim();

        if service.is_empty() {
            return Err(anyhow!(
                "Could not find network service for interface {}",
                iface
            ));
        }

        // 2. Set DNS
        Self::run_command(
            "sudo",
            &["networksetup", "-setdnsservers", service, &dns_server],
        )?;
        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn restore_macos() -> Result<()> {
        info!("Restoring original routing on macOS");

        // Restore IPv6 default route (remove tunnel route, let system reconfigure)
        // This is safe - if there was no IPv6 exit route, this just fails silently
        let _ = Self::run_command("sudo", &["route", "-n", "delete", "-inet6", "default"]);
        info!("IPv6 default route cleared (will be restored by DHCP/RA)");

        Self::restore_dns_macos()?;
        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn restore_dns_macos() -> Result<()> {
        let service_out = Self::run_command(
            "sh",
            &[
                "-c",
                "networksetup -listallnetworkservices | grep -v '*' | head -n 1",
            ],
        )?;
        let service = service_out.trim();
        if !service.is_empty() {
            info!(
                "Restoring DNS to Empty (DHCP default) on macOS service: {}",
                service
            );
            Self::run_command(
                "sudo",
                &["networksetup", "-setdnsservers", service, "Empty"],
            )?;
        }
        Ok(())
    }

    // --- Windows Implementation Shell ---
    #[cfg(target_os = "windows")]
    fn setup_windows(
        exit_node_ip: &str,
        exit_node_ip_v6: Option<&str>,
        _nucleus_host: &str,
    ) -> Result<()> {
        info!(
            "Setting up exit node on Windows: {} (v6: {:?})",
            exit_node_ip, exit_node_ip_v6
        );
        let iface = Self::get_primary_interface()?;

        // On Windows, 'route' can use interface names or indices.
        // We'll use the interface alias we detected.
        let _ = Self::run_command("route", &["delete", "0.0.0.0"]);
        Self::run_command(
            "route",
            &[
                "ADD",
                "0.0.0.0",
                "MASK",
                "0.0.0.0",
                exit_node_ip,
                "IF",
                &iface,
            ],
        )?;

        // Setup IPv6 routing if exit node has IPv6 (safe - failures logged but don't fail overall)
        if let Some(exit_v6) = exit_node_ip_v6 {
            info!("Setting up IPv6 exit node routing via {}", exit_v6);

            // Get interface index for IPv6 routing
            let iface_idx = Self::get_interface_index(&iface).unwrap_or_else(|_| iface.clone());

            // Delete existing IPv6 default route (ignore errors)
            let _ = Self::run_command("route", &["-6", "delete", "::/0"]);

            // Add IPv6 default route via exit node using PowerShell for better reliability
            // PowerShell's New-NetRoute is more reliable for IPv6 on Windows
            match Self::run_command(
                "powershell",
                &[
                    "-Command",
                    &format!(
                        "New-NetRoute -DestinationPrefix '::/0' -NextHop '{}' -InterfaceAlias '{}' -PolicyStore ActiveStore -ErrorAction SilentlyContinue",
                        exit_v6, iface
                    ),
                ],
            ) {
                Ok(_) => info!("IPv6 exit node routing configured successfully"),
                Err(e) => {
                    // Try fallback with route command
                    match Self::run_command(
                        "route",
                        &["-6", "ADD", "::/0", exit_v6, "IF", &iface_idx],
                    ) {
                        Ok(_) => info!("IPv6 exit node routing configured via route command"),
                        Err(e2) => {
                            // Log warning but don't fail - IPv4 exit node still works
                            log::warn!(
                                "Failed to setup IPv6 exit node routing (IPv4 still active): PowerShell: {}, route: {}",
                                e, e2
                            );
                        }
                    }
                }
            }
        }

        Self::setup_dns_windows(&iface)?;

        Ok(())
    }

    /// Get Windows interface index from interface alias
    #[cfg(target_os = "windows")]
    fn get_interface_index(iface_alias: &str) -> Result<String> {
        let output = Self::run_command(
            "powershell",
            &[
                "-Command",
                &format!(
                    "Get-NetAdapter -Name '{}' | Select-Object -ExpandProperty ifIndex",
                    iface_alias
                ),
            ],
        )?;
        let idx = output.trim();
        if idx.is_empty() {
            return Err(anyhow!("Could not get interface index for {}", iface_alias));
        }
        Ok(idx.to_string())
    }

    #[cfg(target_os = "windows")]
    fn setup_dns_windows(iface: &str) -> Result<()> {
        let dns_server = Self::get_dns_server();
        info!(
            "Setting up DNS on Windows ({}) for interface {}",
            dns_server, iface
        );
        let _ = Self::run_command(
            "netsh",
            &[
                "interface",
                "ip",
                "set",
                "dns",
                iface,
                "static",
                &dns_server,
            ],
        );
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn restore_windows() -> Result<()> {
        info!("Restoring original routing on Windows");

        // Restore IPv6 default route (remove tunnel route)
        // Try PowerShell first, then fallback to route command
        let _ = Self::run_command(
            "powershell",
            &[
                "-Command",
                "Remove-NetRoute -DestinationPrefix '::/0' -PolicyStore ActiveStore -Confirm:$false -ErrorAction SilentlyContinue",
            ],
        );
        let _ = Self::run_command("route", &["-6", "delete", "::/0"]);
        info!("IPv6 default route cleared");

        Self::restore_dns_windows()?;
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn restore_dns_windows() -> Result<()> {
        info!("Restoring DNS to DHCP on Windows");
        let _ = Self::run_command(
            "netsh",
            &["interface", "ip", "set", "dns", "Ethernet", "dhcp"],
        );
        let _ = Self::run_command("netsh", &["interface", "ip", "set", "dns", "Wi-Fi", "dhcp"]);
        Ok(())
    }

    fn get_primary_interface() -> Result<String> {
        #[cfg(target_os = "linux")]
        {
            // Use 8.8.8.8 as a routing probe destination (doesn't actually connect)
            // This determines which interface handles default internet traffic
            let out = Self::run_command(
                "sh",
                &["-c", "ip route get 8.8.8.8 | grep -oP 'dev \\K\\S+'"],
            )?;
            let iface = out.trim();
            if iface.is_empty() {
                return Err(anyhow!("Could not detect primary Linux interface"));
            }
            Ok(iface.to_string())
        }

        #[cfg(target_os = "macos")]
        {
            let out = Self::run_command(
                "sh",
                &[
                    "-c",
                    "route -n get default | grep interface | awk '{print $2}'",
                ],
            )?;
            let iface = out.trim();
            if iface.is_empty() {
                return Err(anyhow!("Could not detect primary macOS interface"));
            }
            Ok(iface.to_string())
        }

        #[cfg(target_os = "windows")]
        {
            let out = Self::run_command("powershell", &["-Command", "Get-NetIPInterface -AddressFamily IPv4 -ConnectionState Connected | Sort-Object InterfaceMetric | Select-Object -First 1 -ExpandProperty InterfaceAlias"])?;
            let iface = out.trim();
            if iface.is_empty() {
                return Err(anyhow!("Could not detect primary Windows interface"));
            }
            Ok(iface.to_string())
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        Err(anyhow!(
            "Primary interface detection not supported on this OS"
        ))
    }

    pub fn run_command(name: &str, args: &[&str]) -> Result<String> {
        let output = Command::new(name)
            .args(args)
            .output()
            .map_err(|e| anyhow!("Failed to execute command {} {:?}: {}", name, args, e))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow!("Command {} {:?} failed: {}", name, args, stderr))
        }
    }
}
