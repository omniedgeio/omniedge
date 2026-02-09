use crate::RunMode;
use anyhow::{anyhow, Result};
use ipnetwork::Ipv4Network;
use local_ip_address::local_ip;
use mac_address::get_mac_address;
use machineid_rs::{Encryption, IdBuilder};
use network_interface::{NetworkInterface, NetworkInterfaceConfig};
use omni_api::{CreateUserServerRequest, UpdateUserServerRequest, UserServerService};
use std::time::Duration;
use uuid::Uuid;

pub fn get_hardware_id() -> Result<String> {
    let mut builder = IdBuilder::new(Encryption::SHA256);
    builder
        .add_component(machineid_rs::HWIDComponent::SystemID)
        .add_component(machineid_rs::HWIDComponent::CPUID)
        .add_component(machineid_rs::HWIDComponent::DriveSerial);

    match builder.build("omniedge") {
        Ok(id) => {
            // Map the hash to a stable UUID-like format
            if id.len() >= 32 {
                let hex_id = &id[0..32];
                if let Ok(bytes) = hex::decode(hex_id) {
                    if let Ok(u) = Uuid::from_slice(&bytes) {
                        return Ok(u.to_string());
                    }
                }
            }
            Ok(id[..std::cmp::min(id.len(), 36)].to_string())
        }
        Err(_) => {
            // Fallback to hostname-username if machineid fails (consistent with Desktop)
            let hostname = whoami::fallible::hostname().unwrap_or_else(|_| "unknown".to_string());
            let username = whoami::username();
            Ok(format!("{}-{}", hostname, username))
        }
    }
}

pub fn run_native_scan(cidr: &str, _timeout_secs: i64) -> Result<Vec<omni_api::types::ScanResult>> {
    log::info!("Running native scan on {}...", cidr);

    let network: Ipv4Network = cidr.parse().map_err(|e| anyhow!("Invalid CIDR: {}", e))?;

    let rt = tokio::runtime::Runtime::new()?;
    let scan_results = rt.block_on(async {
        let mut tasks = Vec::new();
        // Limit to a reasonable range if CIDR is too large (e.g. /24)
        for ip in network.iter().take(256) {
            tasks.push(tokio::spawn(async move {
                let ports = [80, 443, 22, 135];
                for port in ports {
                    let addr = format!("{}:{}", ip, port);
                    if let Ok(Ok(_)) = tokio::time::timeout(
                        Duration::from_millis(100),
                        tokio::net::TcpStream::connect(&addr),
                    )
                    .await
                    {
                        return Some(ip);
                    }
                }
                None
            }));
        }

        let mut discovered = Vec::new();
        for task in tasks {
            if let Ok(Some(ip)) = task.await {
                discovered.push(omni_api::types::ScanResult {
                    ipv4: ip.to_string(),
                    host_name: String::new(),
                    ipv6: String::new(),
                    mac_address: String::new(),
                    vendor: String::new(),
                    os: String::new(),
                });
            }
        }
        discovered
    });

    Ok(scan_results)
}

pub struct DeviceNet {
    pub ip: String,
    pub mac: String,
    pub mask: String,
}

pub fn get_current_device_net_status(cidr_hint: &str) -> Result<DeviceNet> {
    let my_local_ip = local_ip().map_err(|e| anyhow!("Failed to get local IP: {}", e))?;
    let interfaces =
        NetworkInterface::show().map_err(|e| anyhow!("Failed to list interfaces: {}", e))?;

    for iface in interfaces {
        for addr in iface.addr {
            if addr.ip() == my_local_ip {
                let mac = get_mac_address()
                    .unwrap_or_default()
                    .map(|m| m.to_string())
                    .unwrap_or_else(|| "00:00:00:00:00:00".to_string());

                let mask = addr
                    .netmask()
                    .map(|m| m.to_string())
                    .unwrap_or_else(|| "255.255.255.0".to_string());

                return Ok(DeviceNet {
                    ip: my_local_ip.to_string(),
                    mac,
                    mask,
                });
            }
        }
    }

    // Fallback to hint if not found
    let network: Ipv4Network = cidr_hint
        .parse()
        .map_err(|e| anyhow!("Invalid CIDR hint: {}", e))?;

    Ok(DeviceNet {
        ip: my_local_ip.to_string(),
        mac: "00:00:00:00:00:00".to_string(),
        mask: network.mask().to_string(),
    })
}

pub async fn fetch_public_ip() -> Option<String> {
    // Try multiple methods to fetch public IP

    // Method 1: Use curl.exe on Windows (not the PowerShell alias)
    #[cfg(windows)]
    {
        // First try curl.exe directly (if installed, e.g., via Git for Windows)
        if let Ok(output) = tokio::process::Command::new("curl.exe")
            .args(["-s", "--connect-timeout", "5", "https://api.ipify.org"])
            .output()
            .await
        {
            if output.status.success() {
                let ip = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !ip.is_empty() && ip.chars().all(|c| c.is_ascii_digit() || c == '.') {
                    return Some(ip);
                }
            }
        }

        // Fallback: Use PowerShell's Invoke-WebRequest with ipify.org (returns plain text)
        if let Ok(output) = tokio::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "(Invoke-WebRequest -Uri 'https://api.ipify.org' -TimeoutSec 5 -UseBasicParsing).Content",
            ])
            .output()
            .await
        {
            if output.status.success() {
                let ip = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !ip.is_empty() && ip.chars().all(|c| c.is_ascii_digit() || c == '.') {
                    return Some(ip);
                }
            }
        }

        log::info!("Public IP lookup failed on Windows");
        return None;
    }

    #[cfg(not(windows))]
    {
        let output = tokio::process::Command::new("curl")
            .args(["-s", "--connect-timeout", "5", "https://api.ipify.org"])
            .output()
            .await
            .ok()?;

        if !output.status.success() {
            log::info!("Public IP lookup failed with status: {}", output.status);
            return None;
        }

        let ip = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if ip.is_empty() {
            log::info!("Public IP lookup returned empty response");
            return None;
        }

        Some(ip)
    }
}

pub async fn sync_custom_server(
    user_server_service: &UserServerService<'_>,
    auth_token: &str,
    mode: RunMode,
    nucleus_port: u16,
) -> Result<()> {
    if mode != RunMode::Nucleus && mode != RunMode::Dual {
        return Ok(());
    }

    if auth_token.is_empty() {
        log::info!("Skipping custom server sync (no auth token)");
        return Ok(());
    }

    let public_ip = match fetch_public_ip().await {
        Some(ip) => ip,
        None => {
            log::info!("Skipping custom server sync (public IP unavailable)");
            return Ok(());
        }
    };

    let hostname = whoami::fallible::hostname().unwrap_or_else(|_| "omniedge-device".to_string());
    let name = format!("Nucleus - {}", hostname);
    let host = format!("{}:{}", public_ip, nucleus_port);

    let servers = user_server_service.list().await?;
    let existing = servers
        .iter()
        .find(|server| !server.is_default && server.name == name);

    if let Some(server) = existing {
        let request = UpdateUserServerRequest {
            name: Some(name),
            host: Some(host),
            country: None,
        };
        let _ = user_server_service.update(&server.id, request).await?;
    } else {
        let request = CreateUserServerRequest {
            name,
            host,
            country: None,
        };
        let _ = user_server_service.create(request).await?;
    }

    Ok(())
}

/// Get the real user's home directory, even when running with sudo
#[cfg(not(windows))]
pub fn get_real_user_home() -> Option<std::path::PathBuf> {
    use std::path::PathBuf;
    // First check SUDO_USER (set when running with sudo)
    if let Ok(sudo_user) = std::env::var("SUDO_USER") {
        if !sudo_user.is_empty() && sudo_user != "root" {
            // Try to get the user's home from /etc/passwd or expand ~user
            if let Ok(output) = std::process::Command::new("sh")
                .args(["-c", &format!("eval echo ~{}", sudo_user)])
                .output()
            {
                let home = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !home.is_empty() && home != "~" && std::path::Path::new(&home).exists() {
                    return Some(PathBuf::from(home));
                }
            }
            // Fallback: try common home paths
            let home_path = PathBuf::from(format!("/home/{}", sudo_user));
            if home_path.exists() {
                return Some(home_path);
            }
        }
    }
    None
}

/// Get the real user's home directory on Windows
/// On Windows, elevation doesn't change the user context like sudo does on Unix,
/// so we just return the standard user profile directory.
#[cfg(windows)]
pub fn get_real_user_home() -> Option<std::path::PathBuf> {
    dirs::home_dir()
}
