use crate::utils::get_hardware_id;
use crate::RunMode;
#[cfg(windows)]
use crate::SERVICE_NAME;
use anyhow::{Context, Result};
use log::info;
use omni_core::{CliConfig, ConnectionManager};
use omni_proto::{handle_nucleus_message, NucleusState};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;

#[derive(Debug, Serialize, Deserialize)]
struct HelperRequest {
    command: String,
    args: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct HelperResponse {
    success: bool,
    message: String,
    data: Option<serde_json::Value>,
}

/// Try to connect to the omni-helper service
async fn call_helper(req: &HelperRequest) -> Result<HelperResponse> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[cfg(windows)]
    {
        use tokio::net::windows::named_pipe::ClientOptions;
        let pipe_name = r"\\.\pipe\omniedge-helper";

        let mut client = ClientOptions::new().open(pipe_name)?;
        let payload = serde_json::to_vec(req)?;
        client.write_all(&payload).await?;
        client.shutdown().await?;

        let mut buf = Vec::new();
        client.read_to_end(&mut buf).await?;
        let resp: HelperResponse = serde_json::from_slice(&buf)?;
        Ok(resp)
    }

    #[cfg(unix)]
    {
        use tokio::net::UnixStream;
        let socket_path = "/var/run/omniedge-helper.sock";

        let mut stream = UnixStream::connect(socket_path).await?;
        let payload = serde_json::to_vec(req)?;
        stream.write_all(&payload).await?;
        stream.shutdown().await?;

        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).await?;
        let resp: HelperResponse = serde_json::from_slice(&buf)?;
        Ok(resp)
    }
}

/// Check if helper service is running and available
async fn is_helper_available() -> bool {
    let req = HelperRequest {
        command: "ping".to_string(),
        args: serde_json::json!({}),
    };

    match call_helper(&req).await {
        Ok(resp) => resp.success,
        Err(_) => false,
    }
}

/// Status information for the OmniEdge service
#[derive(Debug, Default)]
pub struct ServiceStatus {
    pub is_running: bool,
    pub virtual_ip: Option<String>,
    pub network_id: Option<String>,
    pub interface_name: Option<String>,
    pub mode: Option<String>,
    pub nucleus_port: Option<u16>,
}

/// Get comprehensive status of the OmniEdge service
pub async fn get_service_status(
    last_mode: Option<&str>,
    nucleus_port: Option<u16>,
) -> ServiceStatus {
    let mut status = ServiceStatus::default();

    // First check if omniedge daemon process is running
    let daemon_running = is_omniedge_daemon_running();

    // Check based on last known mode
    match last_mode {
        Some("nucleus") => {
            // Nucleus-only mode: check if nucleus port is in use AND daemon is running
            if let Some(port) = nucleus_port {
                if daemon_running && is_port_in_use(port) {
                    status.is_running = true;
                    status.mode = Some("nucleus".to_string());
                    status.nucleus_port = Some(port);
                }
            }
        }
        Some("dual") => {
            // Dual mode: check daemon running AND interface
            if daemon_running {
                if let Some(iface_info) = get_omniedge_interface() {
                    status.is_running = true;
                    status.interface_name = Some(iface_info.name);
                    status.virtual_ip = Some(iface_info.ip);
                    status.mode = Some("dual".to_string());
                    if let Some(port) = nucleus_port {
                        if is_port_in_use(port) {
                            status.nucleus_port = Some(port);
                        }
                    }
                }
            }
        }
        _ => {
            // Edge mode (default): check daemon running AND interface exists with valid IP
            if daemon_running {
                if let Some(iface_info) = get_omniedge_interface() {
                    status.is_running = true;
                    status.interface_name = Some(iface_info.name);
                    status.virtual_ip = Some(iface_info.ip);
                    status.mode = Some("edge".to_string());
                }
            }
        }
    }

    // Try to get additional info from helper if available
    if is_helper_available().await {
        let req = HelperRequest {
            command: "status".to_string(),
            args: serde_json::json!({}),
        };
        if let Ok(resp) = call_helper(&req).await {
            if resp.success {
                if let Some(data) = resp.data {
                    if let Some(net_id) = data.get("network_id").and_then(|v| v.as_str()) {
                        if !net_id.is_empty() {
                            status.network_id = Some(net_id.to_string());
                        }
                    }
                    // Use helper's virtual_ip if we don't have one from interface
                    if status.virtual_ip.is_none() {
                        if let Some(vip) = data.get("virtual_ip").and_then(|v| v.as_str()) {
                            if !vip.is_empty() {
                                status.virtual_ip = Some(vip.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    status
}

/// Check if a UDP port is in use
fn is_port_in_use(port: u16) -> bool {
    use std::net::UdpSocket;
    // Try to bind to the port - if it fails, the port is in use
    UdpSocket::bind(format!("0.0.0.0:{}", port)).is_err()
}

struct InterfaceInfo {
    name: String,
    ip: String,
}

/// Get OmniEdge network interface information
/// On macOS, we identify the correct utun interface by matching the expected virtual IP from config
fn get_omniedge_interface() -> Option<InterfaceInfo> {
    use network_interface::{NetworkInterface, NetworkInterfaceConfig};

    let interfaces = NetworkInterface::show().ok()?;

    // Try to get the expected virtual IP from config
    let expected_vip = CliConfig::load()
        .ok()
        .and_then(|c| c.last_join_info)
        .map(|j| j.virtual_ip);

    // On Windows/Linux, look for interface by name
    // On macOS, we must match by virtual IP since utun names are assigned dynamically
    #[cfg(windows)]
    let target_names = ["OmniEdge"];
    #[cfg(target_os = "linux")]
    let target_names = ["omniedge0", "omniedge"];

    // First pass: try to find interface with matching virtual IP (most accurate for macOS)
    if let Some(ref vip) = expected_vip {
        for iface in &interfaces {
            for addr in &iface.addr {
                if let std::net::IpAddr::V4(ipv4) = addr.ip() {
                    if ipv4.to_string() == *vip {
                        return Some(InterfaceInfo {
                            name: iface.name.clone(),
                            ip: ipv4.to_string(),
                        });
                    }
                }
            }
        }
    }

    // Second pass: name-based detection (Windows/Linux only, not macOS)
    // On macOS, we cannot reliably detect by name since all VPNs use utun*
    #[cfg(not(target_os = "macos"))]
    for iface in interfaces {
        let name_lower = iface.name.to_lowercase();
        let is_omniedge = target_names
            .iter()
            .any(|t| name_lower.contains(&t.to_lowercase()));

        if is_omniedge {
            for addr in &iface.addr {
                if let std::net::IpAddr::V4(ipv4) = addr.ip() {
                    // Skip loopback and link-local
                    if !ipv4.is_loopback() && !ipv4.is_link_local() {
                        return Some(InterfaceInfo {
                            name: iface.name.clone(),
                            ip: ipv4.to_string(),
                        });
                    }
                }
            }
        }
    }

    None
}

/// Check if omniedge daemon process is running (excluding current process)
fn is_omniedge_daemon_running() -> bool {
    #[cfg(windows)]
    {
        use std::process::Command;
        if let Ok(output) = Command::new("tasklist")
            .args(["/FI", "IMAGENAME eq omniedge.exe", "/NH"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Count running instances - if more than 1 (current process), daemon is running
            return stdout.matches("omniedge.exe").count() > 1;
        }
    }

    #[cfg(unix)]
    {
        use std::process::Command;
        // Use pgrep to find omniedge processes
        if let Ok(output) = Command::new("pgrep")
            .args(["-f", "omniedge.*--daemon"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let pids: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();
            return !pids.is_empty();
        }
    }

    false
}

/// Check if OmniEdge service is currently running
#[allow(dead_code)]
pub async fn is_service_running(last_mode: Option<&str>, nucleus_port: Option<u16>) -> bool {
    get_service_status(last_mode, nucleus_port).await.is_running
}

/// Start VPN through helper service
/// NOTE: Currently disabled as the old helper uses incompatible n2n protocol
#[allow(dead_code)]
async fn start_via_helper(
    token: &str,
    network_id: &str,
    device_id: &str,
    hardware_id: &str,
    mode: RunMode,
    as_exit_node: bool,
    exit_node_ip: Option<String>,
) -> Result<()> {
    let mode_str = match mode {
        RunMode::Edge => "edge",
        RunMode::Nucleus => "nucleus",
        RunMode::Dual => "dual",
    };

    let req = HelperRequest {
        command: "start_vpn".to_string(),
        args: serde_json::json!({
            "token": token,
            "network_id": network_id,
            "device_id": device_id,
            "hardware_id": hardware_id,
            "mode": mode_str,
            "as_exit_node": as_exit_node,
            "exit_node_ip": exit_node_ip,
        }),
    };

    let resp = call_helper(&req).await?;
    if resp.success {
        Ok(())
    } else {
        Err(anyhow::anyhow!("Helper failed: {}", resp.message))
    }
}

/// Run nucleus-only signaling server (no VPN, no network auth)
pub async fn run_nucleus_only(port: u16, secret: &str) -> Result<()> {
    info!(
        "Starting OmniEdge Nucleus-only signaling server on port {}",
        port
    );

    let nucleus_state = Arc::new(Mutex::new(NucleusState::new()));
    let secret = if secret.is_empty() {
        None
    } else {
        Some(secret.to_string())
    };

    let socket = UdpSocket::bind(format!("0.0.0.0:{}", port)).await?;
    info!("Nucleus signaling server listening on UDP port {}", port);

    let mut buf = [0u8; 4096];
    let mut cleanup_interval = tokio::time::interval(tokio::time::Duration::from_secs(60));

    loop {
        tokio::select! {
            res = socket.recv_from(&mut buf) => {
                match res {
                    Ok((len, src)) => {
                        let pkt = &buf[..len];
                        if pkt.is_empty() || pkt[0] < 0x11 {
                            continue;
                        }

                        let mut state = nucleus_state.lock().await;
                        if let Some(response) = handle_nucleus_message(
                            &mut state,
                            pkt,
                            src,
                            secret.as_deref(),
                        ) {
                            if let Err(e) = socket.send_to(&response, src).await {
                                log::warn!("Failed to send nucleus response to {}: {}", src, e);
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Nucleus socket error: {}", e);
                    }
                }
            }
            _ = cleanup_interval.tick() => {
                let mut state = nucleus_state.lock().await;
                state.cleanup();
                log::debug!("Nucleus state cleanup complete. {} peers registered.", state.peer_count());
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Nucleus signaling server shutting down...");
                break;
            }
        }
    }

    Ok(())
}

/// Run edge or dual mode worker
pub async fn run_worker(
    base_url: &str,
    network_id: &str,
    mode: RunMode,
    as_exit_node: bool,
    exit_node: Option<String>,
    nucleus_port: u16,
    _cluster_secret: Option<String>,
) -> Result<()> {
    let mode_str = match mode {
        RunMode::Edge => "edge",
        RunMode::Nucleus => "nucleus-only",
        RunMode::Dual => "dual (edge + nucleus)",
    };
    log::info!(
        "Starting OmniEdge background worker in {} mode for network: {} (API: {})",
        mode_str,
        network_id,
        base_url
    );

    let config = CliConfig::load().context("Failed to load config")?;
    let auth = config.auth_response.context("Not authenticated")?;
    let device_id = config.device_uuid.context("Device not registered")?;

    let identity_pk: Option<[u8; 32]> = config
        .identity_private_key
        .as_ref()
        .and_then(|k| hex::decode(k).ok())
        .and_then(|b| b.try_into().ok());

    let mut manager = ConnectionManager::new(base_url.to_string(), identity_pk);

    // Configure nucleus settings if in dual mode
    // Note: In dual mode, the secret comes from the backend (join_resp.secret_key),
    // so we pass None here - the CLI --secret is only for nucleus-only mode.
    if mode == RunMode::Dual {
        manager.set_nucleus_config(nucleus_port, None);
    }

    let is_nucleus = mode == RunMode::Dual;
    info!("Connecting with token for network: {}...", network_id);
    let join_resp = manager
        .connect_with_token(
            auth.token,
            network_id,
            &device_id,
            &get_hardware_id().unwrap_or_else(|_| "unknown".to_string()),
            is_nucleus,
            as_exit_node,
            exit_node,
        )
        .await?;

    // Save the join info to config for status detection
    {
        let mut config = CliConfig::load().unwrap_or_default();
        config.last_join_info = Some(join_resp);
        config.last_network_id = Some(network_id.to_string());
        if let Err(e) = config.save() {
            log::warn!("Failed to save join info to config: {}", e);
        } else {
            info!("Saved join info to config for status detection");
        }
    }

    info!("Worker connected successfully. Waiting for SIGINT...");

    tokio::signal::ctrl_c().await?;
    manager.disconnect().await?;
    Ok(())
}

/// Setup and start nucleus-only service
pub async fn setup_and_start_nucleus_service(port: u16, secret: &str) -> Result<()> {
    info!(
        "Starting nucleus-only background service on port {}...",
        port
    );

    #[cfg(windows)]
    {
        setup_windows_nucleus_service(port, secret).await?;
    }

    #[cfg(target_os = "linux")]
    {
        setup_linux_nucleus_service(port, secret)?;
    }

    #[cfg(target_os = "macos")]
    {
        setup_macos_nucleus_service(port, secret)?;
    }

    Ok(())
}

pub async fn setup_and_start_service(
    _base_url: &str,
    network_id: &str,
    mode: RunMode,
    as_exit_node: bool,
    exit_node: Option<&str>,
    nucleus_port: u16,
    cluster_secret: Option<&str>,
) -> Result<()> {
    // NOTE: The old omni-helper service uses n2n protocol which is incompatible
    // with the new WireGuard-based CLI. Skip helper and use standalone service.
    // TODO: Re-enable helper when omni-helper is updated to use WireGuard protocol.

    // Create standalone background service with sudo (required for TUN on macOS)
    info!("Starting standalone background service...");

    #[cfg(windows)]
    {
        setup_windows_service(
            _base_url,
            network_id,
            mode,
            as_exit_node,
            exit_node,
            nucleus_port,
            cluster_secret,
        )
        .await?;
    }

    #[cfg(target_os = "linux")]
    {
        setup_linux_service(
            network_id,
            mode,
            as_exit_node,
            exit_node,
            nucleus_port,
            cluster_secret,
        )?;
    }

    #[cfg(target_os = "macos")]
    {
        setup_macos_service(
            network_id,
            mode,
            as_exit_node,
            exit_node,
            nucleus_port,
            cluster_secret,
        )?;
    }

    Ok(())
}

#[cfg(windows)]
fn build_mode_args(mode: RunMode, nucleus_port: u16, cluster_secret: Option<&str>) -> Vec<String> {
    let mut args = vec![
        "--mode".to_string(),
        match mode {
            RunMode::Edge => "edge".to_string(),
            RunMode::Nucleus => "nucleus".to_string(),
            RunMode::Dual => "dual".to_string(),
        },
    ];

    if mode == RunMode::Dual || mode == RunMode::Nucleus {
        args.push("--port".to_string());
        args.push(nucleus_port.to_string());
        if let Some(secret) = cluster_secret {
            args.push("--secret".to_string());
            args.push(secret.to_string());
        }
    }

    args
}

#[cfg(windows)]
async fn setup_windows_nucleus_service(port: u16, secret: &str) -> Result<()> {
    let exe_path = std::env::current_exe()?;

    let mut args = vec![
        "start".to_string(),
        "--mode".to_string(),
        "nucleus".to_string(),
        "--port".to_string(),
        port.to_string(),
    ];
    if !secret.is_empty() {
        args.push("--secret".to_string());
        args.push(secret.to_string());
    }
    args.push("--daemon".to_string());

    let bin_path_val = format!("\"{}\" {}", exe_path.display(), args.join(" "));
    setup_windows_service_common(&bin_path_val).await
}

#[cfg(windows)]
async fn setup_windows_service(
    _base_url: &str,
    network_id: &str,
    mode: RunMode,
    as_exit_node: bool,
    exit_node: Option<&str>,
    nucleus_port: u16,
    cluster_secret: Option<&str>,
) -> Result<()> {
    let exe_path = std::env::current_exe()?;

    let mut args = vec![
        "start".to_string(),
        "-n".to_string(),
        network_id.to_string(),
    ];

    args.extend(build_mode_args(mode, nucleus_port, cluster_secret));

    if as_exit_node {
        args.push("--as-exit-node".to_string());
    }
    if let Some(ip) = exit_node {
        args.push("--exit-node".to_string());
        args.push(ip.to_string());
    }
    args.push("--daemon".to_string());

    let bin_path_val = format!("\"{}\" {}", exe_path.display(), args.join(" "));
    setup_windows_service_common(&bin_path_val).await
}

#[cfg(windows)]
async fn setup_windows_service_common(bin_path_val: &str) -> Result<()> {
    use std::process::Command;

    let current_pid = std::process::id();
    let kill_cmd = format!(
        "Get-Process {} -ErrorAction SilentlyContinue | Where-Object {{ $_.Id -ne {} }} | Stop-Process -Force",
        "omniedge", current_pid
    );
    let _ = Command::new("powershell")
        .args(["-Command", &kill_cmd])
        .output();
    let _ = Command::new("sc").args(["stop", SERVICE_NAME]).output();

    let mut retries = 10;
    while retries > 0 {
        let create_cmd = format!(
            "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; if (!(Get-Service {} -ErrorAction SilentlyContinue)) {{ New-Service -Name {} -BinaryPathName '{}' -DisplayName 'OmniEdge Network Service' -Description 'OmniEdge zero-config Mesh VPN' -StartupType Automatic }} else {{ sc.exe config {} binPath= '{}' }}",
            SERVICE_NAME, SERVICE_NAME, bin_path_val, SERVICE_NAME, bin_path_val
        );

        let output = Command::new("powershell")
            .args(["-Command", &create_cmd])
            .output()?;

        if output.status.success() {
            break;
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
            let stdout = String::from_utf8_lossy(&output.stdout).to_lowercase();
            let combined_err = format!("{}{}", stdout, stderr);

            if combined_err.contains("1072") || combined_err.contains("marked for deletion") {
                log::warn!(
                    "Service is marked for deletion, retrying in 2 seconds... (Attempts left: {})",
                    retries - 1
                );
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                retries -= 1;
                if retries == 0 {
                    return Err(anyhow::anyhow!(
                        "Failed to configure service: Service is stuck in 'Marked for Deletion' state."
                    ));
                }
            } else {
                return Err(anyhow::anyhow!(
                    "Failed to configure service via PowerShell: {}",
                    combined_err.trim()
                ));
            }
        }
    }

    let output = Command::new("sc").args(["start", SERVICE_NAME]).output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let combined = format!("{}{}", stdout, stderr);

        if !combined.contains("already running") && !combined.contains("1056") {
            return Err(anyhow::anyhow!(
                "Failed to start windows service: {}",
                combined.trim()
            ));
        }
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn setup_linux_nucleus_service(port: u16, secret: &str) -> Result<()> {
    use std::fs;
    use std::process::Command;

    // Stop any existing daemon processes before setting up new service
    if is_omniedge_daemon_running() {
        info!("Existing OmniEdge daemon detected. Stopping it first...");
        let _ = Command::new("pkill")
            .args(["-f", "omniedge.*--daemon"])
            .output();
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    let exe_path = std::env::current_exe()?;
    let secret_flag = if secret.is_empty() {
        "".to_string()
    } else {
        format!("--secret {}", secret)
    };

    let service_content = format!(
        r#"[Unit]
Description=OmniEdge Nucleus Signaling Server
After=network.target

[Service]
ExecStart={} start --mode nucleus --port {} {} --daemon
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
"#,
        exe_path.display(),
        port,
        secret_flag
    );

    fs::write("/tmp/omniedge.service", &service_content)?;

    let _ = Command::new("sudo")
        .args([
            "cp",
            "/tmp/omniedge.service",
            "/etc/systemd/system/omniedge.service",
        ])
        .output();
    let _ = Command::new("sudo")
        .args(["systemctl", "daemon-reload"])
        .output();
    let _ = Command::new("sudo")
        .args(["systemctl", "enable", "omniedge"])
        .output();
    let _ = Command::new("sudo")
        .args(["systemctl", "start", "omniedge"])
        .output();

    Ok(())
}

#[cfg(target_os = "linux")]
fn setup_linux_service(
    network_id: &str,
    mode: RunMode,
    as_exit_node: bool,
    exit_node: Option<&str>,
    nucleus_port: u16,
    cluster_secret: Option<&str>,
) -> Result<()> {
    use std::fs;
    use std::process::Command;

    // Stop any existing daemon processes before setting up new service
    if is_omniedge_daemon_running() {
        info!("Existing OmniEdge daemon detected. Stopping it first...");
        let _ = Command::new("pkill")
            .args(["-f", "omniedge.*--daemon"])
            .output();
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    let exe_path = std::env::current_exe()?;

    let mode_str = match mode {
        RunMode::Edge => "edge",
        RunMode::Nucleus => "nucleus",
        RunMode::Dual => "dual",
    };

    let as_exit_flag = if as_exit_node { "--as-exit-node" } else { "" };
    let exit_node_flag = if let Some(ip) = exit_node {
        format!("--exit-node {}", ip)
    } else {
        "".to_string()
    };

    let nucleus_flags = if mode == RunMode::Dual {
        let secret_flag = cluster_secret
            .map(|s| format!("--secret {}", s))
            .unwrap_or_default();
        format!("--port {} {}", nucleus_port, secret_flag)
    } else {
        "".to_string()
    };

    let service_content = format!(
        r#"[Unit]
Description=OmniEdge Service
After=network.target

[Service]
ExecStart={} start -n {} --mode {} {} {} {} --daemon
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
"#,
        exe_path.display(),
        network_id,
        mode_str,
        nucleus_flags,
        as_exit_flag,
        exit_node_flag
    );

    fs::write("/tmp/omniedge.service", &service_content)?;

    let _ = Command::new("sudo")
        .args([
            "cp",
            "/tmp/omniedge.service",
            "/etc/systemd/system/omniedge.service",
        ])
        .output();
    let _ = Command::new("sudo")
        .args(["systemctl", "daemon-reload"])
        .output();
    let _ = Command::new("sudo")
        .args(["systemctl", "enable", "omniedge"])
        .output();
    let _ = Command::new("sudo")
        .args(["systemctl", "start", "omniedge"])
        .output();

    Ok(())
}

#[cfg(target_os = "macos")]
fn setup_macos_nucleus_service(port: u16, secret: &str) -> Result<()> {
    use std::fs;
    use std::process::Command;

    // Check if a daemon is already running and stop it first
    if is_omniedge_daemon_running() {
        info!("Existing OmniEdge daemon detected. Stopping it first...");
        let _ = Command::new("pkill")
            .args(["-f", "omniedge.*--daemon"])
            .output();
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    let exe_path = std::env::current_exe()?;
    let home_dir = dirs::home_dir().context("Failed to get home directory")?;

    // Use system LaunchDaemon for root privileges
    let plist_path = std::path::PathBuf::from("/Library/LaunchDaemons/io.omniedge.daemon.plist");

    let mut program_args = vec![
        format!("<string>{}</string>", exe_path.display()),
        "<string>start</string>".to_string(),
        "<string>--mode</string>".to_string(),
        "<string>nucleus</string>".to_string(),
        "<string>--port</string>".to_string(),
        format!("<string>{}</string>", port),
    ];
    if !secret.is_empty() {
        program_args.push("<string>--secret</string>".to_string());
        program_args.push(format!("<string>{}</string>", secret));
    }
    program_args.push("<string>--daemon</string>".to_string());

    let plist_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>io.omniedge.daemon</string>
    <key>ProgramArguments</key>
    <array>
        {}
    </array>
    <key>RunAtLoad</key>
    <false/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/var/log/omniedge.log</string>
    <key>StandardErrorPath</key>
    <string>/var/log/omniedge.error.log</string>
    <key>EnvironmentVariables</key>
    <dict>
        <key>HOME</key>
        <string>{}</string>
    </dict>
</dict>
</plist>
"#,
        program_args.join("\n        "),
        home_dir.display()
    );

    // Write to temp file first
    let temp_plist = "/tmp/io.omniedge.daemon.plist";
    fs::write(temp_plist, &plist_content)?;

    // Unload existing service if any
    let _ = Command::new("sudo")
        .args(["launchctl", "unload", &plist_path.to_string_lossy()])
        .output();

    // Copy plist to LaunchDaemons (requires sudo)
    let copy_output = Command::new("sudo")
        .args(["cp", temp_plist, &plist_path.to_string_lossy()])
        .output()?;

    if !copy_output.status.success() {
        let stderr = String::from_utf8_lossy(&copy_output.stderr);
        return Err(anyhow::anyhow!(
            "Failed to install service (sudo required): {}",
            stderr
        ));
    }

    // Set correct ownership
    let _ = Command::new("sudo")
        .args(["chown", "root:wheel", &plist_path.to_string_lossy()])
        .output();

    // Load the service
    let output = Command::new("sudo")
        .args(["launchctl", "load", &plist_path.to_string_lossy()])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "Failed to load launchd service: {}",
            stderr
        ));
    }

    // Clean up temp file
    let _ = fs::remove_file(temp_plist);

    Ok(())
}

#[cfg(target_os = "macos")]
fn setup_macos_service(
    network_id: &str,
    mode: RunMode,
    as_exit_node: bool,
    exit_node: Option<&str>,
    nucleus_port: u16,
    cluster_secret: Option<&str>,
) -> Result<()> {
    use std::process::Command;

    // Check if a daemon is already running and stop it first
    if is_omniedge_daemon_running() {
        info!("Existing OmniEdge daemon detected. Stopping it first...");
        // Kill existing daemon processes
        let _ = Command::new("pkill")
            .args(["-f", "omniedge.*--daemon"])
            .output();
        // Give it a moment to clean up
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    let exe_path = std::env::current_exe()?;

    let mode_str = match mode {
        RunMode::Edge => "edge",
        RunMode::Nucleus => "nucleus",
        RunMode::Dual => "dual",
    };

    // Build command arguments
    let mut args = vec![
        "start".to_string(),
        "-n".to_string(),
        network_id.to_string(),
        "--mode".to_string(),
        mode_str.to_string(),
    ];

    if mode == RunMode::Dual {
        args.push("--port".to_string());
        args.push(nucleus_port.to_string());
        if let Some(secret) = cluster_secret {
            args.push("--secret".to_string());
            args.push(secret.to_string());
        }
    }

    if as_exit_node {
        args.push("--as-exit-node".to_string());
    }
    if let Some(ip) = exit_node {
        args.push("--exit-node".to_string());
        args.push(ip.to_string());
    }
    args.push("--daemon".to_string());

    info!(
        "Starting OmniEdge daemon process: {} {:?}",
        exe_path.display(),
        args
    );

    // Fork the daemon process to background
    // We're already running as root (checked in main.rs), so just spawn directly
    let child = Command::new(&exe_path)
        .args(&args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();

    match child {
        Ok(child) => {
            info!("OmniEdge daemon started with PID: {}", child.id());
            // Give the daemon a moment to start
            std::thread::sleep(std::time::Duration::from_millis(500));
            Ok(())
        }
        Err(e) => Err(anyhow::anyhow!("Failed to start daemon process: {}", e)),
    }
}

pub async fn stop_and_cleanup_service(base_url: &str) -> Result<()> {
    // First try to stop via helper if available
    if is_helper_available().await {
        info!("Stopping VPN via helper service...");
        let req = HelperRequest {
            command: "stop_vpn".to_string(),
            args: serde_json::json!({}),
        };
        let _ = call_helper(&req).await;
    }

    #[cfg(windows)]
    {
        use std::process::Command;

        // Stop and delete Windows service if it exists
        let _ = Command::new("sc").args(["stop", SERVICE_NAME]).output();
        let _ = Command::new("sc").args(["delete", SERVICE_NAME]).output();

        // Kill any running omniedge daemon processes
        // This is needed if the daemon was started manually (not via Windows service)
        let kill_cmd = format!(
            "Get-Process -Name 'omniedge' -ErrorAction SilentlyContinue | Where-Object {{ $_.Id -ne {} }} | Stop-Process -Force",
            std::process::id()
        );
        let _ = Command::new("powershell")
            .args(["-NoProfile", "-Command", &kill_cmd])
            .output();

        // Give the process time to terminate
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    #[cfg(target_os = "linux")]
    {
        use std::process::Command;

        // Stop and disable systemd service if it exists
        let _ = Command::new("systemctl")
            .args(["stop", "omniedge"])
            .output();
        let _ = Command::new("systemctl")
            .args(["disable", "omniedge"])
            .output();

        // Kill any running omniedge daemon processes
        // This is needed if the daemon was started manually (not via systemd)
        let _ = Command::new("pkill")
            .args(["-f", "omniedge.*--daemon"])
            .output();

        // Give the process time to terminate
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        // Stop and remove system LaunchDaemon (new location)
        let daemon_plist = "/Library/LaunchDaemons/io.omniedge.daemon.plist";
        let _ = Command::new("launchctl")
            .args(["unload", daemon_plist])
            .output();
        let _ = Command::new("rm").args(["-f", daemon_plist]).output();

        // Also clean up old user LaunchAgent if it exists
        if let Some(home_dir) = dirs::home_dir() {
            let agent_plist = home_dir.join("Library/LaunchAgents/io.omniedge.cli.plist");
            let _ = Command::new("launchctl")
                .args(["unload", &agent_plist.to_string_lossy()])
                .output();
            let _ = std::fs::remove_file(&agent_plist);
        }

        // Kill any running omniedge daemon processes
        // This is needed if the daemon was started manually (not via LaunchDaemon)
        let _ = Command::new("pkill")
            .args(["-f", "omniedge.*--daemon"])
            .output();

        // Give the process time to terminate
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    let config = CliConfig::load().unwrap_or_default();
    let identity_pk = config
        .identity_private_key
        .as_ref()
        .and_then(|k| hex::decode(k).ok())
        .and_then(|b| b.try_into().ok());
    let manager = ConnectionManager::new(base_url.to_string(), identity_pk);
    manager.cleanup_adapters()?;
    Ok(())
}
