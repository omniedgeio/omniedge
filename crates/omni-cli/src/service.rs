use crate::utils::get_hardware_id;
use crate::SERVICE_NAME;
use anyhow::{Context, Result};
use log::info;
use omni_core::{CliConfig, ConnectionManager};

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
    #[cfg(windows)]
    {
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

        // Use standard quotes for paths with potential spaces.
        // We will wrap this in single quotes for the PowerShell command string.
        let bin_path_val = format!("\"{}\" {}", exe_path.display(), args.join(" "));

        let current_pid = std::process::id();
        let kill_cmd = format!("Get-Process {} -ErrorAction SilentlyContinue | Where-Object {{ $_.Id -ne {} }} | Stop-Process -Force", "omniedge", current_pid);
        let _ = Command::new("powershell")
            .args(["-Command", &kill_cmd])
            .output();
        let _ = Command::new("sc").args(["stop", SERVICE_NAME]).output();

        let mut retries = 10;
        while retries > 0 {
            // Use PowerShell for more robust service creation and configuration.
            // Force UTF8 encoding for predictable parsing across different locale settings.
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
                    log::warn!("Service is marked for deletion, retrying in 2 seconds... (Attempts left: {})", retries - 1);
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                    retries -= 1;
                    if retries == 0 {
                        return Err(anyhow::anyhow!("Failed to configure service: Service is stuck in 'Marked for Deletion' state. Please close any 'Services' windows (services.msc) or Task Manager and try again. If the problem persists, a system restart may be required."));
                    }
                } else {
                    return Err(anyhow::anyhow!(
                        "Failed to configure service via PowerShell: {}",
                        combined_err.trim()
                    ));
                }
            }
        }

        // Pass base_url as env var to service
        if base_url != crate::DEFAULT_BASE_URL {
            // Note: Standard 'sc' doesn't easily set env vars, but our main.rs will pick it up
            // if we provide it as a flag or if we had a proper installer.
            // For now, let's just log it. The user might need to set it globally.
        }

        let output = Command::new("sc").args(["start", SERVICE_NAME]).output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let combined = format!("{}{}", stdout, stderr);

            if !combined.contains("already running") && !combined.contains("1056") {
                return Err(anyhow::anyhow!("Failed to start windows service: {}. (Note: Ensure no other instances are running and you are elevated)", combined.trim()));
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
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
            "[Unit]\nDescription=OmniEdge Service\nAfter=network.target\n\n[Service]\nExecStart={} start -n {} {} {} {} --daemon\nRestart=always\n\n[Install]\nWantedBy=multi-user.target\n",
            exe_path.display(), network_id, n_flag, as_exit_flag, exit_node_flag
        );

        fs::write("/tmp/omniedge.service", service_content)?;
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
    }

    Ok(())
}

pub async fn stop_and_cleanup_service(base_url: &str) -> Result<()> {
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
