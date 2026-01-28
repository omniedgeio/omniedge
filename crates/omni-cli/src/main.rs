extern crate hex;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use omni_api::{ApiClient, DeviceService, NetworkService};
use omni_core::{CliConfig, ConnectionManager};

mod oauth;
mod service;
mod utils;

use regex::Regex;
use utils::get_hardware_id;

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

#[derive(Parser, Debug)]
#[command(
    name = "omniedge",
    about = "OmniEdge CLI - Connect your devices from anywhere with zero-config Mesh VPN.",
    long_about = "OmniEdge is a zero-config mesh VPN that connects your devices from anywhere. \n\nTypical workflow:\n1. Run 'omniedge start' to log in and connect to your first network.\n2. Use flags like -n to specify a network ID, or -x to act as an exit node.",
    version,
    arg_required_else_help = true
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start OmniEdge and run in background. Use -N/--nucleus to run as a signaling controller.
    Start {
        /// The virtual network ID to join
        #[arg(short = 'n', long)]
        network_id: Option<String>,

        /// Run as a nucleus (signaling controller) for the mesh network
        #[arg(short = 'N', long, help = "Run as a nucleus node for mesh signaling")]
        nucleus: bool,

        /// Act as an exit node (allow others to route traffic through this node)
        #[arg(short = 'x', long)]
        as_exit_node: bool,

        /// Use a specific exit node IP
        #[arg(short = 'e', long = "exit-node")]
        exit_node: Option<String>,

        /// Internal flag: Run as a background daemon
        #[arg(long, hide = true)]
        daemon: bool,

        /// Login with a security key instead of browser login
        #[arg(short = 's', long)]
        security_key: Option<String>,
    },
    /// Stop OmniEdge connection and background service
    Stop,
    /// Scan local subnet and automatically upload results to OmniEdge
    Scan {
        /// The CIDR to scan (e.g. 192.168.1.0/24)
        #[arg(short, long)]
        cidr: String,
        /// Scan timeout in seconds
        #[arg(short, long, default_value = "120")]
        timeout: i64,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize unified logger for both interactive and background use
    #[cfg(windows)]
    let mut log_dir = std::path::PathBuf::from("C:\\ProgramData\\OmniEdge");
    #[cfg(not(windows))]
    let mut log_dir = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp/omniedge"))
        .join(".omniedge");

    log_dir.push("logs");
    let _ = std::fs::create_dir_all(&log_dir);

    let _logger = flexi_logger::Logger::try_with_str("info")?
        .log_to_file(
            flexi_logger::FileSpec::default()
                .directory(&log_dir)
                .basename("omniedge")
                .suffix("log"),
        )
        .duplicate_to_stderr(flexi_logger::Duplicate::All)
        .start()?;

    log::info!(
        "OmniEdge CLI starting. Version: {}. Args: {:?}",
        env!("CARGO_PKG_VERSION"),
        std::env::args().collect::<Vec<_>>()
    );

    log::info!("Checking Elevation...");
    #[cfg(windows)]
    {
        if !is_elevated::is_elevated() {
            println!(
                "Error: Administrator privileges are required to manage virtual network adapters."
            );
            println!("Please restart your terminal (PowerShell or CMD) as an Administrator and try again.");
            return Ok(());
        }
    }
    log::info!("Elevation check passed.");

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

    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) if e.kind() == clap::error::ErrorKind::DisplayHelp => {
            print_unified_help();
            std::process::exit(0);
        }
        Err(e) => {
            log::error!("CLI Parse Error: {}", e);
            e.exit();
        }
    };
    log::info!("CLI parsed. Loading config...");
    let mut config = CliConfig::load()?;
    log::info!("Config loaded.");

    match cli.command {
        Commands::Start {
            network_id,
            nucleus,
            daemon,
            as_exit_node,
            exit_node,
            security_key,
        } => {
            // Validation
            if let Some(id) = &network_id {
                let re = Regex::new(r"^[a-zA-Z0-9_\-]+$").unwrap();
                if !re.is_match(id) {
                    return Err(anyhow::anyhow!(
                        "Invalid network_id format. Only alphanumeric, -, and _ are allowed."
                    ));
                }
            }
            if let Some(ip) = &exit_node {
                let re = Regex::new(r"^[0-9\.]+$").unwrap();
                if !re.is_match(ip) {
                    return Err(anyhow::anyhow!(
                        "Invalid exit_node format. Must be a valid IP address."
                    ));
                }
            }

            if daemon {
                let vn_id = network_id.context("Network ID required for daemon mode")?;
                return service::run_worker(&base_url, &vn_id, nucleus, as_exit_node, exit_node)
                    .await;
            }

            config.is_exit_node = as_exit_node;
            config.exit_node_ip = exit_node.clone();
            config.save()?;

            // 1. Ensure Auth
            let auth = if let Some(key) = security_key {
                oauth::login_with_security_key(&base_url, &key, &mut config).await?
            } else {
                oauth::ensure_auth(&base_url, &mut config).await?
            };

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
                println!("Registering device...");
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
            }

            // 3. Get Network
            let vn_id = if let Some(id) = network_id {
                id
            } else {
                let networks = net_service.list_all().await?;
                if networks.is_empty() {
                    return Err(anyhow::anyhow!(
                        "No networks found. Please create one on the dashboard first."
                    ));
                }
                let first = &networks[0];
                println!("Selecting network: {} ({})", first.name, first.id);
                first.id.clone()
            };

            // 4. Start Background Service
            println!("Backgrounding OmniEdge in network {}...", vn_id);
            service::setup_and_start_service(
                &base_url,
                &vn_id,
                nucleus,
                as_exit_node,
                exit_node.as_deref(),
            )
            .await?;
            println!("OmniEdge is now running in the background.");
        }
        Commands::Stop => {
            println!("Stopping OmniEdge background service...");
            service::stop_and_cleanup_service(&base_url).await?;
        }
        Commands::Scan { cidr, timeout } => {
            let results = utils::run_native_scan(&cidr, timeout)?;
            let device_net = utils::get_current_device_net_status(&cidr)?;
            config.scan_ip = Some(device_net.ip.clone());
            config.scan_mac = Some(device_net.mac.clone());
            config.scan_mask = Some(device_net.mask.clone());
            config.scan_results = Some(results.clone());
            config.save()?;

            println!("Scan complete. Found {} hosts.", results.len());

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
                        Ok(_) => println!("Upload successful."),
                        Err(e) => eprintln!("Failed to upload results: {}", e),
                    }
                } else {
                    println!(
                        "Device not registered. Please run 'omniedge start' first to register."
                    );
                }
            } else {
                println!("Not logged in. Results saved locally, will be uploaded next time you run 'scan' while logged in.");
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
            network_id,
            nucleus,
            daemon,
            as_exit_node,
            exit_node,
            ..
        } if daemon => {
            let vn_id = network_id.context("Network ID required")?;
            log::info!("Starting background worker for network {}", vn_id);
            if let Err(e) =
                service::run_worker(base_url, &vn_id, nucleus, as_exit_node, exit_node).await
            {
                log::error!("Worker failed: {}", e);
                return Err(e);
            }
        }
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

    for sub_name in ["start", "stop", "scan"] {
        if let Some(sub) = cmd.get_subcommands().find(|s| s.get_name() == sub_name) {
            println!("--- {} ---", sub_name.to_uppercase());
            let mut sub_cmd = sub.clone();
            sub_cmd.print_help().unwrap();
            println!();
        }
    }
}
