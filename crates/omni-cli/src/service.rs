use crate::utils::get_hardware_id;
use crate::SERVICE_NAME;
use anyhow::{Context, Result};
use log::info;
use omni_core::{CliConfig, ConnectionManager};
use serde::{Deserialize, Serialize};

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
        let pipe_name = r"\\.\pipe\omniedge_helper";

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
        let socket_path = "/tmp/omniedge_helper.sock";

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

/// Start VPN through helper service
async fn start_via_helper(
    token: &str,
    network_id: &str,
    device_id: &str,
    hardware_id: &str,
    nucleus: bool,
    as_exit_node: bool,
    exit_node_ip: Option<String>,
) -> Result<()> {
    let req = HelperRequest {
        command: "start_vpn".to_string(),
        args: serde_json::json!({
            "token": token,
            "network_id": network_id,
            "device_id": device_id,
            "hardware_id": hardware_id,
            "nucleus": nucleus,
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

pub async fn run_worker(
    base_url: &str,
    network_id: &str,
    nucleus: bool,
    as_exit_node: bool,
    exit_node: Option<String>,
) -> Result<()> {
    log::info!(
        "Starting OmniEdge background worker for network: {} (API: {})",
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
    info!("Connecting with token for network: {}...", network_id);
    manager
        .connect_with_token(
            auth.token,
            network_id,
            &device_id,
            &get_hardware_id().unwrap_or_else(|_| "unknown".to_string()),
            nucleus,
            as_exit_node,
            exit_node,
        )
        .await?;

    info!("Worker connected successfully. Waiting for SIGINT...");

    tokio::signal::ctrl_c().await?;
    manager.disconnect().await?;
    Ok(())
}

pub async fn setup_and_start_service(
    base_url: &str,
    network_id: &str,
    nucleus: bool,
    as_exit_node: bool,
    exit_node: Option<&str>,
) -> Result<()> {
    // First, try to use existing omni-helper service if available
    if is_helper_available().await {
        info!("Found running omni-helper service, using it for VPN connection...");
        let config = CliConfig::load().context("Failed to load config")?;
        let auth = config.auth_response.context("Not authenticated")?;
        let device_id = config.device_uuid.context("Device not registered")?;
        let hardware_id = get_hardware_id().unwrap_or_else(|_| "unknown".to_string());

        match start_via_helper(
            &auth.token,
            network_id,
            &device_id,
            &hardware_id,
            nucleus,
            as_exit_node,
            exit_node.map(|s| s.to_string()),
        )
        .await
        {
            Ok(_) => {
                println!("VPN started via background helper service.");
                return Ok(());
            }
            Err(e) => {
                log::warn!(
                    "Failed to start via helper: {}. Falling back to standalone service.",
                    e
                );
            }
        }
    }

    // Fall back to creating a standalone background service
    info!("Starting standalone background service...");

    #[cfg(windows)]
    {
        setup_windows_service(base_url, network_id, nucleus, as_exit_node, exit_node).await?;
    }

    #[cfg(target_os = "linux")]
    {
        setup_linux_service(network_id, nucleus, as_exit_node, exit_node)?;
    }

    #[cfg(target_os = "macos")]
    {
        setup_macos_service(network_id, nucleus, as_exit_node, exit_node)?;
    }

    Ok(())
}

#[cfg(windows)]
async fn setup_windows_service(
    _base_url: &str,
    network_id: &str,
    nucleus: bool,
    as_exit_node: bool,
    exit_node: Option<&str>,
) -> Result<()> {
    use std::process::Command;
    let exe_path = std::env::current_exe()?;

    let mut args = vec![
        "start".to_string(),
        "-n".to_string(),
        network_id.to_string(),
    ];
    if nucleus {
        args.push("--nucleus".to_string());
    }
    if as_exit_node {
        args.push("--as-exit-node".to_string());
    }
    if let Some(ip) = exit_node {
        args.push("--exit-node".to_string());
        args.push(ip.to_string());
    }
    args.push("--daemon".to_string());

    let bin_path_val = format!("\"{}\" {}", exe_path.display(), args.join(" "));

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
fn setup_linux_service(
    network_id: &str,
    nucleus: bool,
    as_exit_node: bool,
    exit_node: Option<&str>,
) -> Result<()> {
    use std::fs;
    use std::process::Command;

    let exe_path = std::env::current_exe()?;
    let n_flag = if nucleus { "--nucleus" } else { "" };
    let as_exit_flag = if as_exit_node { "--as-exit-node" } else { "" };
    let exit_node_flag = if let Some(ip) = exit_node {
        format!("--exit-node {}", ip)
    } else {
        "".to_string()
    };

    let service_content = format!(
        r#"[Unit]
Description=OmniEdge Service
After=network.target

[Service]
ExecStart={} start -n {} {} {} {} --daemon
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
"#,
        exe_path.display(),
        network_id,
        n_flag,
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
fn setup_macos_service(
    network_id: &str,
    nucleus: bool,
    as_exit_node: bool,
    exit_node: Option<&str>,
) -> Result<()> {
    use std::fs;
    use std::process::Command;

    let exe_path = std::env::current_exe()?;
    let home_dir = dirs::home_dir().context("Failed to get home directory")?;
    let launch_agents_dir = home_dir.join("Library/LaunchAgents");
    let plist_path = launch_agents_dir.join("io.omniedge.cli.plist");

    // Ensure LaunchAgents directory exists
    fs::create_dir_all(&launch_agents_dir)?;

    // Build program arguments
    let mut program_args = vec![
        format!("<string>{}</string>", exe_path.display()),
        "<string>start</string>".to_string(),
        "<string>-n</string>".to_string(),
        format!("<string>{}</string>", network_id),
    ];

    if nucleus {
        program_args.push("<string>--nucleus</string>".to_string());
    }
    if as_exit_node {
        program_args.push("<string>--as-exit-node</string>".to_string());
    }
    if let Some(ip) = exit_node {
        program_args.push("<string>--exit-node</string>".to_string());
        program_args.push(format!("<string>{}</string>", ip));
    }
    program_args.push("<string>--daemon</string>".to_string());

    let plist_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>io.omniedge.cli</string>
    <key>ProgramArguments</key>
    <array>
        {}
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{}/Library/Logs/omniedge.log</string>
    <key>StandardErrorPath</key>
    <string>{}/Library/Logs/omniedge.error.log</string>
</dict>
</plist>
"#,
        program_args.join("\n        "),
        home_dir.display(),
        home_dir.display()
    );

    // Stop existing service if running
    let _ = Command::new("launchctl")
        .args(["unload", &plist_path.to_string_lossy()])
        .output();

    // Write plist file
    fs::write(&plist_path, plist_content)?;

    // Load and start the service
    let output = Command::new("launchctl")
        .args(["load", &plist_path.to_string_lossy()])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "Failed to load launchd service: {}",
            stderr
        ));
    }

    Ok(())
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
        let _ = Command::new("sc").args(["stop", SERVICE_NAME]).output();
        let _ = Command::new("sc").args(["delete", SERVICE_NAME]).output();
    }

    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        let _ = Command::new("sudo")
            .args(["systemctl", "stop", "omniedge"])
            .output();
        let _ = Command::new("sudo")
            .args(["systemctl", "disable", "omniedge"])
            .output();
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        if let Some(home_dir) = dirs::home_dir() {
            let plist_path = home_dir.join("Library/LaunchAgents/io.omniedge.cli.plist");
            let _ = Command::new("launchctl")
                .args(["unload", &plist_path.to_string_lossy()])
                .output();
            let _ = std::fs::remove_file(&plist_path);
        }
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
