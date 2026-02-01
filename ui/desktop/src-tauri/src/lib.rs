use log::{error, info};
use omni_api::types::{
    AuthResp, DeviceCodeResp, DeviceResponse, ProfileResponse, SessionResponse,
    VirtualNetworkDeviceResponse, VirtualNetworkResponse,
};
use omni_core::{CliConfig, ConnectionManager, ConnectionState};
use omni_plugin::{PluginConfig, PluginManager};
use std::sync::Arc;
#[cfg(target_os = "macos")]
use tauri::menu::{Menu, MenuItem};
use tauri::{
    image::Image,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, Runtime,
};
use tokio::sync::Mutex;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
#[cfg(windows)]
use tokio::net::windows::named_pipe::ClientOptions;
#[cfg(unix)]
use tokio::net::UnixStream;

use omni_helper::{HelperRequest, HelperResponse, StartArgs};

struct AppState {
    manager: Arc<Mutex<ConnectionManager>>,
    plugin_manager: Arc<Mutex<PluginManager>>,
}

async fn call_helper(req: &HelperRequest) -> Result<HelperResponse, String> {
    use tokio::time::{timeout, Duration};
    let req_bytes = serde_json::to_vec(req).map_err(|e| e.to_string())?;

    let call_future = async {
        let mut buf = [0; 4096];
        #[cfg(unix)]
        {
            let socket_path = "/var/run/omniedge-helper.sock";
            let mut stream = UnixStream::connect(socket_path)
                .await
                .map_err(|e| e.to_string())?;
            stream
                .write_all(&req_bytes)
                .await
                .map_err(|e| e.to_string())?;
            let n = stream.read(&mut buf).await.map_err(|e| e.to_string())?;
            serde_json::from_slice(&buf[..n]).map_err(|e| e.to_string())
        }
        #[cfg(windows)]
        {
            let pipe_name = r"\\.\pipe\omniedge-helper";
            let mut client = ClientOptions::new()
                .open(pipe_name)
                .map_err(|e| e.to_string())?;
            client
                .write_all(&req_bytes)
                .await
                .map_err(|e| e.to_string())?;
            let n = client.read(&mut buf).await.map_err(|e| e.to_string())?;
            serde_json::from_slice(&buf[..n]).map_err(|e| e.to_string())
        }
    };

    match timeout(Duration::from_secs(10), call_future).await {
        Ok(res) => res,
        Err(_) => {
            error!("Helper request timed out for command: {}", req.command);
            Err("Helper request timed out".to_string())
        }
    }
}

#[tauri::command]
async fn try_auto_login(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let mut manager = state.manager.lock().await;
    manager.try_auto_login().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn login_pwd(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    email: String,
    password: String,
) -> Result<(), String> {
    let mut manager = state.manager.lock().await;
    let auth = manager
        .login_with_password(&email, &password)
        .await
        .map_err(|e| e.to_string())?;

    // Save to config after successful login
    if let Ok(mut config) = CliConfig::load() {
        config.auth_response = Some(auth);
        let _ = config.save();
    }
    update_tray_icon(&app, manager.get_state().await);
    Ok(())
}

#[tauri::command]
async fn install_helper(_app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;

        // Find the sidecar path
        let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
        let exe_dir = exe_path.parent().ok_or("Could not find exe directory")?;

        let sidecar_name = format!(
            "omni-helper-{}.exe",
            if cfg!(target_arch = "x86_64") {
                "x86_64-pc-windows-msvc"
            } else {
                "i686-pc-windows-msvc"
            }
        );
        let tried_paths = vec![
            exe_dir.join("omni-helper.exe"),
            exe_dir.join(&sidecar_name),
            exe_dir.join("binaries").join("omni-helper.exe"),
            exe_dir.join("binaries").join(&sidecar_name),
            exe_path.parent().unwrap().join("omni-helper.exe"),
            exe_path.parent().unwrap().join(&sidecar_name),
        ];

        let helper_path = tried_paths.iter().find(|p| p.exists())
            .ok_or_else(|| {
                let tried_str = tried_paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join("\n");
                format!(
                    "OmniEdge Helper binary not found.\nTried:\n{}\n\nPlease ensure the background service is installed or run the setup again.",
                    tried_str
                )
            })?;

        // We use sc.exe to create the service because the helper binary doesn't self-install.
        // Robust quoting for PowerShell Start-Process with Verb RunAs.
        let helper_str = helper_path.to_str().unwrap();

        // Using PowerShell native New-Service which handles quoting reliably
        let ps_commands = format!(
            "$ErrorActionPreference = 'SilentlyContinue'; \
             sc.exe stop OmniEdgeHelper; \
             Start-Sleep -s 1; \
             sc.exe delete OmniEdgeHelper; \
             $bin = [char]34 + '{}' + [char]34; \
             New-Service -Name OmniEdgeHelper -BinaryPathName $bin -DisplayName 'OmniEdge Helper Service' -StartupType Automatic; \
             sc.exe start OmniEdgeHelper",
            helper_str
        );

        let elevation_cmd = format!(
            "Start-Process powershell -Verb RunAs -Wait -ArgumentList '-NoProfile', '-Command', \"{}\"",
            ps_commands.replace("\"", "\\\"")
        );

        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command", &elevation_cmd])
            .output()
            .map_err(|e| e.to_string())?;

        if !output.status.success() {
            return Err("Failed to elevate and install service. Please ensure you clicked 'Yes' on the UAC prompt.".to_string());
        }

        Ok(())
    }
    #[cfg(target_os = "linux")]
    {
        use std::fs;
        use std::process::Command;

        let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
        let exe_dir = exe_path.parent().ok_or("Could not find exe directory")?;
        let helper_path = exe_dir.join("omni-helper"); // Simplified for linux

        if !helper_path.exists() {
            return Err("Helper binary not found".to_string());
        }

        let service_content = format!(
            "[Unit]\nDescription=OmniEdge Helper Service\nAfter=network.target\n\n[Service]\nExecStart={}\nRestart=always\n\n[Install]\nWantedBy=multi-user.target\n",
            helper_path.display()
        );

        fs::write("/tmp/omniedge-helper.service", service_content).map_err(|e| e.to_string())?;

        // Stop existing service first to prevent duplicates
        let _ = Command::new("sudo")
            .args(["systemctl", "stop", "omniedge-helper"])
            .output();

        let _ = Command::new("sudo")
            .args([
                "cp",
                "/tmp/omniedge-helper.service",
                "/etc/systemd/system/omniedge-helper.service",
            ])
            .output();
        let _ = Command::new("sudo")
            .args(["systemctl", "daemon-reload"])
            .output();
        let _ = Command::new("sudo")
            .args(["systemctl", "enable", "omniedge-helper"])
            .output();
        let _ = Command::new("sudo")
            .args(["systemctl", "start", "omniedge-helper"])
            .output();
        Ok(())
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        // macOS: Use osascript to prompt for admin password and install helper as LaunchDaemon
        use std::process::Command;

        let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
        let exe_dir = exe_path.parent().ok_or("Could not find exe directory")?;

        // Try to find the helper binary in various locations
        let arch = if cfg!(target_arch = "aarch64") {
            "aarch64-apple-darwin"
        } else {
            "x86_64-apple-darwin"
        };
        let sidecar_name = format!("omni-helper-{}", arch);

        let tried_paths = vec![
            exe_dir.join("omni-helper"),
            exe_dir.join(&sidecar_name),
            exe_dir.join("../MacOS/omni-helper"),
            exe_dir.join(format!("../MacOS/{}", &sidecar_name)),
            exe_dir.join("../Resources/binaries/omni-helper"),
            exe_dir.join(format!("../Resources/binaries/{}", &sidecar_name)),
        ];

        let helper_path = tried_paths.iter().find(|p| p.exists())
            .ok_or_else(|| {
                let tried_str = tried_paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join("\n");
                format!(
                    "OmniEdge Helper binary not found.\nTried:\n{}\n\nPlease reinstall the application.",
                    tried_str
                )
            })?;

        let helper_str = helper_path.to_str().ok_or("Invalid helper path")?;
        let install_path = "/Library/PrivilegedHelperTools/io.omniedge.helper";
        let plist_path = "/Library/LaunchDaemons/io.omniedge.helper.plist";
        let socket_path = "/var/run/omniedge-helper.sock";

        // Create LaunchDaemon plist content
        let plist_content = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>io.omniedge.helper</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/var/log/omniedge-helper.log</string>
    <key>StandardErrorPath</key>
    <string>/var/log/omniedge-helper.error.log</string>
</dict>
</plist>"#,
            install_path
        );

        // Write plist to temp file
        let temp_plist = "/tmp/io.omniedge.helper.plist";
        std::fs::write(temp_plist, &plist_content).map_err(|e| e.to_string())?;

        // Create a shell script file to avoid escaping issues with osascript
        let install_script_path = "/tmp/omniedge-install-helper.sh";
        let install_script_content = format!(
            r#"#!/bin/bash
set -e

# First, kill any running helper process
pkill -9 -f io.omniedge.helper 2>/dev/null || true
killall omni-helper 2>/dev/null || true

# Stop and unload existing service (try both old and new plist names)
launchctl bootout system {plist} 2>/dev/null || true
launchctl unload {plist} 2>/dev/null || true
launchctl unload /Library/LaunchDaemons/io.omniedge.mac.Omniedge.HelperTool.plist 2>/dev/null || true

# Wait for process to terminate
sleep 1

# Force kill again if still running
pkill -9 -f io.omniedge.helper 2>/dev/null || true

# Remove old socket if exists
rm -f {socket}

# Create directory for helper
mkdir -p /Library/PrivilegedHelperTools

# Remove old binary first
rm -f {install}

# Copy helper binary
cp {helper} {install}
chmod 755 {install}
chown root:wheel {install}

# Install plist
cp {temp_plist} {plist}
chmod 644 {plist}
chown root:wheel {plist}

# Load the service using modern launchctl
launchctl bootstrap system {plist} 2>/dev/null || launchctl load {plist}

# Wait for socket to be created
for i in 1 2 3 4 5; do
    if [ -S {socket} ]; then
        echo "Helper socket created successfully"
        exit 0
    fi
    sleep 1
done

# Check if helper is running
if launchctl list | grep -q io.omniedge.helper; then
    echo "Helper service is running"
    exit 0
fi

echo "Helper installed but socket not yet available"
exit 0
"#,
            plist = plist_path,
            socket = socket_path,
            install = install_path,
            helper = helper_str,
            temp_plist = temp_plist,
        );

        std::fs::write(install_script_path, &install_script_content)
            .map_err(|e| format!("Failed to write install script: {}", e))?;

        // Make script executable (not strictly needed since we call it with bash)
        #[allow(unused)]
        let _ = Command::new("chmod")
            .args(["+x", install_script_path])
            .output();

        // Log debugging info
        eprintln!("[install_helper] Helper source path: {}", helper_str);
        eprintln!("[install_helper] Helper exists: {}", helper_path.exists());
        eprintln!("[install_helper] Temp plist written to: {}", temp_plist);
        eprintln!(
            "[install_helper] Install script written to: {}",
            install_script_path
        );

        // Use osascript to run the script with admin privileges
        // Running a script file avoids all the escaping issues
        let apple_script = format!(
            r#"do shell script "/bin/bash {}" with administrator privileges"#,
            install_script_path
        );

        eprintln!("[install_helper] Running osascript...");

        let output = Command::new("osascript")
            .args(["-e", &apple_script])
            .output()
            .map_err(|e| format!("Failed to run osascript: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        eprintln!("[install_helper] osascript exit status: {}", output.status);
        eprintln!("[install_helper] osascript stdout: {}", stdout);
        eprintln!("[install_helper] osascript stderr: {}", stderr);

        if !output.status.success() {
            // Clean up script file on failure
            let _ = std::fs::remove_file(install_script_path);

            if stderr.contains("User canceled") || stderr.contains("(-128)") {
                return Err("Installation cancelled by user.".to_string());
            }
            return Err(format!(
                "Failed to install helper service.\nExit code: {}\nstdout: {}\nstderr: {}",
                output.status, stdout, stderr
            ));
        }

        // Clean up temp files
        let _ = std::fs::remove_file(temp_plist);
        let _ = std::fs::remove_file(install_script_path);

        // Verify installation
        let helper_installed = std::path::Path::new(install_path).exists();
        let plist_installed = std::path::Path::new(plist_path).exists();
        let socket_exists = std::path::Path::new(socket_path).exists();

        eprintln!("[install_helper] After install - helper exists: {}, plist exists: {}, socket exists: {}", 
            helper_installed, plist_installed, socket_exists);

        if !helper_installed {
            return Err(format!("Helper binary was not copied to {}. The install script ran but the file is missing.", install_path));
        }

        Ok(())
    }
}

#[tauri::command]
async fn list_networks(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<VirtualNetworkResponse>, String> {
    let manager = state.manager.lock().await;
    manager.get_networks().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_devices(state: tauri::State<'_, AppState>) -> Result<Vec<DeviceResponse>, String> {
    let manager = state.manager.lock().await;
    manager.get_devices().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_network_devices(
    state: tauri::State<'_, AppState>,
    network_id: String,
) -> Result<Vec<VirtualNetworkDeviceResponse>, String> {
    let manager = state.manager.lock().await;
    manager
        .get_network_devices(&network_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_profile(state: tauri::State<'_, AppState>) -> Result<ProfileResponse, String> {
    let manager = state.manager.lock().await;
    manager
        .get_profile()
        .await
        .map_err(|e| format!("Profile error: {}", e))
}

#[tauri::command]
async fn get_device_id() -> Result<String, String> {
    Ok(get_hardware_id())
}

#[tauri::command]
async fn get_virtual_ip(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let req = HelperRequest {
        command: "get_virtual_ip".to_string(),
        args: serde_json::json!({}),
    };

    match call_helper(&req).await {
        Ok(resp) => {
            if resp.success {
                return Ok(resp
                    .data
                    .as_ref()
                    .and_then(|d| d.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_default());
            }
            Err(resp.message)
        }
        Err(_) => {
            // Fallback to local
            let manager = state.manager.lock().await;
            Ok(manager.get_virtual_ip().await)
        }
    }
}

#[tauri::command]
async fn check_helper() -> Result<bool, String> {
    // First check if helper responds to ping
    let ping_req = HelperRequest {
        command: "ping".to_string(),
        args: serde_json::json!({}),
    };

    // Retry up to 3 times to allow for service startup delay
    let mut ping_ok = false;
    for _ in 0..3 {
        if call_helper(&ping_req).await.is_ok() {
            ping_ok = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    if !ping_ok {
        return Ok(false);
    }

    // Now check if it's the correct (Rust v2) helper by sending version command
    // Old Go helper won't understand this command and won't return protocol field
    let version_req = HelperRequest {
        command: "version".to_string(),
        args: serde_json::json!({}),
    };

    match call_helper(&version_req).await {
        Ok(resp) => {
            if resp.success {
                if let Some(data) = resp.data {
                    // Check for rust-v2 protocol identifier
                    if let Some(protocol) = data.get("protocol").and_then(|p| p.as_str()) {
                        if protocol == "rust-v2" {
                            return Ok(true);
                        }
                    }
                }
            }
            // Helper responded but it's not the correct version
            info!("Helper responded but wrong version/protocol. Need to reinstall.");
            Ok(false)
        }
        Err(_) => {
            // Helper doesn't understand version command - it's the old Go helper
            info!("Helper doesn't support version command. Old helper detected.");
            Ok(false)
        }
    }
}

#[tauri::command]
async fn get_helper_version() -> Result<serde_json::Value, String> {
    let req = HelperRequest {
        command: "version".to_string(),
        args: serde_json::json!({}),
    };

    match call_helper(&req).await {
        Ok(resp) => {
            if resp.success {
                if let Some(data) = resp.data {
                    return Ok(data);
                }
            }
            Err(resp.message)
        }
        Err(e) => Err(e),
    }
}

#[tauri::command]
async fn check_is_admin() -> bool {
    #[cfg(target_os = "windows")]
    {
        use std::ptr;
        use winapi::um::processthreadsapi::{GetCurrentProcess, OpenProcessToken};
        use winapi::um::securitybaseapi::GetTokenInformation;
        use winapi::um::winnt::{TokenElevation, HANDLE, TOKEN_ELEVATION, TOKEN_QUERY};

        let mut handle: HANDLE = ptr::null_mut();
        unsafe {
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut handle) != 0 {
                let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
                let mut size = std::mem::size_of::<TOKEN_ELEVATION>() as u32;
                if GetTokenInformation(
                    handle,
                    TokenElevation,
                    &mut elevation as *mut _ as *mut _,
                    size,
                    &mut size,
                ) != 0
                {
                    return elevation.TokenIsElevated != 0;
                }
            }
        }
        false
    }
    #[cfg(not(target_os = "windows"))]
    {
        // On Unix-like systems, check if EUID is 0
        unsafe { ::libc::geteuid() == 0 }
    }
}

fn get_hardware_id() -> String {
    // Cross-platform machine ID using machineid_rs - same as CLI for consistency
    use machineid_rs::{Encryption, IdBuilder};
    use uuid::Uuid;

    let mut builder = IdBuilder::new(Encryption::SHA256);
    builder
        .add_component(machineid_rs::HWIDComponent::SystemID)
        .add_component(machineid_rs::HWIDComponent::CPUID)
        .add_component(machineid_rs::HWIDComponent::DriveSerial);

    if let Ok(id) = builder.build("omniedge") {
        // Map the hash to a stable UUID-like format
        if id.len() >= 32 {
            let hex_id = &id[0..32];
            if let Ok(bytes) = hex::decode(hex_id) {
                if let Ok(u) = Uuid::from_slice(&bytes) {
                    return u.to_string();
                }
            }
        }
        return id[..std::cmp::min(id.len(), 36)].to_string();
    }

    // Fallback to hostname-username if machineid fails
    let hostname = ::whoami::fallible::hostname().unwrap_or_else(|_| "unknown".to_string());
    let username = ::whoami::username();
    format!("{}-{}", hostname, username)
}

#[allow(unused_variables)]
fn ensure_wintun_dll(app: &tauri::AppHandle) {
    #[cfg(target_os = "windows")]
    {
        use std::env;
        use std::fs;

        // In Tauri 2, resources are often in the resource_dir
        if let Ok(resource_dir) = app.path().resource_dir() {
            let resource_path = resource_dir.join("resources").join("wintun.dll");
            if let Ok(exe_path) = env::current_exe() {
                if let Some(exe_dir) = exe_path.parent() {
                    let target_path = exe_dir.join("wintun.dll");
                    if resource_path.exists() && !target_path.exists() {
                        let _ = fs::copy(resource_path, target_path);
                    }
                }
            }
        }
    }
}

#[tauri::command]
async fn connect(
    app: tauri::AppHandle,
    _state: tauri::State<'_, AppState>,
    network_id: String,
    as_exit_node: Option<bool>,
    nucleus: Option<bool>,
    exit_node: Option<String>,
) -> Result<(), String> {
    ensure_wintun_dll(&app);
    let mut config = CliConfig::load().map_err(|e| e.to_string())?;
    if config.auth_response.is_none() {
        return Err("Not authenticated".to_string());
    }

    // Register device if not already registered
    if config.device_uuid.is_none() {
        // Registration is usually handled by the core/API, but let's ensure we have a name
        let device_name =
            ::whoami::fallible::hostname().unwrap_or_else(|_| "OmniEdge Device".to_string());
        config.device_name = Some(device_name);
        // device_uuid will be filled after join or we can explicitly register
    }

    let hardware_id = get_hardware_id();
    let device_id = config
        .device_uuid
        .clone()
        .unwrap_or_else(|| hardware_id.clone());

    let req = HelperRequest {
        command: "start_vpn".to_string(),
        args: serde_json::to_value(StartArgs {
            token: config
                .auth_response
                .as_ref()
                .unwrap()
                .effective_token()
                .to_string(),
            network_id: network_id.clone(),
            device_id: device_id.clone(),
            hardware_id: hardware_id.clone(),
            as_exit_node: as_exit_node.unwrap_or(false),
            nucleus: nucleus.unwrap_or(false),
            exit_node_ip: exit_node.clone(),
        })
        .map_err(|e| e.to_string())?,
    };

    match call_helper(&req).await {
        Ok(resp) => {
            if !resp.success {
                return Err(resp.message);
            }
        }
        Err(e) => {
            return Err(format!("Background helper not running. Please click 'Install Service' to enable background operations (requires one-time elevation). Error: {}", e));
        }
    }

    update_tray_icon(&app, ConnectionState::Connected);
    Ok(())
}

#[tauri::command]
async fn disconnect(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let req = HelperRequest {
        command: "stop_vpn".to_string(),
        args: serde_json::json!({}),
    };

    match call_helper(&req).await {
        Ok(resp) => {
            if !resp.success {
                return Err(resp.message);
            }
        }
        Err(_) => {
            // Fallback to local manager
            let mut manager = state.manager.lock().await;
            manager.disconnect().await.map_err(|e| e.to_string())?;
        }
    }
    update_tray_icon(&app, ConnectionState::Disconnected);
    Ok(())
}

#[tauri::command]
async fn quit(app: tauri::AppHandle, state: tauri::State<'_, AppState>) -> Result<(), String> {
    use tokio::time::{timeout, Duration};
    // Try to disconnect gracefully within 2 seconds
    let _ = timeout(Duration::from_secs(2), disconnect(app.clone(), state)).await;
    app.exit(0);
    Ok(())
}

#[tauri::command]
async fn open_browser(app: tauri::AppHandle, url: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url(url, None::<String>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn start_device_flow(state: tauri::State<'_, AppState>) -> Result<DeviceCodeResp, String> {
    let manager = state.manager.lock().await;
    manager.start_device_flow().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn poll_device_flow(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    device_code: String,
) -> Result<AuthResp, String> {
    let mut manager = state.manager.lock().await;
    let auth = manager
        .poll_device_flow(&device_code)
        .await
        .map_err(|e| e.to_string())?;

    if let Ok(mut config) = CliConfig::load() {
        config.auth_response = Some(auth.clone());
        let _ = config.save();
    }
    update_tray_icon(&app, manager.get_state().await);
    Ok(auth)
}

#[tauri::command]
async fn start_session_login(state: tauri::State<'_, AppState>) -> Result<SessionResponse, String> {
    let manager = state.manager.lock().await;
    manager
        .start_session_login()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn wait_for_session_login(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    session_id: String,
) -> Result<AuthResp, String> {
    // 1. Get base_url from manager with brief lock
    let base_url = {
        let manager = state.manager.lock().await;
        manager.get_base_url().to_string()
    };

    // 2. Wait for token without holding the lock (prevent UI blocking)
    let token_resp = ConnectionManager::wait_for_session_login(&base_url, &session_id)
        .await
        .map_err(|e| e.to_string())?;

    // 3. Acquire lock again to update manager state
    let mut manager = state.manager.lock().await;
    let auth = manager
        .handle_login_token(token_resp)
        .await
        .map_err(|e| e.to_string())?;

    if let Ok(mut config) = CliConfig::load() {
        config.auth_response = Some(auth.clone());
        let _ = config.save();
    }
    update_tray_icon(&app, manager.get_state().await);
    Ok(auth)
}

#[tauri::command]
async fn get_state(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<ConnectionState, String> {
    let req = HelperRequest {
        command: "status".to_string(),
        args: serde_json::json!({}),
    };

    let curr_state = match call_helper(&req).await {
        Ok(resp) => {
            if resp.success {
                if let Some(data) = resp.data {
                    if let Ok(st) = serde_json::from_value::<ConnectionState>(data["state"].clone())
                    {
                        st
                    } else {
                        ConnectionState::Disconnected
                    }
                } else {
                    ConnectionState::Disconnected
                }
            } else {
                ConnectionState::Disconnected
            }
        }
        Err(_) => {
            let manager = state.manager.lock().await;
            manager.get_state().await
        }
    };

    update_tray_icon(&app, curr_state.clone());
    Ok(curr_state)
}

#[tauri::command]
async fn set_exit_node(
    state: tauri::State<'_, AppState>,
    network_id: String,
    exit_node_id: String,
    exit_node_ip: String,
) -> Result<(), String> {
    let mut manager = state.manager.lock().await;
    manager
        .set_exit_node(
            &network_id,
            &exit_node_id,
            if exit_node_ip.is_empty() {
                None
            } else {
                Some(&exit_node_ip)
            },
        )
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_as_exit_node(state: tauri::State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let req = HelperRequest {
        command: "set_as_exit_node".to_string(),
        args: serde_json::json!({ "enabled": enabled }),
    };

    match call_helper(&req).await {
        Ok(resp) => {
            if !resp.success {
                return Err(resp.message);
            }
        }
        Err(_) => {
            // Fallback to local
            let mut manager = state.manager.lock().await;
            manager
                .set_as_exit_node(enabled)
                .await
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
async fn is_exit_node(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let req = HelperRequest {
        command: "is_exit_node".to_string(),
        args: serde_json::json!({}),
    };

    match call_helper(&req).await {
        Ok(resp) => {
            if resp.success {
                return Ok(resp.data.and_then(|d| d.as_bool()).unwrap_or(false));
            }
            Err(resp.message)
        }
        Err(_) => {
            // Fallback to local
            let manager = state.manager.lock().await;
            Ok(manager.is_exit_node())
        }
    }
}

#[tauri::command]
async fn get_debug_info(state: tauri::State<'_, AppState>) -> Result<serde_json::Value, String> {
    let mut info = serde_json::Map::new();

    // 1. Helper Status
    let req = HelperRequest {
        command: "status".to_string(),
        args: serde_json::json!({}),
    };

    if let Ok(resp) = call_helper(&req).await {
        info.insert("helper_active".to_string(), serde_json::json!(true));
        if let Some(data) = resp.data {
            info.insert("helper_state".to_string(), data);
        } else {
            info.insert(
                "helper_message".to_string(),
                serde_json::json!(resp.message),
            );
        }
    } else {
        info.insert("helper_active".to_string(), serde_json::json!(false));
    }

    // 2. Local State
    let manager = state.manager.lock().await;
    let local_state = manager.get_state().await;
    info.insert("local_state".to_string(), serde_json::json!(local_state));

    // 3. Logs
    #[cfg(windows)]
    let log_dir = std::env::var("PROGRAMDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("OmniEdge")
        .join("logs");
    #[cfg(not(windows))]
    let log_dir = std::path::PathBuf::from("/var/log/omniedge");

    if let Ok(entries) = std::fs::read_dir(&log_dir) {
        let mut log_files: Vec<_> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "log"))
            .collect();

        log_files.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());

        if let Some(latest) = log_files.last() {
            if let Ok(content) = std::fs::read_to_string(latest.path()) {
                let lines: Vec<&str> = content.lines().rev().take(50).collect();
                let mut reversed = lines;
                reversed.reverse();
                info.insert(
                    "helper_logs".to_string(),
                    serde_json::json!(reversed.join("\n")),
                );
                info.insert(
                    "log_file".to_string(),
                    serde_json::json!(latest.path().display().to_string()),
                );
            }
        }
    }

    Ok(serde_json::Value::Object(info))
}

fn update_tray_icon<R: Runtime>(app: &tauri::AppHandle<R>, state: ConnectionState) {
    if let Some(tray) = app.tray_by_id("main") {
        let icon_name = match state {
            ConnectionState::Connected => "trayicon_connected.png",
            _ => "trayicon_disconnected.png",
        };

        if let Ok(res_dir) = app.path().resource_dir() {
            let icon_path = res_dir.join("resources").join(icon_name);
            if let Ok(icon) = Image::from_path(icon_path) {
                let _ = tray.set_icon(Some(icon));
            }
        }

        /*
        // Trigger menu refresh on state change
        let handle = app.clone();
        tauri::async_runtime::spawn(async move {
            let state = handle.state::<AppState>();
            let manager = state.manager.lock().await;
            if let Ok(menu) = build_tray_menu(&handle, &manager).await {
                let _ = tray.set_menu(Some(menu));
            }
        });
        */
    }
}

#[tauri::command]
async fn open_logs(_app: tauri::AppHandle) -> Result<(), String> {
    let local_log_dir = CliConfig::config_path()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or("Invalid config path")?
        .join("logs");

    let _ = std::fs::create_dir_all(&local_log_dir);

    #[cfg(target_os = "windows")]
    let helper_log_dir = std::env::var("PROGRAMDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("OmniEdge")
        .join("logs");
    #[cfg(not(target_os = "windows"))]
    let helper_log_dir = std::path::PathBuf::from("/var/log/omniedge");

    let _ = std::fs::create_dir_all(&helper_log_dir);

    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer")
            .arg(local_log_dir)
            .spawn();
        let _ = std::process::Command::new("explorer")
            .arg(helper_log_dir)
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .arg(local_log_dir)
            .spawn();
        let _ = std::process::Command::new("open")
            .arg(helper_log_dir)
            .spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open")
            .arg(local_log_dir)
            .spawn();
        let _ = std::process::Command::new("xdg-open")
            .arg(helper_log_dir)
            .spawn();
    }
    Ok(())
}

#[tauri::command]
async fn resize_window(app: tauri::AppHandle, height: u32) -> Result<(), String> {
    use tauri::Manager;

    if let Some(window) = app.get_webview_window("main") {
        // Clamp height between min and max
        let clamped_height = height.max(200).min(700);

        // Get current scale factor for proper sizing
        let scale_factor = window.scale_factor().unwrap_or(1.0);

        // Get current window position before resizing
        let current_pos = window
            .outer_position()
            .unwrap_or(tauri::PhysicalPosition::new(0, 0));
        let old_size = window
            .outer_size()
            .unwrap_or(tauri::PhysicalSize::new(320, 480));

        // Set the window size using logical size (will be converted to physical)
        let logical_size = tauri::LogicalSize::new(320, clamped_height);
        window.set_size(logical_size).map_err(|e| e.to_string())?;

        // Get the new size after resize
        let new_size = window
            .outer_size()
            .unwrap_or(tauri::PhysicalSize::new(320, clamped_height));
        let height_diff = new_size.height as i32 - old_size.height as i32;

        // If window grew taller, move it up so content expands upward (tray app behavior)
        // This keeps the bottom of the window anchored near the taskbar
        if height_diff != 0 {
            if let Ok(Some(monitor)) = window.current_monitor() {
                let monitor_pos = monitor.position();
                let padding = (12.0 * scale_factor) as i32;

                // Calculate new Y position (move up by the height difference)
                let mut new_y = current_pos.y - height_diff;

                // Ensure window doesn't go above the screen top
                if new_y < monitor_pos.y + padding {
                    new_y = monitor_pos.y + padding;
                }

                let _ = window.set_position(tauri::PhysicalPosition::new(current_pos.x, new_y));
            }
        }

        info!(
            "Window resized to height: {} (scale: {}, height_diff: {})",
            clamped_height, scale_factor, height_diff
        );
    }
    Ok(())
}

// ============================================================================
// Plugin Management Commands
// ============================================================================

/// Plugin info for frontend (simplified from omni_plugin::PluginInfo)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PluginInfoUI {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub plugin_type: String,
    pub enabled: bool,
    pub status: String, // "active", "disabled", "error"
    pub error_message: Option<String>,
    pub permissions: Vec<String>,
}

#[tauri::command]
async fn list_plugins(state: tauri::State<'_, AppState>) -> Result<Vec<PluginInfoUI>, String> {
    let manager = state.plugin_manager.lock().await;
    let plugins = manager.list_plugins();

    Ok(plugins
        .into_iter()
        .map(|p| {
            let plugin_type = p
                .capabilities
                .first()
                .map(|c| format!("{:?}", c).to_lowercase())
                .unwrap_or_else(|| "event".to_string());

            let status = if p.error.is_some() {
                "error".to_string()
            } else if p.enabled {
                "active".to_string()
            } else {
                "disabled".to_string()
            };

            let permissions = p
                .capabilities
                .iter()
                .map(|c| format!("{:?}", c).to_lowercase())
                .collect();

            PluginInfoUI {
                id: p.id,
                name: p.name,
                version: p.version,
                author: p.author,
                description: p.description,
                plugin_type,
                enabled: p.enabled,
                status,
                error_message: p.error,
                permissions,
            }
        })
        .collect())
}

#[tauri::command]
async fn get_plugin_info(
    state: tauri::State<'_, AppState>,
    plugin_id: String,
) -> Result<PluginInfoUI, String> {
    let manager = state.plugin_manager.lock().await;
    let p = manager
        .get_plugin_info(&plugin_id)
        .ok_or_else(|| format!("Plugin '{}' not found", plugin_id))?;

    let plugin_type = p
        .capabilities
        .first()
        .map(|c| format!("{:?}", c).to_lowercase())
        .unwrap_or_else(|| "event".to_string());

    let status = if p.error.is_some() {
        "error".to_string()
    } else if p.enabled {
        "active".to_string()
    } else {
        "disabled".to_string()
    };

    let permissions = p
        .capabilities
        .iter()
        .map(|c| format!("{:?}", c).to_lowercase())
        .collect();

    Ok(PluginInfoUI {
        id: p.id,
        name: p.name,
        version: p.version,
        author: p.author,
        description: p.description,
        plugin_type,
        enabled: p.enabled,
        status,
        error_message: p.error,
        permissions,
    })
}

#[tauri::command]
async fn enable_plugin(state: tauri::State<'_, AppState>, plugin_id: String) -> Result<(), String> {
    let manager = state.plugin_manager.lock().await;
    manager
        .enable_plugin(&plugin_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn disable_plugin(
    state: tauri::State<'_, AppState>,
    plugin_id: String,
) -> Result<(), String> {
    let manager = state.plugin_manager.lock().await;
    manager
        .disable_plugin(&plugin_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn install_plugin_from_file(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<PluginInfoUI, String> {
    let manager = state.plugin_manager.lock().await;
    let plugin_id = manager
        .install_plugin(std::path::Path::new(&path))
        .await
        .map_err(|e| e.to_string())?;

    drop(manager);

    // Fetch and return the new plugin info
    get_plugin_info(state, plugin_id).await
}

#[tauri::command]
async fn uninstall_plugin(
    state: tauri::State<'_, AppState>,
    plugin_id: String,
) -> Result<(), String> {
    let manager = state.plugin_manager.lock().await;
    manager
        .uninstall_plugin(&plugin_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_plugin_config(
    state: tauri::State<'_, AppState>,
    plugin_id: String,
) -> Result<serde_json::Value, String> {
    let manager = state.plugin_manager.lock().await;
    let entry = manager
        .registry()
        .get(&plugin_id)
        .ok_or_else(|| format!("Plugin '{}' not found", plugin_id))?;

    Ok(serde_json::to_value(&entry.config).unwrap_or(serde_json::json!({})))
}

#[tauri::command]
async fn set_plugin_config(
    state: tauri::State<'_, AppState>,
    plugin_id: String,
    config: serde_json::Value,
) -> Result<(), String> {
    let manager = state.plugin_manager.lock().await;

    // Convert Value to HashMap
    let config_map: std::collections::HashMap<String, serde_json::Value> =
        serde_json::from_value(config).map_err(|e| e.to_string())?;

    manager
        .update_config(&plugin_id, config_map)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn reload_plugin(state: tauri::State<'_, AppState>, plugin_id: String) -> Result<(), String> {
    let manager = state.plugin_manager.lock().await;

    // Unload then load
    let _ = manager.unload_plugin(&plugin_id).await;
    manager
        .load_plugin(&plugin_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn discover_plugins(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    let manager = state.plugin_manager.lock().await;
    manager.discover_plugins().await.map_err(|e| e.to_string())
}

fn toggle_window<R: Runtime>(
    app: &tauri::AppHandle<R>,
    tray_position: Option<tauri::PhysicalPosition<f64>>,
) {
    if let Some(window) = app.get_webview_window("main") {
        let is_visible = window.is_visible().unwrap_or(false);
        if is_visible {
            window.hide().unwrap();
        } else {
            // Position near system tray (bottom-right on Windows)
            if let Ok(Some(monitor)) = window.current_monitor() {
                let window_size = window
                    .outer_size()
                    .unwrap_or(tauri::PhysicalSize::new(320, 480));
                let monitor_size = monitor.size();
                let monitor_pos = monitor.position();
                let scale_factor = monitor.scale_factor();

                // Estimate taskbar height (typically 40-48 pixels on Windows at 100% scale)
                let taskbar_height = (48.0 * scale_factor) as i32;
                // Padding from screen edge
                let padding = (12.0 * scale_factor) as i32;

                let (x, y) = if let Some(tray_pos) = tray_position {
                    // Position window centered above the tray icon click position
                    let tray_x = tray_pos.x as i32;
                    let tray_y = tray_pos.y as i32;

                    // Center horizontally on tray icon, but keep within screen bounds
                    let mut x = tray_x - (window_size.width as i32 / 2);
                    // Position above the tray click (above taskbar)
                    let y = tray_y - window_size.height as i32 - padding;

                    // Ensure window stays within screen bounds
                    let screen_right = monitor_pos.x + monitor_size.width as i32;
                    let screen_left = monitor_pos.x;

                    if x + window_size.width as i32 > screen_right - padding {
                        x = screen_right - window_size.width as i32 - padding;
                    }
                    if x < screen_left + padding {
                        x = screen_left + padding;
                    }

                    (x, y.max(monitor_pos.y + padding))
                } else {
                    // Fallback: position at bottom-right corner above taskbar
                    let x = monitor_pos.x + monitor_size.width as i32
                        - window_size.width as i32
                        - padding;
                    let y = monitor_pos.y + monitor_size.height as i32
                        - window_size.height as i32
                        - taskbar_height
                        - padding;
                    (x, y)
                };

                let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
            }

            window.show().unwrap();
            window.set_focus().unwrap();
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    dotenvy::dotenv().ok();
    let base_url = omni_core::config::get_api_base_url();
    // Initialize logging
    let log_dir = CliConfig::config_path()
        .unwrap()
        .parent()
        .unwrap()
        .join("logs");
    let _ = std::fs::create_dir_all(&log_dir);

    let _ = flexi_logger::Logger::try_with_str(
        "info, omni_core=debug, omni_api=debug, omni_helper=debug",
    )
    .unwrap()
    .log_to_file(
        flexi_logger::FileSpec::default()
            .directory(&log_dir)
            .basename("omniedge"),
    )
    .duplicate_to_stdout(flexi_logger::Duplicate::All)
    .start();

    info!("OmniEdge Desktop starting (Log dir: {})", log_dir.display());

    let manager = ConnectionManager::new(base_url, None);

    // Initialize plugin manager
    let plugin_config = PluginConfig::default();
    let plugin_manager = match PluginManager::new(plugin_config) {
        Ok(pm) => pm,
        Err(e) => {
            error!("Failed to initialize plugin manager: {}", e);
            // Create a fallback with minimal config
            PluginManager::with_defaults().expect("Failed to create default plugin manager")
        }
    };

    let app_state = AppState {
        manager: Arc::new(Mutex::new(manager)),
        plugin_manager: Arc::new(Mutex::new(plugin_manager)),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state)
        .setup(|app| {
            // macOS Native Menu
            #[cfg(target_os = "macos")]
            {
                let app_menu = Menu::new(app)?;
                let quit_i = MenuItem::with_id(app, "quit", "Quit OmniEdge", true, None::<&str>)?;
                let about_i =
                    MenuItem::with_id(app, "about", "About OmniEdge", true, None::<&str>)?;
                app_menu.append_items(&[&about_i, &quit_i])?;
                app.set_menu(app_menu)?;
            }

            // Tray icon - left click only, no right-click menu
            let _tray = TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().unwrap().clone())
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        position,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        toggle_window(app, Some(position));
                    }
                })
                .build(app)?;

            // 1. Initial menu population
            let handle_init = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let state = handle_init.state::<AppState>();

                // Initialize plugin manager
                {
                    let mut plugin_manager = state.plugin_manager.lock().await;
                    if let Err(e) = plugin_manager.initialize().await {
                        error!("Failed to initialize plugin manager: {}", e);
                    } else {
                        // Discover and load plugins
                        if let Err(e) = plugin_manager.discover_plugins().await {
                            error!("Failed to discover plugins: {}", e);
                        }
                        if let Err(e) = plugin_manager.load_all().await {
                            error!("Failed to load plugins: {}", e);
                        }
                        info!("Plugin system initialized");
                    }
                }

                /*
                let manager = state.manager.lock().await;
                if let Ok(menu) = build_tray_menu(&handle_init, &manager).await {
                    if let Some(tray) = handle_init.tray_by_id("main") {
                        let _ = tray.set_menu(Some(menu));
                    }
                }
                */
            });

            // 2. Background Polling for Helper State Sync
            let handle_poll = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
                let mut last_state = ConnectionState::Disconnected;
                let mut last_net_id: Option<String> = None;

                loop {
                    interval.tick().await;
                    let req = HelperRequest {
                        command: "status".to_string(),
                        args: serde_json::json!({}),
                    };

                    if let Ok(resp) = call_helper(&req).await {
                        if resp.success {
                            if let Some(data) = resp.data {
                                #[derive(serde::Deserialize)]
                                struct HelperStatusData {
                                    state: ConnectionState,
                                    network_id: Option<String>,
                                    virtual_ip: Option<String>,
                                }

                                if let Ok(status) = serde_json::from_value::<HelperStatusData>(data)
                                {
                                    if status.state != last_state
                                        || status.network_id != last_net_id
                                    {
                                        info!("Helper state/network change detected. Syncing...");

                                        // Sync local manager state
                                        let app_state = handle_poll.state::<AppState>();
                                        let mut manager = app_state.manager.lock().await;
                                        manager
                                            .sync_state(
                                                status.state.clone(),
                                                status.network_id.clone(),
                                                status.virtual_ip.clone(),
                                            )
                                            .await;

                                        update_tray_icon(&handle_poll, status.state.clone());

                                        // Notify frontend of state change
                                        let _ = handle_poll.emit(
                                            "connection-state-changed",
                                            serde_json::json!({
                                                "state": status.state,
                                                "network_id": status.network_id,
                                                "virtual_ip": status.virtual_ip,
                                            }),
                                        );

                                        last_state = status.state;
                                        last_net_id = status.network_id;
                                    }
                                }
                            }
                        }
                    }
                }
            });

            // Hide window when it loses focus (native tray app behavior)
            if let Some(window) = app.get_webview_window("main") {
                let w = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(focused) = event {
                        if !focused {
                            let _ = w.hide();
                        }
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            try_auto_login,
            login_pwd,
            list_networks,
            list_devices,
            get_network_devices,
            get_profile,
            get_device_id,
            get_virtual_ip,
            connect,
            disconnect,
            get_state,
            set_exit_node,
            set_as_exit_node,
            is_exit_node,
            start_device_flow,
            poll_device_flow,
            start_session_login,
            wait_for_session_login,
            open_browser,
            open_logs,
            resize_window,
            check_is_admin,
            check_helper,
            get_helper_version,
            install_helper,
            get_debug_info,
            quit,
            // Plugin management commands
            list_plugins,
            get_plugin_info,
            enable_plugin,
            disable_plugin,
            install_plugin_from_file,
            uninstall_plugin,
            get_plugin_config,
            set_plugin_config,
            reload_plugin,
            discover_plugins
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
