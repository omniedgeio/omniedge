use anyhow::{anyhow, Result};
use ipnetwork::Ipv4Network;
use local_ip_address::local_ip;
use mac_address::get_mac_address;
use machineid_rs::{Encryption, IdBuilder};
use network_interface::{NetworkInterface, NetworkInterfaceConfig};
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
