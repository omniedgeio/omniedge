extern crate hex;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use indicatif::{ProgressBar, ProgressStyle};
use omni_api::{ApiClient, DeviceService, NetworkService};
use omni_core::{CliConfig, ConnectionManager};

mod oauth;
mod service;
mod utils;

use regex::Regex;
use utils::get_hardware_id;

/// Get the real user's home directory, even when running with sudo
#[cfg(not(windows))]
fn get_real_user_home() -> Option<std::path::PathBuf> {
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

/// Check if running with elevated privileges (root on Unix, admin on Windows)
/// If not elevated, re-exec with sudo (Unix) or show error (Windows)
fn require_root_privileges() {
    #[cfg(windows)]
    {
        if !is_elevated::is_elevated() {
            eprintln!("Error: Administrator privileges required.");
            eprintln!();
            eprintln!("Please run one of the following:");
            eprintln!("  • Right-click PowerShell/CMD → 'Run as Administrator'");
            eprintln!("  • Or run: Start-Process powershell -Verb RunAs");
            std::process::exit(exit_codes::PERMISSION_DENIED);
        }
    }
    #[cfg(unix)]
    {
        if !nix::unistd::geteuid().is_root() {
            // Re-exec with sudo - this will prompt user for password
            reexec_with_sudo();
        }
    }
}

/// Re-execute the current process with sudo
#[cfg(unix)]
fn reexec_with_sudo() -> ! {
    use std::os::unix::process::CommandExt;
    
    let exe = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("omniedge"));
    let args: Vec<String> = std::env::args().skip(1).collect();
    
    eprintln!("Root privileges required. Requesting sudo access...");
    
    // Use exec to replace current process with sudo
    let err = std::process::Command::new("sudo")
        .arg("--")
        .arg(&exe)
        .args(&args)
        .exec();
    
    // exec() only returns if there was an error
    eprintln!("Failed to execute sudo: {}", err);
    std::process::exit(exit_codes::PERMISSION_DENIED);
}

#[cfg(windows)]
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
};

pub const SERVICE_NAME: &str = "OmniEdge";

/// Exit codes for CLI
pub mod exit_codes {
    pub const SUCCESS: i32 = 0;
    pub const GENERAL_ERROR: i32 = 1;
    pub const PERMISSION_DENIED: i32 = 2;
    pub const AUTH_REQUIRED: i32 = 3;
    pub const NETWORK_ERROR: i32 = 4;
    pub const INVALID_INPUT: i32 = 5;
}

/// Operating mode for OmniEdge
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum RunMode {
    /// Edge client only - connects to a nucleus server for peer discovery
    #[default]
    Edge,
    /// Nucleus server only - runs signaling server on port 51820, no VPN tunnel
    Nucleus,
    /// Dual mode - runs both edge client AND nucleus signaling server
    Dual,
}

#[derive(Parser, Debug)]
#[command(
    name = "omniedge",
    about = "OmniEdge CLI - Zero-config Mesh VPN",
    long_about = r#"OmniEdge connects your devices securely from anywhere.

GETTING STARTED:
  omniedge start                    Connect to your default network
  omniedge start -n NETWORK_ID      Connect to a specific network  
  omniedge status                   Check connection status
  omniedge stop                     Disconnect

EXAMPLES:
  omniedge start -n my-network
  omniedge start -n my-network -x            Run as exit node
  omniedge start --no-exit-node              Disable exit node mode
  omniedge start --mode nucleus --secret mysecret123456
  omniedge scan --cidr 192.168.1.0/24

For more help: https://omniedge.io/docs/cli"#,
    version,
    after_help = "Use 'omniedge <command> --help' for more information about a command.",
    arg_required_else_help = true
)]
struct Cli {
    /// Enable verbose output (show all logs to stderr)
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start OmniEdge and connect to a network
    ///
    /// Authenticates if needed, then connects to the specified network
    /// (or the first available network if none specified).
    ///
    /// Exit node settings are persisted - use --no-exit-node to disable.
    ///
    /// EXAMPLES:
    ///   omniedge start                     Connect to first available network
    ///   omniedge start -n my-network       Connect to specific network
    ///   omniedge start -x                  Enable exit node mode
    ///   omniedge start --no-exit-node      Disable exit node mode
    ///   omniedge start -s YOUR_KEY         Use security key for authentication
    #[command(after_help = "TIP: Use --mode dual to also run a local signaling server.")]
    Start {
        /// Operating mode:
        ///   edge    - Client mode, connects to OmniEdge cloud (default)
        ///   nucleus - Signaling server only, for self-hosted deployments  
        ///   dual    - Both client and server (advanced)
        #[arg(short = 'm', long, value_enum, default_value = "edge")]
        mode: RunMode,

        /// The virtual network ID to join (required for edge/dual modes)
        #[arg(short = 'n', long)]
        network_id: Option<String>,

        /// Act as an exit node (allow others to route traffic through this node)
        #[arg(short = 'x', long, conflicts_with = "no_exit_node")]
        as_exit_node: bool,

        /// Disable exit node mode (if previously enabled)
        #[arg(long, conflicts_with = "as_exit_node")]
        no_exit_node: bool,

        /// Use a specific exit node IP (e.g., 10.0.0.1)
        #[arg(short = 'e', long = "exit-node")]
        exit_node: Option<String>,

        /// UDP port for nucleus signaling server (default: 51820)
        #[arg(short = 'p', long, default_value = "51820")]
        port: u16,

        /// Cluster secret for nucleus-only mode (optional, min 16 chars if provided)
        #[arg(long)]
        secret: Option<String>,

        /// Internal flag: Run as a background daemon
        #[arg(long, hide = true)]
        daemon: bool,

        /// Use a security key for authentication (for CI/servers)
        #[arg(short = 's', long)]
        security_key: Option<String>,
    },
    /// Stop OmniEdge connection and background service
    Stop,
    /// Show connection status and network information
    Status,
    /// Scan local subnet and upload results to OmniEdge
    ///
    /// EXAMPLE:
    ///   omniedge scan --cidr 192.168.1.0/24
    Scan {
        /// The CIDR to scan (e.g., 192.168.1.0/24)
        #[arg(short, long)]
        cidr: String,
        /// Scan timeout in seconds
        #[arg(short, long, default_value = "120")]
        timeout: i64,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI first to get verbose flag (before logger setup)
    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) if e.kind() == clap::error::ErrorKind::DisplayHelp => {
            print_unified_help();
            std::process::exit(exit_codes::SUCCESS);
        }
        Err(e) if e.kind() == clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => {
            print_unified_help();
            std::process::exit(exit_codes::SUCCESS);
        }
        Err(e) if e.kind() == clap::error::ErrorKind::DisplayVersion => {
            e.exit();
        }
        Err(e) => {
            eprintln!("Error: {}\n", e.kind());
            eprintln!("Run 'omniedge --help' for usage information.");
            std::process::exit(exit_codes::INVALID_INPUT);
        }
    };

    // Initialize unified logger for both interactive and background use
    #[cfg(windows)]
    let log_dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("C:\\ProgramData"))
        .join("OmniEdge")
        .join("logs");
    #[cfg(not(windows))]
    let log_dir = get_real_user_home()
        .or_else(dirs::home_dir)
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
        .join(".omniedge")
        .join("logs");
    let _ = std::fs::create_dir_all(&log_dir);

    // Set log level based on verbose flag
    let log_level = if cli.verbose { "debug" } else { "info" };
    let duplicate_level = if cli.verbose {
        flexi_logger::Duplicate::All
    } else {
        flexi_logger::Duplicate::Warn // Only show warnings and errors to stderr by default
    };

    let _logger = flexi_logger::Logger::try_with_str(log_level)?
        .log_to_file(
            flexi_logger::FileSpec::default()
                .directory(&log_dir)
                .basename("omniedge")
                .suffix("log"),
        )
        .duplicate_to_stderr(duplicate_level)
        .start()?;

    log::info!(
        "OmniEdge CLI starting. Version: {}. Args: {:?}",
        env!("CARGO_PKG_VERSION"),
        std::env::args().collect::<Vec<_>>()
    );

    // Root check moved to start/stop commands only

    // On Windows, if we are started by SCM, dispatcher will take over
    #[cfg(windows)]
    {
        log::info!("Attempting service dispatcher start...");
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(parent) = exe_path.parent() {
                let _ = std::env::set_current_dir(parent);
            }
        }
        if let Err(e) = service_dispatcher::start(SERVICE_NAME, ffi_service_main) {
            log::info!("Service dispatcher failed (probably CLI mode): {}", e);
            // If dispatcher fails, it probably means we were started from CLI
        } else {
            log::info!("Service dispatcher successfully took over.");
            return Ok(()); // SCM took over
        }
    }

    dotenvy::dotenv().ok();

    log::info!("Parsing configuration...");
    let base_url = omni_core::config::get_api_base_url();
    log::info!("Using API base URL: {}", base_url);

    log::info!("CLI parsed. Loading config...");
    let mut config = CliConfig::load()?;
    log::info!("Config loaded.");

    match cli.command {
        Commands::Start {
            mode,
            network_id,
            daemon,
            as_exit_node,
            no_exit_node,
            exit_node,
            port,
            secret,
            security_key,
        } => {
            // Require root/admin for TUN creation
            require_root_privileges();
            
            // Check if already connected
            let current_status = service::get_service_status(
                config.last_run_mode.as_deref(),
                config.nucleus_port,
            ).await;
            
            if current_status.is_running {
                // Already connected - check if user wants to change exit node settings
                let wants_exit_node_change = as_exit_node || no_exit_node;
                
                if wants_exit_node_change {
                    // User wants to toggle exit node mode while connected
                    let new_exit_node_status = as_exit_node; // true if -x, false if --no-exit-node
                    
                    // Update config
                    config.is_exit_node = new_exit_node_status;
                    config.save()?;
                    
                    // Call API to update device status
                    if let (Some(auth), Some(net_id), Some(dev_id)) = 
                        (&config.auth_response, &config.last_network_id, &config.device_uuid) 
                    {
                        let client = omni_api::ApiClient::new(base_url.clone(), Some(auth.token.clone()));
                        
                        // Send heartbeat with new status
                        let dev_service = omni_api::DeviceService::new(&client);
                        if let Err(e) = dev_service.heartbeat(dev_id, new_exit_node_status).await {
                            log::warn!("Failed to send heartbeat: {}", e);
                        }
                        
                        // Update device in network
                        let net_service = omni_api::NetworkService::new(&client);
                        if let Err(e) = net_service.update_device(net_id, dev_id, new_exit_node_status).await {
                            log::warn!("Failed to update device: {}", e);
                        }
                    }
                    
                    if new_exit_node_status {
                        println!("Exit node mode enabled.");
                        println!("This device will now allow other peers to route traffic through it.");
                    } else {
                        println!("Exit node mode disabled.");
                    }
                    std::process::exit(exit_codes::SUCCESS);
                }
                
                // No exit node change requested - just show status
                let vip = current_status.virtual_ip.as_deref().unwrap_or("unknown");
                let iface = current_status.interface_name.as_deref().unwrap_or("unknown");
                println!("OmniEdge is already connected.");
                println!();
                println!("  Virtual IP:  {}", vip);
                println!("  Interface:   {}", iface);
                if let Some(ref net_id) = config.last_network_id {
                    println!("  Network:     {}", net_id);
                }
                println!();
                println!("Run 'omniedge stop' first to disconnect, then start again.");
                std::process::exit(exit_codes::SUCCESS);
            }
            
            // Validation based on mode
            match mode {
                RunMode::Edge | RunMode::Dual => {
                    // Edge and Dual modes require network authentication
                    if let Some(id) = &network_id {
                        if id.is_empty() {
                            eprintln!("Error: Network ID cannot be empty.");
                            std::process::exit(exit_codes::INVALID_INPUT);
                        }
                        let re = Regex::new(r"^[a-zA-Z0-9_\-]+$").unwrap();
                        if !re.is_match(id) {
                            eprintln!("Error: Invalid network ID format.");
                            eprintln!("Network IDs can only contain letters, numbers, hyphens (-), and underscores (_).");
                            eprintln!("No spaces or special characters allowed.");
                            std::process::exit(exit_codes::INVALID_INPUT);
                        }
                        if id.len() > 64 {
                            eprintln!("Error: Network ID is too long (max 64 characters).");
                            std::process::exit(exit_codes::INVALID_INPUT);
                        }
                    }
                }
                RunMode::Nucleus => {
                    // Nucleus-only mode: secret is optional but recommended
                    if secret.is_none() {
                        eprintln!("Warning: Running nucleus server WITHOUT authentication.");
                        eprintln!("         Any client can connect. Use --secret for production.");
                        eprintln!();
                    } else if let Some(ref s) = secret {
                        if s.len() < 16 {
                            eprintln!("Error: Cluster secret must be at least 16 characters for security.");
                            std::process::exit(exit_codes::INVALID_INPUT);
                        }
                    }
                }
            }

            if let Some(ip) = &exit_node {
                // Validate that the exit node is a valid IPv4 address
                use std::net::Ipv4Addr;
                if ip.parse::<Ipv4Addr>().is_err() {
                    eprintln!("Error: Invalid exit node IP address '{}'.", ip);
                    eprintln!("Expected format: 10.0.0.1 (IPv4 address)");
                    std::process::exit(exit_codes::INVALID_INPUT);
                }
            }

            // Daemon mode handling
            if daemon {
                match mode {
                    RunMode::Nucleus => {
                        // Nucleus-only mode: just run the signaling server
                        return service::run_nucleus_only(port, secret.as_deref().unwrap_or(""))
                            .await;
                    }
                    RunMode::Edge | RunMode::Dual => {
                        let vn_id = network_id.context("Network ID required for edge/dual mode")?;
                        return service::run_worker(
                            &base_url,
                            &vn_id,
                            mode,
                            as_exit_node,
                            exit_node,
                            port,
                            secret,
                        )
                        .await;
                    }
                }
            }

            // Handle nucleus-only mode (no network/auth needed)
            if mode == RunMode::Nucleus {
                // Save nucleus config before starting
                config.last_run_mode = Some("nucleus".to_string());
                config.nucleus_port = Some(port);
                config.has_cluster_secret = secret.is_some();
                config.save()?;

                println!(
                    "Starting OmniEdge Nucleus signaling server on port {}...",
                    port
                );
                service::setup_and_start_nucleus_service(port, secret.as_deref().unwrap_or(""))
                    .await?;
                println!("✓ Nucleus signaling server is now running in the background.");
                return Ok(());
            }

            // Edge and Dual modes need authentication and network
            // Update exit node settings based on flags:
            // -x / --as-exit-node: enable exit node
            // --no-exit-node: disable exit node
            // Neither: preserve previous setting
            if as_exit_node {
                config.is_exit_node = true;
            } else if no_exit_node {
                config.is_exit_node = false;
            }
            if exit_node.is_some() {
                config.exit_node_ip = exit_node.clone();
            }
            // Save running mode
            config.last_run_mode = Some(match mode {
                RunMode::Edge => "edge".to_string(),
                RunMode::Nucleus => "nucleus".to_string(),
                RunMode::Dual => "dual".to_string(),
            });
            // Save nucleus config when running in nucleus or dual mode
            if mode == RunMode::Nucleus || mode == RunMode::Dual {
                config.nucleus_port = Some(port);
                config.has_cluster_secret = secret.is_some();
            } else {
                // Clear nucleus config when running in edge mode
                config.nucleus_port = None;
                config.has_cluster_secret = false;
            }
            // Use the effective exit node setting (from flag or saved config)
            let effective_as_exit_node = config.is_exit_node;
            let effective_exit_node = exit_node.clone().or_else(|| config.exit_node_ip.clone());
            config.save()?;

            // Create progress spinner
            let spinner = ProgressBar::new_spinner();
            spinner.set_style(
                ProgressStyle::default_spinner()
                    .template("{spinner:.cyan} {msg}")
                    .unwrap(),
            );
            spinner.enable_steady_tick(std::time::Duration::from_millis(100));

            // 1. Ensure Auth
            spinner.set_message("Authenticating...");
            let auth = if let Some(key) = security_key {
                oauth::login_with_security_key(&base_url, &key, &mut config).await?
            } else {
                oauth::ensure_auth(&base_url, &mut config).await?
            };
            spinner.set_message("Authenticated ✓");

            let identity_pk = config
                .identity_private_key
                .as_ref()
                .and_then(|k| hex::decode(k).ok())
                .and_then(|b| b.try_into().ok());
            let client = ApiClient::new(base_url.clone(), Some(auth.token.clone()));
            let conn_manager = ConnectionManager::new(base_url.clone(), identity_pk);

            // Persist newly generated identity if needed
            if config.identity_private_key.is_none() {
                config.identity_private_key =
                    Some(hex::encode(conn_manager.get_identity_private_key()));
                config.save()?;
            }

            let net_service = NetworkService::new(&client);
            let device_service = DeviceService::new(&client);

            // 2. Ensure Device Registration
            if config.device_uuid.is_none() {
                spinner.set_message("Registering device...");
                let hostname =
                    whoami::fallible::hostname().unwrap_or_else(|_| "omniedge-device".to_string());
                let os = whoami::platform().to_string();
                let hardware_id = get_hardware_id().unwrap_or_else(|_| "unknown_hwid".to_string());
                let dr = device_service
                    .register(&hostname, &hardware_id, &os)
                    .await?;
                config.device_uuid = Some(dr.id);
                config.device_name = Some(dr.name);
                config.save()?;
                spinner.set_message("Device registered ✓");
            }

            // 3. Get Network
            spinner.set_message("Fetching networks...");
            let vn_id = if let Some(id) = network_id {
                id
            } else {
                let networks = net_service.list_all().await?;
                if networks.is_empty() {
                    spinner.finish_and_clear();
                    eprintln!("Error: No networks found in your account.");
                    eprintln!();
                    eprintln!("To create a network:");
                    eprintln!("  1. Visit https://omniedge.io/dashboard");
                    eprintln!("  2. Create a new virtual network");
                    eprintln!("  3. Run 'omniedge start -n YOUR_NETWORK_ID'");
                    std::process::exit(exit_codes::GENERAL_ERROR);
                }
                let first = &networks[0];
                first.id.clone()
            };

            // 4. Start Background Service
            // Note: Exit node status will be synced after the device joins the network
            // via the heartbeat mechanism in the background service.
            spinner.set_message(format!("Connecting to network {}...", vn_id));
            service::setup_and_start_service(
                &base_url,
                &vn_id,
                mode,
                effective_as_exit_node,
                effective_exit_node.as_deref(),
                port,
                secret.as_deref(),
            )
            .await?;

            spinner.finish_and_clear();

            // Show success message
            let mode_str = match mode {
                RunMode::Edge => "edge",
                RunMode::Dual => "dual (edge + nucleus)",
                RunMode::Nucleus => "nucleus",
            };
            println!("✓ OmniEdge connected!");
            println!();
            println!("  Network: {}", vn_id);
            println!("  Mode:    {}", mode_str);
            if effective_as_exit_node {
                println!("  Role:    Exit node (routing traffic for peers)");
            }
            if let Some(ref exit_ip) = effective_exit_node {
                println!("  Exit:    Routing through {}", exit_ip);
            }
            println!();
            println!("Run 'omniedge status' to see connection details.");
            println!("Run 'omniedge stop' to disconnect.");
        }
        Commands::Stop => {
            // Require root/admin to stop daemon
            require_root_privileges();
            
            let spinner = ProgressBar::new_spinner();
            spinner.set_style(
                ProgressStyle::default_spinner()
                    .template("{spinner:.cyan} {msg}")
                    .unwrap(),
            );
            spinner.enable_steady_tick(std::time::Duration::from_millis(100));
            spinner.set_message("Stopping OmniEdge...");

            service::stop_and_cleanup_service(&base_url).await?;

            spinner.finish_and_clear();
            println!("✓ OmniEdge stopped.");
        }
        Commands::Status => {
            let status =
                service::get_service_status(config.last_run_mode.as_deref(), config.nucleus_port)
                    .await;

            println!();
            println!("OmniEdge Status");
            println!("───────────────");

            if status.is_running {
                println!("  Connection:  ● Connected");

                // Show running mode from status (which was detected) or fall back to config
                let mode = status
                    .mode
                    .as_deref()
                    .or(config.last_run_mode.as_deref())
                    .unwrap_or("edge");
                let mode_display = match mode {
                    "edge" => "Edge (VPN client)",
                    "nucleus" => "Nucleus (signaling server)",
                    "dual" => "Dual (VPN + signaling)",
                    _ => mode,
                };
                println!("  Mode:        {}", mode_display);

                // Show nucleus-specific info when in nucleus or dual mode
                if mode == "nucleus" || mode == "dual" {
                    // Prefer live port from status, fall back to config
                    let port = status.nucleus_port.or(config.nucleus_port);
                    if let Some(p) = port {
                        println!("  Nucleus Port: {}", p);
                    }
                    let secret_status = if config.has_cluster_secret {
                        "Configured"
                    } else {
                        "Not set"
                    };
                    println!("  Cluster Secret: {}", secret_status);
                }

                // Show virtual IP (prefer live data, fall back to config)
                let virtual_ip = status
                    .virtual_ip
                    .or_else(|| config.last_join_info.as_ref().map(|j| j.virtual_ip.clone()));
                if let Some(ref vip) = virtual_ip {
                    println!("  Virtual IP:  {}", vip);
                }

                // Show network ID (prefer live data, fall back to config)
                let network_id = status.network_id.or(config.last_network_id.clone());
                if let Some(ref net_id) = network_id {
                    println!("  Network:     {}", net_id);
                }

                // Show interface name
                if let Some(ref iface) = status.interface_name {
                    println!("  Interface:   {}", iface);
                }

                // Show device name from config
                if let Some(ref device_name) = config.device_name {
                    println!("  Device:      {}", device_name);
                }

                // Show exit node info
                if config.is_exit_node {
                    println!("  Role:        Exit node");
                }
                if let Some(ref exit_ip) = config.exit_node_ip {
                    println!("  Exit Node:   {}", exit_ip);
                }
            } else {
                println!("  Connection:  ○ Disconnected");

                // Show last known info if available
                if config.last_run_mode.is_some() || config.last_join_info.is_some() {
                    println!();
                    println!("  Last session:");

                    // Show last mode
                    if let Some(ref mode) = config.last_run_mode {
                        let mode_display = match mode.as_str() {
                            "edge" => "Edge (VPN client)",
                            "nucleus" => "Nucleus (signaling server)",
                            "dual" => "Dual (VPN + signaling)",
                            _ => mode.as_str(),
                        };
                        println!("    Mode:       {}", mode_display);

                        // Show nucleus info if last mode was nucleus or dual
                        if mode == "nucleus" || mode == "dual" {
                            if let Some(port) = config.nucleus_port {
                                println!("    Nucleus Port: {}", port);
                            }
                        }
                    }

                    if let Some(ref join_info) = config.last_join_info {
                        println!("    Virtual IP: {}", join_info.virtual_ip);
                    }
                    if let Some(ref net_id) = config.last_network_id {
                        println!("    Network:    {}", net_id);
                    }
                }

                println!();
                println!("  Run 'omniedge start' to connect.");
            }
            println!();

            // Show auth status
            if config.auth_response.is_some() {
                if config.is_token_expired() {
                    println!("  Auth:        Token expired (will refresh on start)");
                } else {
                    println!("  Auth:        Logged in");
                }
            } else {
                println!("  Auth:        Not logged in");
            }

            // Show log location
            println!("  Logs:        {}", log_dir.display());
            println!();
        }
        Commands::Scan { cidr, timeout } => {
            // Validate timeout
            if timeout <= 0 {
                eprintln!("Error: Timeout must be a positive number.");
                std::process::exit(exit_codes::INVALID_INPUT);
            }

            println!("Scanning {}...", cidr);
            let results = match utils::run_native_scan(&cidr, timeout) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Error: Failed to scan network.");
                    eprintln!("Details: {}", e);
                    eprintln!();
                    eprintln!("Expected CIDR format: 192.168.1.0/24");
                    std::process::exit(exit_codes::INVALID_INPUT);
                }
            };

            let device_net = match utils::get_current_device_net_status(&cidr) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("Warning: Could not get device network info: {}", e);
                    utils::DeviceNet {
                        ip: "unknown".to_string(),
                        mac: "unknown".to_string(),
                        mask: "unknown".to_string(),
                    }
                }
            };

            config.scan_ip = Some(device_net.ip.clone());
            config.scan_mac = Some(device_net.mac.clone());
            config.scan_mask = Some(device_net.mask.clone());
            config.scan_results = Some(results.clone());
            config.save()?;

            println!("✓ Scan complete. Found {} hosts.", results.len());

            if let Some(auth) = config.auth_response.as_ref() {
                println!("Uploading scan results to OmniEdge...");
                let client = ApiClient::new(base_url.clone(), Some(auth.token.clone()));
                let net_service = NetworkService::new(&client);
                if let Some(device_id) = config.device_uuid.as_ref() {
                    match net_service
                        .upload_subnets(
                            device_id,
                            &device_net.ip,
                            &device_net.mac,
                            &device_net.mask,
                            &results,
                        )
                        .await
                    {
                        Ok(_) => println!("✓ Upload successful."),
                        Err(e) => {
                            eprintln!("Warning: Failed to upload results: {}", e);
                            eprintln!(
                                "Results saved locally. Run 'omniedge scan' again when online."
                            );
                        }
                    }
                } else {
                    println!("Device not registered. Run 'omniedge start' first to register.");
                }
            } else {
                println!("Not logged in. Results saved locally.");
                println!("Run 'omniedge start' to log in and upload.");
            }
        }
    }

    Ok(())
}

#[cfg(windows)]
define_windows_service!(ffi_service_main, win_service_main);

#[cfg(windows)]
fn win_service_main(_arguments: Vec<std::ffi::OsString>) {
    dotenvy::dotenv().ok();
    let base_url = omni_core::config::get_api_base_url();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        if let Err(e) = service_main_res(&base_url).await {
            log::error!("Service error: {}", e);
        }
    });
}

#[cfg(windows)]
async fn service_main_res(base_url: &str) -> Result<()> {
    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop => ServiceControlHandlerResult::NoError,
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;

    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: std::time::Duration::default(),
        process_id: None,
    })?;

    log::info!("Service dispatcher reached. Parsing command line...");
    let cli = match Cli::try_parse() {
        Ok(c) => {
            log::info!("Parsed command successfully: {:?}", c.command);
            c
        }
        Err(e) => {
            log::error!(
                "Failed to parse CLI arguments in service context: {}. Args were: {:?}",
                e,
                std::env::args().collect::<Vec<_>>()
            );
            return Err(anyhow::anyhow!("CLI Parse Error: {}", e));
        }
    };

    match cli.command {
        Commands::Start {
            mode,
            network_id,
            daemon,
            as_exit_node,
            exit_node,
            port,
            secret,
            ..
        } if daemon => match mode {
            RunMode::Nucleus => {
                log::info!("Starting nucleus-only signaling server on port {}", port);
                if let Err(e) =
                    service::run_nucleus_only(port, secret.as_deref().unwrap_or("")).await
                {
                    log::error!("Nucleus server failed: {}", e);
                    return Err(e);
                }
            }
            RunMode::Edge | RunMode::Dual => {
                let vn_id = network_id.context("Network ID required")?;
                log::info!(
                    "Starting background worker for network {} in {:?} mode",
                    vn_id,
                    mode
                );
                if let Err(e) = service::run_worker(
                    base_url,
                    &vn_id,
                    mode,
                    as_exit_node,
                    exit_node,
                    port,
                    secret,
                )
                .await
                {
                    log::error!("Worker failed: {}", e);
                    return Err(e);
                }
            }
        },
        _ => {
            log::error!("Service started with invalid command: {:?}", cli.command);
        }
    }

    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: std::time::Duration::default(),
        process_id: None,
    })?;

    Ok(())
}

fn print_unified_help() {
    let mut cmd = Cli::command();
    cmd.print_long_help().unwrap();
    println!("\n=== SUBCOMMAND DETAILS ===\n");

    for sub_name in ["start", "stop", "status", "scan"] {
        if let Some(sub) = cmd.get_subcommands().find(|s| s.get_name() == sub_name) {
            println!("--- {} ---", sub_name.to_uppercase());
            let mut sub_cmd = sub.clone();
            sub_cmd.print_help().unwrap();
            println!();
        }
    }
}
