use log::info;
use omni_api::types::{
    AuthResp, DeviceCodeResp, DeviceResponse, ProfileResponse, SessionResponse,
    VirtualNetworkDeviceResponse, VirtualNetworkResponse,
};
use omni_core::{CliConfig, ConnectionManager, ConnectionState};
use std::sync::Arc;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, Runtime,
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

    match timeout(Duration::from_secs(3), call_future).await {
        Ok(res) => res,
        Err(_) => Err("Helper request timed out".to_string()),
    }
}

#[tauri::command]
async fn try_auto_login(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let mut manager = state.manager.lock().await;
    manager.try_auto_login().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn login_pwd(
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
        Err("Auto-install not supported on this platform".to_string())
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
                    .unwrap_or_else(|| "0.0.0.0".to_string()));
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
async fn check_helper() -> bool {
    let req = HelperRequest {
        command: "ping".to_string(),
        args: serde_json::json!({}),
    };

    // Retry up to 3 times to allow for service startup delay
    for _ in 0..3 {
        if call_helper(&req).await.is_ok() {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    false
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
    // Cross-platform machine ID helper
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::*;
        use winreg::RegKey;
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        if let Ok(sqm) = hklm.open_subkey("SOFTWARE\\Microsoft\\Cryptography") {
            if let Ok(id) = sqm.get_value::<String, _>("MachineGuid") {
                return id.replace("-", "").to_lowercase();
            }
        }
    }

    // Fallback or other platforms
    let hostname = ::whoami::hostname();
    let username = ::whoami::username();
    format!("{}-{}", hostname, username)
}

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
        let device_name = ::whoami::hostname();
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
    Ok(auth)
}

#[tauri::command]
async fn get_state(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<ConnectionState, String> {
    let manager = state.manager.lock().await;
    let curr_state = manager.get_state().await;
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
            manager.set_as_exit_node(enabled);
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
    }
}

#[tauri::command]
async fn open_logs(_app: tauri::AppHandle) -> Result<(), String> {
    let log_dir = CliConfig::config_path()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or("Invalid config path")?
        .join("logs");

    let _ = std::fs::create_dir_all(&log_dir);

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(log_dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(log_dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(log_dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn toggle_window<R: Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let is_visible = window.is_visible().unwrap_or(false);
        if is_visible {
            window.hide().unwrap();
        } else {
            // Position near tray if possible
            #[cfg(target_os = "windows")]
            {
                if let Ok(monitor) = window.current_monitor() {
                    if let Some(monitor) = monitor {
                        let size = window
                            .inner_size()
                            .unwrap_or(tauri::PhysicalSize::new(320, 480));
                        let monitor_size = monitor.size();
                        let monitor_pos = monitor.position();

                        // Default to bottom right (typical for Windows tray)
                        let x = monitor_pos.x + monitor_size.width as i32 - size.width as i32 - 10;
                        let y =
                            monitor_pos.y + monitor_size.height as i32 - size.height as i32 - 50; // Above taskbar

                        let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
                    }
                }
            }

            window.show().unwrap();
            window.set_focus().unwrap();
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let base_url = "https://api.omniedge.io".to_string();
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
    let app_state = AppState {
        manager: Arc::new(Mutex::new(manager)),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
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

            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let show_i = MenuItem::with_id(app, "show", "Show OmniEdge", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

            let _tray = TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => {
                        let handle = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let state = handle.state::<AppState>();
                            let _ = disconnect(handle.clone(), state).await;
                            handle.exit(0);
                        });
                    }
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            window.show().unwrap();
                            window.set_focus().unwrap();
                        }
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        toggle_window(app);
                    }
                })
                .build(app)?;

            if let Some(window) = app.get_webview_window("main") {
                let w = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(focused) = event {
                        if !focused {
                            w.hide().unwrap();
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
            check_is_admin,
            check_helper,
            install_helper,
            quit
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
