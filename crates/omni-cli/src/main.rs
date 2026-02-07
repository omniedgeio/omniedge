extern crate hex;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use indicatif::{ProgressBar, ProgressStyle};
use omni_api::{ApiClient, DeviceService, NetworkService, UserServerService};

use omni_core::{CliConfig, ConnectionManager};
#[cfg(feature = "wasm-plugins")]
use omni_plugin::{PluginConfig, PluginManager};

mod oauth;
mod service;
mod utils;

use regex::Regex;
use utils::{get_hardware_id, sync_custom_server};

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
/// If not elevated, re-exec with sudo (Unix) or elevate via UAC (Windows)
fn require_root_privileges() {
    #[cfg(windows)]
    {
        if !is_elevated::is_elevated() {
            // Re-exec with elevated privileges via UAC
            reexec_with_elevation();
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

/// Re-execute the current process with elevated privileges (Windows)
#[cfg(windows)]
fn reexec_with_elevation() -> ! {
    use std::process::Command;

    let exe = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("omniedge.exe"));
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args_str = args.join(" ");

    eprintln!("Administrator privileges required. Requesting elevation...");

    // Use PowerShell to launch elevated process and wait for it
    let ps_command = format!(
        "Start-Process -FilePath '{}' -ArgumentList '{}' -Verb RunAs -Wait",
        exe.display(),
        args_str.replace("'", "''") // Escape single quotes for PowerShell
    );

    let result = Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps_command])
        .status();

    match result {
        Ok(status) => {
            std::process::exit(status.code().unwrap_or(0));
        }
        Err(e) => {
            eprintln!("Failed to elevate: {}", e);
            eprintln!();
            eprintln!("Please run manually as Administrator:");
            eprintln!("  • Right-click PowerShell/CMD → 'Run as Administrator'");
            std::process::exit(exit_codes::PERMISSION_DENIED);
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

/// Transport mode for VPN tunnel
///
/// L3 (TUN) mode operates at the IP layer (Layer 3) - this is the default and works
/// on all platforms. L2 (TAP) mode operates at the Ethernet layer (Layer 2), enabling
/// bridging and non-IP protocols - this is only available on Linux with the l2-vpn feature.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default, serde::Serialize, serde::Deserialize,
)]
pub enum TransportMode {
    /// Layer 3 TUN mode (IP packets) - Default, works on all platforms
    #[default]
    L3,
    /// Layer 2 TAP mode (Ethernet frames) - Linux only, requires --features l2-vpn
    L2,
}

impl TransportMode {
    /// Check if this transport mode is supported on the current platform
    pub fn is_supported(&self) -> bool {
        match self {
            TransportMode::L3 => true,
            TransportMode::L2 => {
                // L2 mode requires Linux AND the l2-vpn feature
                #[cfg(all(feature = "l2-vpn", target_os = "linux"))]
                {
                    true
                }
                #[cfg(not(all(feature = "l2-vpn", target_os = "linux")))]
                {
                    false
                }
            }
        }
    }

    /// Get the reason why this transport mode is not supported (if any)
    pub fn unsupported_reason(&self) -> Option<&'static str> {
        match self {
            TransportMode::L3 => None,
            TransportMode::L2 => {
                #[cfg(all(feature = "l2-vpn", target_os = "linux"))]
                {
                    None
                }
                #[cfg(all(not(feature = "l2-vpn"), target_os = "linux"))]
                {
                    Some("L2 mode requires the 'l2-vpn' feature. Rebuild with: cargo build --features l2-vpn")
                }
                #[cfg(not(target_os = "linux"))]
                {
                    Some("L2 mode is only supported on Linux (TAP devices require Linux kernel)")
                }
            }
        }
    }
}

/// Version from git tag (set by build.rs), falls back to Cargo.toml version
const VERSION: &str = env!("GIT_VERSION");
/// Git commit hash (set by build.rs)
const GIT_COMMIT: Option<&str> = option_env!("GIT_COMMIT");
/// Build date (set by build.rs)
const BUILD_DATE: Option<&str> = option_env!("BUILD_DATE");

/// Generate long version string with git info (for --version)
fn long_version() -> &'static str {
    // Use a static to cache the version string
    use std::sync::OnceLock;
    static VERSION_STRING: OnceLock<String> = OnceLock::new();
    VERSION_STRING
        .get_or_init(|| match (GIT_COMMIT, BUILD_DATE) {
            (Some(commit), Some(date)) => {
                format!("{}\ncommit: {}\nbuilt:  {}", VERSION, commit, date)
            }
            (Some(commit), None) => format!("{}\ncommit: {}", VERSION, commit),
            _ => VERSION.to_string(),
        })
        .as_str()
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

For more help: https://connect.omniedge.io/docs/cli"#,
    version = VERSION,
    long_version = long_version(),
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

        /// Transport mode for VPN tunnel:
        ///   l3 - Layer 3 TUN (IP packets) - default, all platforms
        ///   l2 - Layer 2 TAP (Ethernet frames) - Linux only
        ///
        /// L2 mode enables Ethernet bridging and non-IP protocols.
        /// Only available on Linux with --features l2-vpn
        #[arg(short = 't', long, value_enum, default_value = "l3")]
        transport_mode: TransportMode,

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

        /// Use a specific exit node IPv6 address (e.g., fd00::1)
        /// Used together with --exit-node for dual-stack exit node routing
        #[arg(long = "exit-node-v6")]
        exit_node_v6: Option<String>,

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
    ///
    /// EXAMPLES:
    ///   omniedge status            Show basic connection status
    ///   omniedge status --debug    Show detailed P2P connection info
    Status {
        /// Show detailed P2P connection debugging information
        /// Including disco ping/pong state, NAT traversal status, and per-peer RTT
        #[arg(short = 'd', long)]
        debug: bool,
    },
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
    /// Configure network settings for NAT traversal
    ///
    /// Manage low-level networking options including relay, port mapping,
    /// IPv6 preferences, and encrypted signaling.
    ///
    /// EXAMPLES:
    ///   omniedge config show              Show current settings
    ///   omniedge config relay on          Enable relay fallback
    ///   omniedge config portmap off       Disable port mapping
    ///   omniedge config ipv6 prefer       Enable IPv6 with preference
    ///   omniedge config reset             Reset to defaults
    #[command(subcommand)]
    Config(ConfigCommands),
    /// Manage plugins
    ///
    /// Install, enable, disable, and manage OmniEdge plugins.
    /// Plugins extend OmniEdge functionality through secure WASM sandboxes.
    ///
    /// EXAMPLES:
    ///   omniedge plugin list                    List installed plugins
    ///   omniedge plugin install ./my-plugin.zip Install a plugin
    ///   omniedge plugin enable my-plugin        Enable a plugin
    ///   omniedge plugin disable my-plugin       Disable a plugin
    ///   omniedge plugin info my-plugin          Show plugin details
    ///   omniedge plugin uninstall my-plugin     Remove a plugin
    #[cfg(feature = "wasm-plugins")]
    #[command(subcommand)]
    Plugin(PluginCommands),

    /// Check for updates and show version information
    ///
    /// EXAMPLES:
    ///   omniedge version                Check current version
    ///   omniedge version --check        Check for updates
    ///   omniedge version --releases     Show recent releases
    Version {
        /// Check GitHub for available updates
        #[arg(short, long)]
        check: bool,

        /// Show recent release history
        #[arg(short, long)]
        releases: bool,

        /// Include pre-release versions
        #[arg(long)]
        prerelease: bool,
    },

    /// Upgrade OmniEdge to the latest version
    ///
    /// Downloads and installs the latest release from GitHub.
    /// The current executable is backed up before replacement.
    ///
    /// EXAMPLES:
    ///   omniedge upgrade               Upgrade to latest stable
    ///   omniedge upgrade --check       Check only, don't install
    ///   omniedge upgrade --prerelease  Include pre-release versions
    #[cfg(feature = "updater")]
    Upgrade {
        /// Only check for updates, don't install
        #[arg(short, long)]
        check: bool,

        /// Include pre-release versions
        #[arg(long)]
        prerelease: bool,

        /// Skip confirmation prompt
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// SSH into an OmniEdge peer
    ///
    /// Connect to another device in your OmniEdge network via SSH.
    /// Uses OmniEdge identity-based authentication (no SSH keys needed).
    ///
    /// EXAMPLES:
    ///   omniedge ssh user@10.0.0.5           SSH to peer by VPN IP
    ///   omniedge ssh admin@webserver         SSH to peer by name
    ///   omniedge ssh user@host -p 2222       Use custom port
    ///   omniedge ssh user@host -- ls -la     Execute command and exit
    #[cfg(feature = "ssh")]
    Ssh {
        /// Target in format user@host[:port] (e.g., admin@10.0.0.5 or user@webserver:2222)
        target: String,

        /// Custom SSH port (default: 22)
        #[arg(short, long, default_value = "22")]
        port: u16,

        /// Command to execute (non-interactive)
        #[arg(last = true)]
        command: Vec<String>,
    },

    /// Transfer files via SFTP
    ///
    /// Securely transfer files to/from OmniEdge peers using SFTP.
    ///
    /// EXAMPLES:
    ///   omniedge sftp user@10.0.0.5          Start interactive SFTP session
    ///   omniedge sftp user@webserver         SFTP to peer by name
    #[cfg(feature = "ssh")]
    Sftp {
        /// Target in format user@host[:port]
        target: String,

        /// Custom SFTP port (default: 22)
        #[arg(short, long, default_value = "22")]
        port: u16,
    },

    /// Copy files between local and remote (SCP-like)
    ///
    /// Copy files to/from OmniEdge peers using secure file transfer.
    ///
    /// EXAMPLES:
    ///   omniedge scp file.txt user@host:/path/     Upload file
    ///   omniedge scp user@host:/path/file.txt .    Download file
    ///   omniedge scp -r dir/ user@host:/path/      Upload directory
    #[cfg(feature = "ssh")]
    Scp {
        /// Source file(s) - local path or user@host:path
        source: String,

        /// Destination - local path or user@host:path
        destination: String,

        /// Copy directories recursively
        #[arg(short, long)]
        recursive: bool,

        /// Custom port (default: 22)
        #[arg(short, long, default_value = "22")]
        port: u16,
    },

    /// Start a standalone SSH server (no OmniEdge backend required)
    ///
    /// Run an SSH server that accepts connections without requiring the full
    /// OmniEdge backend. Useful for testing, development, or standalone deployments.
    ///
    /// EXAMPLES:
    ///   omniedge ssh-server                       Start on default port 2222
    ///   omniedge ssh-server -p 22                 Start on port 22
    ///   omniedge ssh-server --permissive          Accept connections from any IP
    ///   omniedge ssh-server --allow-network 10.0.0.0/8   Only allow this network
    #[cfg(feature = "ssh")]
    SshServer {
        /// Port to listen on
        #[arg(short, long, default_value = "2222")]
        port: u16,

        /// Bind address
        #[arg(short, long, default_value = "0.0.0.0")]
        bind: String,

        /// Accept connections from any IP (permissive mode)
        #[arg(long)]
        permissive: bool,

        /// Allow connections only from localhost
        #[arg(long, conflicts_with = "permissive")]
        localhost_only: bool,

        /// Allow connections from specific network (CIDR notation)
        /// Can be specified multiple times
        #[arg(long = "allow-network", value_name = "CIDR")]
        allow_networks: Vec<String>,

        /// Device ID for this server
        #[arg(long, default_value = "standalone-server")]
        device_id: String,

        /// Map SSH user to local user (format: ssh_user:local_user)
        /// Can be specified multiple times
        #[arg(long = "user-map", value_name = "SSH:LOCAL")]
        user_maps: Vec<String>,

        /// Default local user if no mapping found
        #[arg(long)]
        default_user: Option<String>,

        /// Path to store/load host keys (generates if not present)
        #[arg(long)]
        host_key_path: Option<String>,

        /// Disable event logging
        #[arg(long)]
        quiet: bool,
    },
}

/// Network configuration subcommands
#[derive(Subcommand, Debug)]
enum ConfigCommands {
    /// Show current network configuration
    Show,
    /// Configure relay fallback (for symmetric NAT)
    ///
    /// EXAMPLES:
    ///   omniedge config relay on
    ///   omniedge config relay off
    ///   omniedge config relay server relay.example.com:3478
    Relay {
        /// Enable/disable relay or set server address
        #[arg(value_name = "on|off|server ADDRESS")]
        action: String,
    },
    /// Configure automatic port mapping (UPnP/NAT-PMP)
    ///
    /// EXAMPLES:
    ///   omniedge config portmap on
    ///   omniedge config portmap off
    Portmap {
        /// Enable or disable port mapping
        #[arg(value_name = "on|off")]
        action: String,
    },
    /// Configure IPv6 settings
    ///
    /// EXAMPLES:
    ///   omniedge config ipv6 on           Enable IPv6
    ///   omniedge config ipv6 off          Disable IPv6
    ///   omniedge config ipv6 prefer       Enable and prefer IPv6
    ///   omniedge config ipv6 no-prefer    Enable but don't prefer IPv6
    Ipv6 {
        /// IPv6 mode: on, off, prefer, no-prefer
        #[arg(value_name = "on|off|prefer|no-prefer")]
        action: String,
    },
    /// Configure encrypted signaling
    ///
    /// EXAMPLES:
    ///   omniedge config encrypt on
    ///   omniedge config encrypt off
    Encrypt {
        /// Enable or disable encrypted signaling
        #[arg(value_name = "on|off")]
        action: String,
    },
    /// Reset network configuration to defaults
    Reset,
}

/// Plugin management subcommands
#[cfg(feature = "wasm-plugins")]
#[derive(Subcommand, Debug)]
enum PluginCommands {
    /// List all installed plugins
    List,
    /// Install a plugin from a ZIP file
    ///
    /// EXAMPLE:
    ///   omniedge plugin install ./my-plugin.zip
    Install {
        /// Path to the plugin ZIP file
        path: String,
    },
    /// Uninstall a plugin
    ///
    /// EXAMPLE:
    ///   omniedge plugin uninstall my-plugin
    Uninstall {
        /// Plugin ID to uninstall
        plugin_id: String,
    },
    /// Enable a plugin
    ///
    /// EXAMPLE:
    ///   omniedge plugin enable my-plugin
    Enable {
        /// Plugin ID to enable
        plugin_id: String,
    },
    /// Disable a plugin
    ///
    /// EXAMPLE:
    ///   omniedge plugin disable my-plugin
    Disable {
        /// Plugin ID to disable
        plugin_id: String,
    },
    /// Show detailed information about a plugin
    ///
    /// EXAMPLE:
    ///   omniedge plugin info my-plugin
    Info {
        /// Plugin ID to show info for
        plugin_id: String,
    },
    /// Reload a plugin (disable and re-enable)
    ///
    /// EXAMPLE:
    ///   omniedge plugin reload my-plugin
    Reload {
        /// Plugin ID to reload
        plugin_id: String,
    },
    /// Discover plugins in the plugins directory
    Discover,
}

/// Main entry point - handles both CLI and Windows service modes
///
/// IMPORTANT: On Windows, when started by SCM (Service Control Manager), we must call
/// service_dispatcher::start() BEFORE creating any tokio runtime or initializing logging.
/// The SCM expects the service to register within a short timeout, and any delays
/// (like logging setup) can cause the service to fail with exit code 1067.
fn main() -> Result<()> {
    // On Windows, try service dispatcher FIRST before any other initialization
    // This must happen before tokio runtime, logging, or any heavy initialization
    #[cfg(windows)]
    {
        // Check if we have --daemon flag in args (quick check without full parsing)
        let args: Vec<String> = std::env::args().collect();
        let has_daemon_flag = args.iter().any(|a| a == "--daemon");

        if has_daemon_flag {
            // Try to start as Windows service - if SCM started us, this will take over
            // If started from CLI, this will fail and we continue with normal execution
            match service_dispatcher::start(SERVICE_NAME, ffi_service_main) {
                Ok(_) => {
                    // SCM took over, service_main will handle everything
                    return Ok(());
                }
                Err(_) => {
                    // Not started by SCM, continue with normal CLI execution
                    // This is expected when user runs "omniedge start --daemon" from terminal
                }
            }
        }
    }

    // Now safe to start tokio runtime for CLI mode
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async_main())
}

/// Async main logic - runs after service dispatcher check
async fn async_main() -> Result<()> {
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
        VERSION,
        std::env::args().collect::<Vec<_>>()
    );

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
            transport_mode,
            network_id,
            daemon,
            as_exit_node,
            no_exit_node,
            exit_node,
            exit_node_v6,
            port,
            secret,
            security_key,
        } => {
            // Validate transport mode is supported on this platform
            if !transport_mode.is_supported() {
                if let Some(reason) = transport_mode.unsupported_reason() {
                    eprintln!(
                        "Error: {:?} transport mode is not available.",
                        transport_mode
                    );
                    eprintln!("{}", reason);
                    std::process::exit(exit_codes::INVALID_INPUT);
                }
            }

            // Check if already connected
            let current_status =
                service::get_service_status(config.last_run_mode.as_deref(), config.nucleus_port)
                    .await;

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
                    if let (Some(auth), Some(net_id), Some(dev_id)) = (
                        &config.auth_response,
                        &config.last_network_id,
                        &config.device_uuid,
                    ) {
                        let client =
                            omni_api::ApiClient::new(base_url.clone(), Some(auth.token.clone()));

                        // Send heartbeat with new status
                        let dev_service = omni_api::DeviceService::new(&client);
                        if let Err(e) = dev_service.heartbeat(dev_id, new_exit_node_status).await {
                            log::warn!("Failed to send heartbeat: {}", e);
                        }

                        // Update device in network
                        let net_service = omni_api::NetworkService::new(&client);
                        if let Err(e) = net_service
                            .update_device(net_id, dev_id, new_exit_node_status)
                            .await
                        {
                            log::warn!("Failed to update device: {}", e);
                        }
                    }

                    if new_exit_node_status {
                        println!("Exit node mode enabled.");
                        println!(
                            "This device will now allow other peers to route traffic through it."
                        );
                    } else {
                        println!("Exit node mode disabled.");
                    }
                    std::process::exit(exit_codes::SUCCESS);
                }

                // No exit node change requested - just show status
                let vip = current_status.virtual_ip.as_deref().unwrap_or("unknown");
                let iface = current_status
                    .interface_name
                    .as_deref()
                    .unwrap_or("unknown");
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

            if let Some(ip) = &exit_node_v6 {
                // Validate that the exit node IPv6 is a valid IPv6 address
                use std::net::Ipv6Addr;
                if ip.parse::<Ipv6Addr>().is_err() {
                    eprintln!("Error: Invalid exit node IPv6 address '{}'.", ip);
                    eprintln!("Expected format: fd00::1 or 2001:db8::1 (IPv6 address)");
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
                            transport_mode,
                            as_exit_node,
                            exit_node,
                            exit_node_v6,
                            port,
                            secret,
                        )
                        .await;
                    }
                }
            }

            // Handle nucleus-only mode (no network/auth needed)
            if mode == RunMode::Nucleus {
                // Require root/admin for service installation
                require_root_privileges();

                // Save nucleus config before starting
                config.last_run_mode = Some("nucleus".to_string());
                config.nucleus_port = Some(port);
                config.has_cluster_secret = secret.is_some();
                config.save()?;

                // Sync custom server to backend if we have auth token
                if let Some(ref auth) = config.auth_response {
                    let client =
                        omni_api::ApiClient::new(base_url.clone(), Some(auth.token.clone()));
                    let user_server_service = UserServerService::new(&client);
                    if let Err(e) =
                        sync_custom_server(&user_server_service, &auth.token, mode, port).await
                    {
                        log::info!("Custom server sync skipped: {}", e);
                    } else {
                        println!("Custom server synced to backend.");
                    }
                }

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
            if exit_node_v6.is_some() {
                config.exit_node_ip_v6 = exit_node_v6.clone();
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
            let effective_exit_node_v6 = exit_node_v6
                .clone()
                .or_else(|| config.exit_node_ip_v6.clone());
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
            let token = auth.effective_token().to_string();
            let client = ApiClient::new(base_url.clone(), Some(token.clone()));
            let conn_manager = ConnectionManager::new(base_url.clone(), identity_pk);

            // Persist newly generated identity if needed
            if config.identity_private_key.is_none() {
                config.identity_private_key =
                    Some(hex::encode(conn_manager.get_identity_private_key()));
                config.save()?;
            }

            let net_service = NetworkService::new(&client);
            let device_service = DeviceService::new(&client);
            let user_server_service = UserServerService::new(&client);

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
                    eprintln!("  1. Visit https://connect.omniedge.io/dashboard");
                    eprintln!("  2. Create a new virtual network");
                    eprintln!("  3. Run 'omniedge start -n YOUR_NETWORK_ID'");
                    std::process::exit(exit_codes::GENERAL_ERROR);
                }
                let first = &networks[0];
                first.id.clone()
            };

            // Require root/admin for TUN creation and daemon setup
            require_root_privileges();

            // 4. Sync custom user server for nucleus/dual mode
            if mode == RunMode::Nucleus || mode == RunMode::Dual {
                if let Err(e) = sync_custom_server(&user_server_service, &token, mode, port).await {
                    log::info!("Custom server sync skipped: {}", e);
                }
            }

            // 5. Start Background Service
            // Note: Exit node status will be synced after the device joins the network
            // via the heartbeat mechanism in the background service.
            spinner.set_message(format!("Connecting to network {}...", vn_id));
            service::setup_and_start_service(
                &base_url,
                &vn_id,
                mode,
                transport_mode,
                effective_as_exit_node,
                effective_exit_node.as_deref(),
                effective_exit_node_v6.as_deref(),
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
            let transport_str = match transport_mode {
                TransportMode::L3 => "L3 (TUN)",
                TransportMode::L2 => "L2 (TAP)",
            };
            println!("✓ OmniEdge connected!");
            println!();
            println!("  Network:   {}", vn_id);
            println!("  Mode:      {}", mode_str);
            println!("  Transport: {}", transport_str);
            if effective_as_exit_node {
                println!("  Role:      Exit node (routing traffic for peers)");
            }
            if let Some(ref exit_ip) = effective_exit_node {
                println!("  Exit:      Routing through {}", exit_ip);
            }
            if let Some(ref exit_ip_v6) = effective_exit_node_v6 {
                println!("  Exit v6:   Routing through {}", exit_ip_v6);
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
        Commands::Status { debug } => {
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

                // Show IPv6 virtual IP if available (prefer live data, fall back to config)
                let virtual_ip_v6 = status.virtual_ip_v6.clone().or_else(|| {
                    config
                        .last_join_info
                        .as_ref()
                        .and_then(|j| j.virtual_ip_v6.clone())
                });
                if let Some(ref vip6) = virtual_ip_v6 {
                    println!("  Virtual IPv6: {}", vip6);
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
                if let Some(ref exit_ip_v6) = config.exit_node_ip_v6 {
                    println!("  Exit Node v6: {}", exit_ip_v6);
                }

                // Show debug P2P connection info if requested
                if debug {
                    println!();
                    println!("P2P Connection Debug");
                    println!("────────────────────");

                    // Note: In a real implementation, we'd query the running daemon
                    // For now, show info from NAT config
                    println!("  NAT Traversal Settings:");
                    println!(
                        "    Relay:      {}",
                        if config.network_config.relay_enabled {
                            "Enabled"
                        } else {
                            "Disabled"
                        }
                    );
                    println!(
                        "    Port Map:   {}",
                        if config.network_config.portmap_enabled {
                            "Enabled"
                        } else {
                            "Disabled"
                        }
                    );
                    println!(
                        "    IPv6:       {}",
                        if config.network_config.ipv6_enabled {
                            "Enabled"
                        } else {
                            "Disabled"
                        }
                    );
                    if config.network_config.ipv6_enabled {
                        println!(
                            "    Prefer IPv6: {}",
                            if config.network_config.prefer_ipv6 {
                                "Yes"
                            } else {
                                "No"
                            }
                        );
                    }
                    println!(
                        "    Encrypted:  {}",
                        if config.network_config.encrypt_signaling {
                            "Yes"
                        } else {
                            "No"
                        }
                    );

                    // Show relay server if configured
                    if let Some(ref relay) = config.network_config.relay_server {
                        println!("    Relay Server: {}", relay);
                    }

                    println!();
                    println!("  Note: For live P2P peer state, the daemon exposes this via");
                    println!("        internal APIs. Full debug output requires daemon query.");
                    println!();
                    println!("  Connection Protocol:");
                    println!("    1. Peer discovered via nucleus signaling");
                    println!("    2. Disco ping sent to establish NAT hole punch");
                    println!("    3. Disco pong confirms bidirectional connectivity");
                    println!("    4. WireGuard peer configured with confirmed endpoint");
                    println!("    5. If disco fails after 3 retries, mark peer as unreachable");
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
                        if let Some(ref vip6) = join_info.virtual_ip_v6 {
                            println!("    Virtual IPv6: {}", vip6);
                        }
                    }
                    if let Some(ref net_id) = config.last_network_id {
                        println!("    Network:    {}", net_id);
                    }
                }

                println!();
                println!("  Run 'omniedge start' to connect.");

                // Show debug info even when disconnected
                if debug {
                    println!();
                    println!("P2P Connection Debug");
                    println!("────────────────────");
                    println!("  NAT Traversal Settings:");
                    println!(
                        "    Relay:      {}",
                        if config.network_config.relay_enabled {
                            "Enabled"
                        } else {
                            "Disabled"
                        }
                    );
                    println!(
                        "    Port Map:   {}",
                        if config.network_config.portmap_enabled {
                            "Enabled"
                        } else {
                            "Disabled"
                        }
                    );
                    println!(
                        "    IPv6:       {}",
                        if config.network_config.ipv6_enabled {
                            "Enabled"
                        } else {
                            "Disabled"
                        }
                    );
                    println!();
                    println!("  No active connection - connect first to see P2P state.");
                }
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

            // Show network configuration (NAT traversal settings)
            let net_config = &config.network_config;
            println!("Network Configuration (v0.3.0)");
            println!("──────────────────────────────");
            println!(
                "  Relay Fallback:     {}",
                if net_config.relay_enabled {
                    "Enabled"
                } else {
                    "Disabled"
                }
            );
            if let Some(ref server) = net_config.relay_server {
                println!("  Relay Server:       {}", server);
            }
            println!(
                "  Port Mapping:       {}",
                if net_config.portmap_enabled {
                    "Enabled (UPnP/NAT-PMP)"
                } else {
                    "Disabled"
                }
            );
            println!(
                "  Encrypted Signaling: {}",
                if net_config.encrypt_signaling {
                    "Enabled"
                } else {
                    "Disabled"
                }
            );
            println!(
                "  IPv6:               {}",
                if net_config.ipv6_enabled {
                    if net_config.prefer_ipv6 {
                        "Enabled (Preferred)"
                    } else {
                        "Enabled"
                    }
                } else {
                    "Disabled"
                }
            );
            println!();

            // Show enabled features summary
            let features = net_config.feature_summary();
            println!("  Active Features: {}", features.join(", "));
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
        Commands::Config(config_cmd) => {
            handle_config_command(config_cmd, &mut config)?;
        }
        #[cfg(feature = "wasm-plugins")]
        Commands::Plugin(plugin_cmd) => {
            handle_plugin_command(plugin_cmd).await?;
        }
        Commands::Version {
            check,
            releases,
            prerelease,
        } => {
            handle_version_command(check, releases, prerelease).await?;
        }
        #[cfg(feature = "updater")]
        Commands::Upgrade {
            check,
            prerelease,
            yes,
        } => {
            handle_upgrade_command(check, prerelease, yes).await?;
        }
        #[cfg(feature = "ssh")]
        Commands::Ssh {
            target,
            port,
            command,
        } => {
            handle_ssh_command(&target, port, command, &config).await?;
        }
        #[cfg(feature = "ssh")]
        Commands::Sftp { target, port } => {
            handle_sftp_command(&target, port, &config).await?;
        }
        #[cfg(feature = "ssh")]
        Commands::Scp {
            source,
            destination,
            recursive,
            port,
        } => {
            handle_scp_command(&source, &destination, recursive, port, &config).await?;
        }
        #[cfg(feature = "ssh")]
        Commands::SshServer {
            port,
            bind,
            permissive,
            localhost_only,
            allow_networks,
            device_id,
            user_maps,
            default_user,
            host_key_path,
            quiet,
        } => {
            handle_ssh_server_command(
                port,
                &bind,
                permissive,
                localhost_only,
                allow_networks,
                &device_id,
                user_maps,
                default_user,
                host_key_path,
                quiet,
            )
            .await?;
        }
    }

    Ok(())
}

// ============================================================================
// SSH Command Handlers
// ============================================================================

/// Handle SSH command
#[cfg(feature = "ssh")]
async fn handle_ssh_command(
    target: &str,
    port: u16,
    command: Vec<String>,
    config: &CliConfig,
) -> Result<()> {
    use omni_ssh::client::{SshClient, SshTarget};

    // Parse target (user@host format)
    let mut ssh_target = SshTarget::parse(target)
        .map_err(|e| anyhow::anyhow!("Invalid target '{}': {}", target, e))?;

    // Override port if specified via flag
    if port != 22 {
        ssh_target = ssh_target.with_port(port);
    }

    // Create backend with peer info from config
    let device_id = config.device_uuid.clone().unwrap_or_default();
    let network_id = config.last_network_id.clone().unwrap_or_default();
    let backend = std::sync::Arc::new(CliSshBackend::new(device_id, network_id));

    // Load peers from API if we have auth
    if let (Some(auth), Some(net_id)) = (&config.auth_response, &config.last_network_id) {
        let base_url = omni_core::config::get_api_base_url();
        let client = omni_api::ApiClient::new(base_url, Some(auth.token.clone()));
        let net_service = omni_api::NetworkService::new(&client);

        if let Ok(peers) = net_service.get_devices(net_id).await {
            backend.set_peers(peers);
        }
    }

    println!(
        "Connecting to {}@{}:{}...",
        ssh_target.user, ssh_target.host, ssh_target.port
    );

    let client = SshClient::new(backend);
    let mut session = client.connect(ssh_target.clone()).await?;

    if command.is_empty() {
        // Interactive shell
        println!("Starting interactive shell...");
        println!("(Note: Full interactive shell requires terminal handling)");
        println!();

        let shell = session.shell().await?;

        // For now, just show that we connected
        // Full terminal handling requires crossterm or similar
        println!(
            "Connected to {}. Type 'exit' to disconnect.",
            ssh_target.host
        );
        println!("(Interactive shell not fully implemented yet)");

        shell.close().await?;
    } else {
        // Execute command
        let cmd_str = command.join(" ");
        println!("Executing: {}", cmd_str);
        println!();

        let result = session.exec(&cmd_str).await?;

        // Print stdout
        if !result.stdout.is_empty() {
            print!("{}", result.stdout_str());
        }

        // Print stderr
        if !result.stderr.is_empty() {
            eprint!("{}", result.stderr_str());
        }

        if !result.success() {
            std::process::exit(result.exit_code);
        }
    }

    session.close().await?;
    Ok(())
}

/// Handle SFTP command
#[cfg(feature = "ssh")]
async fn handle_sftp_command(target: &str, port: u16, config: &CliConfig) -> Result<()> {
    use omni_ssh::client::{SshClient, SshTarget};

    // Parse target
    let mut ssh_target = SshTarget::parse(target)
        .map_err(|e| anyhow::anyhow!("Invalid target '{}': {}", target, e))?;

    if port != 22 {
        ssh_target = ssh_target.with_port(port);
    }

    // Create backend with peer info
    let device_id = config.device_uuid.clone().unwrap_or_default();
    let network_id = config.last_network_id.clone().unwrap_or_default();
    let backend = std::sync::Arc::new(CliSshBackend::new(device_id, network_id));

    // Load peers from API if we have auth
    if let (Some(auth), Some(net_id)) = (&config.auth_response, &config.last_network_id) {
        let base_url = omni_core::config::get_api_base_url();
        let client = omni_api::ApiClient::new(base_url, Some(auth.token.clone()));
        let net_service = omni_api::NetworkService::new(&client);

        if let Ok(peers) = net_service.get_devices(net_id).await {
            backend.set_peers(peers);
        }
    }

    println!(
        "Connecting to {}@{}:{} for SFTP...",
        ssh_target.user, ssh_target.host, ssh_target.port
    );

    let client = SshClient::new(backend);
    let mut session = client.connect(ssh_target.clone()).await?;
    let sftp = session.sftp().await?;

    println!("SFTP session established.");
    println!("Interactive SFTP shell not yet implemented.");
    println!();
    println!("Available commands would be:");
    println!("  ls <path>     - List directory");
    println!("  get <remote> <local> - Download file");
    println!("  put <local> <remote> - Upload file");
    println!("  mkdir <path>  - Create directory");
    println!("  rm <path>     - Remove file");
    println!("  exit          - Close session");

    sftp.close().await?;
    session.close().await?;

    Ok(())
}

/// Handle SCP command
#[cfg(feature = "ssh")]
async fn handle_scp_command(
    source: &str,
    destination: &str,
    recursive: bool,
    port: u16,
    config: &CliConfig,
) -> Result<()> {
    use omni_ssh::client::{SshClient, SshTarget};

    // Parse source and destination to determine direction
    let (is_download, target_str, remote_path, local_path) = if source.contains('@') {
        // Download: user@host:path -> local
        let parts: Vec<&str> = source.splitn(2, ':').collect();
        if parts.len() != 2 {
            anyhow::bail!("Invalid remote source format. Use: user@host:path");
        }
        (true, parts[0], parts[1], destination)
    } else if destination.contains('@') {
        // Upload: local -> user@host:path
        let parts: Vec<&str> = destination.splitn(2, ':').collect();
        if parts.len() != 2 {
            anyhow::bail!("Invalid remote destination format. Use: user@host:path");
        }
        (false, parts[0], parts[1], source)
    } else {
        anyhow::bail!("Either source or destination must be remote (user@host:path)");
    };

    let mut ssh_target = SshTarget::parse(target_str)
        .map_err(|e| anyhow::anyhow!("Invalid target '{}': {}", target_str, e))?;

    if port != 22 {
        ssh_target = ssh_target.with_port(port);
    }

    // Create backend with peer info
    let device_id = config.device_uuid.clone().unwrap_or_default();
    let network_id = config.last_network_id.clone().unwrap_or_default();
    let backend = std::sync::Arc::new(CliSshBackend::new(device_id, network_id));

    // Load peers from API if we have auth
    if let (Some(auth), Some(net_id)) = (&config.auth_response, &config.last_network_id) {
        let base_url = omni_core::config::get_api_base_url();
        let client = omni_api::ApiClient::new(base_url, Some(auth.token.clone()));
        let net_service = omni_api::NetworkService::new(&client);

        if let Ok(peers) = net_service.get_devices(net_id).await {
            backend.set_peers(peers);
        }
    }

    let client = SshClient::new(backend);

    println!("Connecting to {}...", ssh_target.host);
    let mut session = client.connect(ssh_target).await?;
    let sftp = session.sftp().await?;

    if is_download {
        println!("Downloading {} -> {}", remote_path, local_path);
        if recursive {
            println!("(Recursive download not yet implemented)");
        }
        let bytes = sftp.get(remote_path, local_path).await?;
        println!("Downloaded {} bytes", bytes);
    } else {
        println!("Uploading {} -> {}", local_path, remote_path);
        if recursive {
            println!("(Recursive upload not yet implemented)");
        }
        let bytes = sftp.put(local_path, remote_path).await?;
        println!("Uploaded {} bytes", bytes);
    }

    sftp.close().await?;
    session.close().await?;

    Ok(())
}

/// Handle standalone SSH server command
#[cfg(feature = "ssh")]
async fn handle_ssh_server_command(
    port: u16,
    bind: &str,
    permissive: bool,
    localhost_only: bool,
    allow_networks: Vec<String>,
    device_id: &str,
    user_maps: Vec<String>,
    default_user: Option<String>,
    host_key_path: Option<String>,
    quiet: bool,
) -> Result<()> {
    use omni_ssh::server::{SshServer, SshServerConfig};
    use omni_ssh::standalone::{StandaloneConfig, StandaloneSshBackend};

    // Build configuration based on mode
    let mut config = if permissive {
        println!("Mode: Permissive (accepting connections from any IP)");
        StandaloneConfig::permissive()
    } else if localhost_only {
        println!("Mode: Localhost only (127.0.0.0/8)");
        StandaloneConfig::localhost_only()
    } else {
        println!("Mode: Private networks (10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16)");
        StandaloneConfig::default()
    };

    config.device_id = device_id.to_string();
    config.log_events = !quiet;

    // Add custom allowed networks
    for network in &allow_networks {
        let net: ipnet::IpNet = network
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid network '{}': {}", network, e))?;
        config.allowed_networks.push(net);
        println!("  Added allowed network: {}", net);
    }

    // Add user mappings
    for mapping in &user_maps {
        let parts: Vec<&str> = mapping.split(':').collect();
        if parts.len() != 2 {
            anyhow::bail!(
                "Invalid user mapping '{}'. Format: ssh_user:local_user",
                mapping
            );
        }
        config
            .user_mapping
            .insert(parts[0].to_string(), parts[1].to_string());
        println!("  User mapping: {} -> {}", parts[0], parts[1]);
    }

    if let Some(ref user) = default_user {
        config.default_local_user = Some(user.clone());
        println!("  Default local user: {}", user);
    }

    if let Some(ref path) = host_key_path {
        config.host_key_path = Some(std::path::PathBuf::from(path));
        println!("  Host key path: {}", path);
    }

    // Create backend
    let backend = std::sync::Arc::new(
        StandaloneSshBackend::new(config)
            .map_err(|e| anyhow::anyhow!("Failed to create backend: {}", e))?,
    );

    // Create server with default config
    let server_config = SshServerConfig::default();
    let server = SshServer::new(server_config, backend)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create SSH server: {}", e))?;

    // Parse bind address
    let bind_addr: std::net::SocketAddr = format!("{}:{}", bind, port)
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid bind address '{}:{}': {}", bind, port, e))?;

    println!();
    println!("========================================");
    println!("  OmniEdge Standalone SSH Server");
    println!("========================================");
    println!("  Listening on: {}", bind_addr);
    println!("  Device ID:    {}", device_id);
    println!();
    println!("Press Ctrl+C to stop the server.");
    println!();

    // Start the server (blocks until shutdown)
    server
        .start(bind_addr)
        .await
        .map_err(|e| anyhow::anyhow!("SSH server error: {}", e))?;

    println!("SSH server stopped.");
    Ok(())
}

/// CLI SSH backend that resolves peers via OmniEdge API
#[cfg(feature = "ssh")]
struct CliSshBackend {
    /// Cached peers from last network join
    peers: std::sync::Mutex<Vec<omni_api::types::VirtualNetworkDeviceResponse>>,
    /// Our device ID
    device_id: String,
    /// Our network ID  
    network_id: String,
}

#[cfg(feature = "ssh")]
impl CliSshBackend {
    fn new(device_id: String, network_id: String) -> Self {
        Self {
            peers: std::sync::Mutex::new(Vec::new()),
            device_id,
            network_id,
        }
    }

    fn set_peers(&self, peers: Vec<omni_api::types::VirtualNetworkDeviceResponse>) {
        let mut guard = self.peers.lock().unwrap();
        *guard = peers;
    }
}

#[cfg(feature = "ssh")]
#[omni_ssh::async_trait]
impl omni_ssh::server::SshBackend for CliSshBackend {
    async fn get_host_keys(&self) -> anyhow::Result<Vec<omni_ssh::russh_keys::key::KeyPair>> {
        // CLI client doesn't run SSH server, return empty
        Ok(Vec::new())
    }

    fn ssh_enabled(&self) -> bool {
        false // CLI is client-only
    }

    async fn who_is(
        &self,
        addr: std::net::IpAddr,
    ) -> anyhow::Result<Option<omni_ssh::server::PeerIdentity>> {
        let peers = self.peers.lock().unwrap();
        for peer in peers.iter() {
            if let Ok(peer_ip) = peer.virtual_ip.parse::<std::net::IpAddr>() {
                if peer_ip == addr {
                    return Ok(Some(omni_ssh::server::PeerIdentity {
                        node: omni_ssh::types::NodeInfo {
                            id: peer.id.clone(),
                            name: peer.name.clone(),
                            virtual_ip: peer.virtual_ip.clone(),
                            tags: Vec::new(),
                            online: peer.online,
                            network_id: self.network_id.clone(),
                        },
                        user: omni_ssh::types::UserProfile {
                            id: String::new(),
                            email: String::new(),
                            name: None,
                        },
                    }));
                }
            }
        }
        Ok(None)
    }

    async fn get_ssh_policy(&self) -> anyhow::Result<omni_ssh::types::SshPolicy> {
        Ok(omni_ssh::types::SshPolicy::default())
    }

    async fn on_ssh_event(&self, _event: omni_ssh::server::SshEvent) {}

    fn is_omniedge_ip(&self, addr: std::net::IpAddr) -> bool {
        let peers = self.peers.lock().unwrap();
        for peer in peers.iter() {
            if let Ok(peer_ip) = peer.virtual_ip.parse::<std::net::IpAddr>() {
                if peer_ip == addr {
                    return true;
                }
            }
        }
        false
    }

    fn device_id(&self) -> &str {
        &self.device_id
    }

    fn network_id(&self) -> &str {
        &self.network_id
    }

    async fn resolve_peer_name(&self, name: &str) -> anyhow::Result<Option<std::net::IpAddr>> {
        let peers = self.peers.lock().unwrap();

        // Try exact name match first
        for peer in peers.iter() {
            if peer.name.eq_ignore_ascii_case(name) {
                if let Ok(ip) = peer.virtual_ip.parse() {
                    return Ok(Some(ip));
                }
            }
        }

        // Try partial match (contains)
        for peer in peers.iter() {
            if peer.name.to_lowercase().contains(&name.to_lowercase()) {
                if let Ok(ip) = peer.virtual_ip.parse() {
                    return Ok(Some(ip));
                }
            }
        }

        Ok(None)
    }

    async fn list_peers(&self) -> anyhow::Result<Vec<omni_ssh::server::PeerInfo>> {
        let peers = self.peers.lock().unwrap();
        let mut result = Vec::new();

        for peer in peers.iter() {
            if let Ok(ip) = peer.virtual_ip.parse() {
                result.push(omni_ssh::server::PeerInfo {
                    name: peer.name.clone(),
                    vpn_ip: ip,
                    online: peer.online,
                    device_id: Some(peer.id.clone()),
                });
            }
        }

        Ok(result)
    }
}

/// Handle network configuration subcommands
fn handle_config_command(cmd: ConfigCommands, config: &mut CliConfig) -> Result<()> {
    use omni_core::NetworkConfig;

    match cmd {
        ConfigCommands::Show => {
            let net_config = &config.network_config;
            println!();
            println!("Network Configuration");
            println!("─────────────────────");
            println!();
            println!(
                "  Relay Fallback:      {}",
                if net_config.relay_enabled {
                    "Enabled"
                } else {
                    "Disabled"
                }
            );
            if let Some(ref server) = net_config.relay_server {
                println!("  Relay Server:        {}", server);
            } else {
                println!("  Relay Server:        (default - use signaling server)");
            }
            println!();
            println!(
                "  Port Mapping:        {}",
                if net_config.portmap_enabled {
                    "Enabled (UPnP/NAT-PMP)"
                } else {
                    "Disabled"
                }
            );
            println!();
            println!(
                "  Encrypted Signaling: {}",
                if net_config.encrypt_signaling {
                    "Enabled (X25519 + XSalsa20-Poly1305)"
                } else {
                    "Disabled"
                }
            );
            println!();
            println!(
                "  IPv6:                {}",
                if net_config.ipv6_enabled {
                    "Enabled"
                } else {
                    "Disabled"
                }
            );
            if net_config.ipv6_enabled {
                println!(
                    "  IPv6 Preference:     {}",
                    if net_config.prefer_ipv6 {
                        format!(
                            "Preferred (within {}ms of IPv4)",
                            net_config.ipv6_preference_threshold_ms
                        )
                    } else {
                        "Not preferred".to_string()
                    }
                );
                println!(
                    "  Happy Eyeballs:      {}ms delay",
                    net_config.happy_eyeballs_delay_ms
                );
            }
            println!();

            // Show feature summary
            let features = net_config.feature_summary();
            println!("  Active Features: {}", features.join(", "));
            println!();
        }
        ConfigCommands::Relay { action } => {
            let action_lower = action.to_lowercase();
            match action_lower.as_str() {
                "on" | "enable" | "true" | "1" => {
                    config.network_config.relay_enabled = true;
                    config.save()?;
                    println!("Relay fallback enabled.");
                    println!("Symmetric NAT scenarios will use relay servers for connectivity.");
                }
                "off" | "disable" | "false" | "0" => {
                    config.network_config.relay_enabled = false;
                    config.save()?;
                    println!("Relay fallback disabled.");
                    println!("Warning: Devices behind symmetric NAT may not be able to connect.");
                }
                "server" => {
                    eprintln!("Error: Missing server address.");
                    eprintln!("Usage: omniedge config relay server HOST:PORT");
                    std::process::exit(exit_codes::INVALID_INPUT);
                }
                _ => {
                    // Assume it's a server address
                    if action.contains(':') {
                        config.network_config.relay_server = Some(action.clone());
                        config.network_config.relay_enabled = true;
                        config.save()?;
                        println!("Relay server set to: {}", action);
                    } else {
                        eprintln!("Error: Invalid relay action '{}'.", action);
                        eprintln!("Usage:");
                        eprintln!("  omniedge config relay on");
                        eprintln!("  omniedge config relay off");
                        eprintln!("  omniedge config relay HOST:PORT");
                        std::process::exit(exit_codes::INVALID_INPUT);
                    }
                }
            }
        }
        ConfigCommands::Portmap { action } => {
            let action_lower = action.to_lowercase();
            match action_lower.as_str() {
                "on" | "enable" | "true" | "1" => {
                    config.network_config.portmap_enabled = true;
                    config.save()?;
                    println!("Port mapping enabled.");
                    println!("Will attempt UPnP/NAT-PMP to open ports automatically.");
                }
                "off" | "disable" | "false" | "0" => {
                    config.network_config.portmap_enabled = false;
                    config.save()?;
                    println!("Port mapping disabled.");
                }
                _ => {
                    eprintln!("Error: Invalid action '{}'. Use 'on' or 'off'.", action);
                    std::process::exit(exit_codes::INVALID_INPUT);
                }
            }
        }
        ConfigCommands::Ipv6 { action } => {
            let action_lower = action.to_lowercase();
            match action_lower.as_str() {
                "on" | "enable" | "true" | "1" => {
                    config.network_config.ipv6_enabled = true;
                    config.save()?;
                    println!("IPv6 enabled.");
                }
                "off" | "disable" | "false" | "0" => {
                    config.network_config.ipv6_enabled = false;
                    config.network_config.prefer_ipv6 = false;
                    config.save()?;
                    println!("IPv6 disabled.");
                }
                "prefer" | "preferred" => {
                    config.network_config.ipv6_enabled = true;
                    config.network_config.prefer_ipv6 = true;
                    config.save()?;
                    println!("IPv6 enabled and preferred.");
                    println!(
                        "IPv6 will be used when latency is within {}ms of IPv4.",
                        config.network_config.ipv6_preference_threshold_ms
                    );
                }
                "no-prefer" | "noprefer" | "no-preferred" => {
                    config.network_config.ipv6_enabled = true;
                    config.network_config.prefer_ipv6 = false;
                    config.save()?;
                    println!("IPv6 enabled but not preferred.");
                    println!("IPv4 will be used when available.");
                }
                _ => {
                    eprintln!(
                        "Error: Invalid action '{}'. Use 'on', 'off', 'prefer', or 'no-prefer'.",
                        action
                    );
                    std::process::exit(exit_codes::INVALID_INPUT);
                }
            }
        }
        ConfigCommands::Encrypt { action } => {
            let action_lower = action.to_lowercase();
            match action_lower.as_str() {
                "on" | "enable" | "true" | "1" => {
                    config.network_config.encrypt_signaling = true;
                    config.save()?;
                    println!("Encrypted signaling enabled.");
                    println!(
                        "Signaling messages will be encrypted with X25519 + XSalsa20-Poly1305."
                    );
                }
                "off" | "disable" | "false" | "0" => {
                    config.network_config.encrypt_signaling = false;
                    config.save()?;
                    println!("Encrypted signaling disabled.");
                    println!("Warning: Signaling messages will be sent in plaintext.");
                }
                _ => {
                    eprintln!("Error: Invalid action '{}'. Use 'on' or 'off'.", action);
                    std::process::exit(exit_codes::INVALID_INPUT);
                }
            }
        }
        ConfigCommands::Reset => {
            config.network_config = NetworkConfig::default();
            config.save()?;
            println!("Network configuration reset to defaults.");
            println!();
            println!("Current settings:");
            println!("  Relay Fallback:      Enabled");
            println!("  Port Mapping:        Enabled");
            println!("  Encrypted Signaling: Enabled");
            println!("  IPv6:                Enabled (Preferred)");
            println!();
        }
    }

    Ok(())
}

/// Handle plugin management subcommands
#[cfg(feature = "wasm-plugins")]
async fn handle_plugin_command(cmd: PluginCommands) -> Result<()> {
    use omni_plugin::registry::PluginState;

    // Initialize plugin manager
    let plugin_config = PluginConfig::default();
    let mut plugin_manager =
        PluginManager::new(plugin_config).context("Failed to create plugin manager")?;

    // Initialize the plugin manager (discovers plugins, etc.)
    if let Err(e) = plugin_manager.initialize().await {
        log::warn!("Plugin manager initialization warning: {}", e);
        // Continue anyway - some operations may still work
    }

    match cmd {
        PluginCommands::List => {
            // Discover plugins first
            if let Err(e) = plugin_manager.discover_plugins().await {
                log::warn!("Plugin discovery warning: {}", e);
            }

            let plugins = plugin_manager.list_plugins();

            println!();
            println!("Installed Plugins");
            println!("─────────────────");

            if plugins.is_empty() {
                println!("  No plugins installed.");
                println!();
                println!("  Install a plugin with: omniedge plugin install <path.zip>");
            } else {
                println!();
                for plugin in &plugins {
                    let status = if plugin.enabled { "●" } else { "○" };
                    let status_text = if plugin.enabled {
                        "enabled"
                    } else {
                        "disabled"
                    };
                    println!(
                        "  {} {} v{} ({})",
                        status, plugin.id, plugin.version, status_text
                    );
                    if !plugin.description.is_empty() {
                        println!("    {}", plugin.description);
                    }
                }
            }
            println!();
        }
        PluginCommands::Install { path } => {
            println!("Installing plugin from {}...", path);

            let path = std::path::Path::new(&path);
            if !path.exists() {
                eprintln!("Error: File not found: {}", path.display());
                std::process::exit(exit_codes::INVALID_INPUT);
            }

            match plugin_manager.install_plugin(path).await {
                Ok(plugin_id) => {
                    println!("✓ Plugin installed successfully!");
                    println!();
                    println!("  ID: {}", plugin_id);

                    // Get full plugin info
                    if let Some(info) = plugin_manager.get_plugin_info(&plugin_id) {
                        println!("  Name:    {}", info.name);
                        println!("  Version: {}", info.version);
                        if !info.author.is_empty() {
                            println!("  Author:  {}", info.author);
                        }
                    }
                    println!();
                    println!("  Enable with: omniedge plugin enable {}", plugin_id);
                }
                Err(e) => {
                    eprintln!("Error: Failed to install plugin: {}", e);
                    std::process::exit(exit_codes::GENERAL_ERROR);
                }
            }
        }
        PluginCommands::Uninstall { plugin_id } => {
            // Discover first to ensure registry is populated
            let _ = plugin_manager.discover_plugins().await;

            println!("Uninstalling plugin {}...", plugin_id);

            match plugin_manager.uninstall_plugin(&plugin_id).await {
                Ok(()) => {
                    println!("✓ Plugin '{}' uninstalled successfully.", plugin_id);
                }
                Err(e) => {
                    eprintln!("Error: Failed to uninstall plugin: {}", e);
                    std::process::exit(exit_codes::GENERAL_ERROR);
                }
            }
        }
        PluginCommands::Enable { plugin_id } => {
            // Discover first to ensure registry is populated
            let _ = plugin_manager.discover_plugins().await;

            println!("Enabling plugin {}...", plugin_id);

            match plugin_manager.enable_plugin(&plugin_id).await {
                Ok(()) => {
                    println!("✓ Plugin '{}' enabled.", plugin_id);
                }
                Err(e) => {
                    eprintln!("Error: Failed to enable plugin: {}", e);
                    std::process::exit(exit_codes::GENERAL_ERROR);
                }
            }
        }
        PluginCommands::Disable { plugin_id } => {
            // Discover first to ensure registry is populated
            let _ = plugin_manager.discover_plugins().await;

            println!("Disabling plugin {}...", plugin_id);

            match plugin_manager.disable_plugin(&plugin_id).await {
                Ok(()) => {
                    println!("✓ Plugin '{}' disabled.", plugin_id);
                }
                Err(e) => {
                    eprintln!("Error: Failed to disable plugin: {}", e);
                    std::process::exit(exit_codes::GENERAL_ERROR);
                }
            }
        }
        PluginCommands::Info { plugin_id } => {
            // Discover first to ensure registry is populated
            let _ = plugin_manager.discover_plugins().await;

            match plugin_manager.get_plugin_info(&plugin_id) {
                Some(plugin) => {
                    println!();
                    println!("Plugin: {}", plugin.name);
                    println!("────────────────────────────────");
                    println!("  ID:          {}", plugin.id);
                    println!("  Version:     {}", plugin.version);
                    if !plugin.author.is_empty() {
                        println!("  Author:      {}", plugin.author);
                    }
                    if !plugin.description.is_empty() {
                        println!("  Description: {}", plugin.description);
                    }
                    println!(
                        "  Enabled:     {}",
                        if plugin.enabled { "Yes" } else { "No" }
                    );

                    let state_str = match plugin.state {
                        PluginState::Discovered => "Discovered",
                        PluginState::Loading => "Loading",
                        PluginState::Loaded => "Loaded",
                        PluginState::Running => "Running",
                        PluginState::Stopped => "Stopped",
                        PluginState::Error => "Error",
                        PluginState::Disabled => "Disabled",
                    };
                    println!("  State:       {}", state_str);

                    if let Some(ref err) = plugin.error {
                        println!("  Error:       {}", err);
                    }

                    // Show capabilities
                    if !plugin.capabilities.is_empty() {
                        println!();
                        println!("  Capabilities:");
                        for cap in &plugin.capabilities {
                            println!("    - {:?}", cap);
                        }
                    }
                    println!();
                }
                None => {
                    eprintln!("Error: Plugin '{}' not found.", plugin_id);
                    eprintln!();
                    eprintln!("Run 'omniedge plugin list' to see installed plugins.");
                    std::process::exit(exit_codes::INVALID_INPUT);
                }
            }
        }
        PluginCommands::Reload { plugin_id } => {
            // Discover first to ensure registry is populated
            let _ = plugin_manager.discover_plugins().await;

            println!("Reloading plugin {}...", plugin_id);

            // Disable then enable
            if let Err(e) = plugin_manager.disable_plugin(&plugin_id).await {
                log::warn!("Disable during reload: {}", e);
            }

            match plugin_manager.enable_plugin(&plugin_id).await {
                Ok(()) => {
                    println!("✓ Plugin '{}' reloaded.", plugin_id);
                }
                Err(e) => {
                    eprintln!("Error: Failed to reload plugin: {}", e);
                    std::process::exit(exit_codes::GENERAL_ERROR);
                }
            }
        }
        PluginCommands::Discover => {
            println!("Discovering plugins...");

            match plugin_manager.discover_plugins().await {
                Ok(plugin_ids) => {
                    println!("✓ Found {} plugin(s).", plugin_ids.len());

                    if !plugin_ids.is_empty() {
                        println!();
                        for id in &plugin_ids {
                            if let Some(info) = plugin_manager.get_plugin_info(id) {
                                println!("  - {} v{}", info.id, info.version);
                            } else {
                                println!("  - {}", id);
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error: Failed to discover plugins: {}", e);
                    std::process::exit(exit_codes::GENERAL_ERROR);
                }
            }
            println!();
        }
    }

    Ok(())
}

/// Handle version command
async fn handle_version_command(check: bool, releases: bool, prerelease: bool) -> Result<()> {
    use omni_core::updater::{Product, Updater, UpdaterConfig};

    println!();
    println!("OmniEdge CLI");
    println!("────────────");
    println!("  Version:  {}", VERSION);
    if let Some(commit) = GIT_COMMIT {
        println!("  Commit:   {}", commit);
    }
    if let Some(date) = BUILD_DATE {
        println!("  Built:    {}", date);
    }
    println!();

    if releases {
        println!("Fetching release history...");
        let config = UpdaterConfig {
            include_prerelease: prerelease,
            ..Default::default()
        };
        let updater = Updater::new(config);

        match updater.get_all_releases(10).await {
            Ok(releases) => {
                println!();
                println!("Recent Releases:");
                println!("────────────────");
                for release in releases {
                    let current_marker = if release.version == VERSION {
                        " (current)"
                    } else {
                        ""
                    };
                    let pre_marker = if release.prerelease {
                        " [pre-release]"
                    } else {
                        ""
                    };
                    println!(
                        "  v{}{}{} - {}",
                        release.version,
                        current_marker,
                        pre_marker,
                        &release.published_at[..10]
                    );
                }
                println!();
                println!("View all releases: https://github.com/omniedgeio/omniedge/releases");
            }
            Err(e) => {
                eprintln!("Error fetching releases: {}", e);
                std::process::exit(exit_codes::GENERAL_ERROR);
            }
        }
    } else if check {
        println!("Checking for updates...");
        let config = UpdaterConfig {
            include_prerelease: prerelease,
            ..Default::default()
        };
        let updater = Updater::new(config);

        match updater.check_for_update(Product::Cli, VERSION).await {
            Ok(result) => {
                if result.update_available {
                    let release = result.latest_release.unwrap();
                    println!();
                    println!("╔════════════════════════════════════════════╗");
                    println!("║  Update available: v{:<23}║", release.version);
                    println!("╚════════════════════════════════════════════╝");
                    println!();
                    println!("Release Notes:");
                    // Print first 5 lines of release notes
                    for line in release.body.lines().take(5) {
                        println!("  {}", line);
                    }
                    println!();
                    println!("To upgrade, run:  omniedge upgrade");
                    println!("Release page:     {}", release.html_url);
                } else {
                    println!();
                    println!("✓ You are running the latest version ({})", VERSION);
                }
            }
            Err(e) => {
                eprintln!("Error checking for updates: {}", e);
                std::process::exit(exit_codes::GENERAL_ERROR);
            }
        }
    }

    println!();
    Ok(())
}

/// Handle upgrade command
#[cfg(feature = "updater")]
async fn handle_upgrade_command(
    check_only: bool,
    prerelease: bool,
    skip_confirm: bool,
) -> Result<()> {
    use omni_core::updater::{Product, Updater, UpdaterConfig};

    println!();
    println!("OmniEdge Updater");
    println!("────────────────");
    println!("  Current version: {}", VERSION);
    println!();

    let config = UpdaterConfig {
        include_prerelease: prerelease,
        ..Default::default()
    };
    let updater = Updater::new(config);

    println!("Checking for updates...");
    let result = match updater.check_for_update(Product::Cli, VERSION).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error checking for updates: {}", e);
            std::process::exit(exit_codes::GENERAL_ERROR);
        }
    };

    if !result.update_available {
        println!();
        println!("✓ You are running the latest version ({})", VERSION);
        println!();
        return Ok(());
    }

    let release = result.latest_release.unwrap();
    println!();
    println!("╔════════════════════════════════════════════╗");
    println!("║  New version available: v{:<18}║", release.version);
    println!("╚════════════════════════════════════════════╝");
    println!();

    if let Some(ref asset_name) = result.asset_name {
        println!("  Asset:    {}", asset_name);
    }
    if let Some(ref url) = result.download_url {
        println!("  Download: {}", url);
    }
    println!("  Released: {}", &release.published_at[..10]);
    println!();

    if check_only {
        println!("Release Notes:");
        for line in release.body.lines().take(10) {
            println!("  {}", line);
        }
        println!();
        println!("To install this update, run: omniedge upgrade");
        println!();
        return Ok(());
    }

    // Check if we have a download URL
    if result.download_url.is_none() {
        eprintln!("Error: No compatible binary found for your platform.");
        eprintln!("Please download manually from: {}", release.html_url);
        std::process::exit(exit_codes::GENERAL_ERROR);
    }

    // Confirm unless --yes flag
    if !skip_confirm {
        print!("Do you want to upgrade to v{}? [y/N] ", release.version);
        use std::io::{self, Write};
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") && !input.trim().eq_ignore_ascii_case("yes") {
            println!("Upgrade cancelled.");
            return Ok(());
        }
    }

    println!();
    println!("Downloading v{}...", release.version);

    // Create progress bar
    let pb = ProgressBar::new(100);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .progress_chars("#>-"),
    );

    let pb_clone = pb.clone();
    let progress_callback: Option<omni_core::updater::ProgressCallback> =
        Some(Box::new(move |downloaded, total| {
            pb_clone.set_length(total);
            pb_clone.set_position(downloaded);
        }));

    let downloaded_path = match updater
        .download_update(&release, Product::Cli, progress_callback)
        .await
    {
        Ok(path) => {
            pb.finish_with_message("Download complete");
            path
        }
        Err(e) => {
            pb.finish_with_message("Download failed");
            eprintln!("Error downloading update: {}", e);
            std::process::exit(exit_codes::GENERAL_ERROR);
        }
    };

    println!();
    println!("Installing...");

    match updater.install_cli_update(&downloaded_path).await {
        Ok(()) => {
            println!();
            println!("╔════════════════════════════════════════════╗");
            println!("║  ✓ Successfully upgraded to v{:<13}║", release.version);
            println!("╚════════════════════════════════════════════╝");
            println!();
            println!("Please restart omniedge to use the new version.");
            println!();
        }
        Err(e) => {
            eprintln!("Error installing update: {}", e);
            eprintln!();
            eprintln!("The downloaded file is available at: {:?}", downloaded_path);
            eprintln!("You can try installing it manually.");
            std::process::exit(exit_codes::GENERAL_ERROR);
        }
    }

    Ok(())
}

#[cfg(windows)]
define_windows_service!(ffi_service_main, win_service_main);

#[cfg(windows)]
fn win_service_main(_arguments: Vec<std::ffi::OsString>) {
    // Initialize logging FIRST when running as a Windows service
    // Use C:\ProgramData which is writable by SYSTEM account
    let log_dir = std::path::PathBuf::from("C:\\ProgramData\\OmniEdge\\logs");
    let _ = std::fs::create_dir_all(&log_dir);

    let _logger = flexi_logger::Logger::try_with_str("info")
        .and_then(|l| {
            l.log_to_file(
                flexi_logger::FileSpec::default()
                    .directory(&log_dir)
                    .basename("omniedge-service")
                    .suffix("log"),
            )
            .start()
        })
        .ok(); // Don't fail if logging can't be set up

    log::info!(
        "Windows service starting. Args: {:?}",
        std::env::args().collect::<Vec<_>>()
    );

    dotenvy::dotenv().ok();
    let base_url = omni_core::config::get_api_base_url();

    log::info!("Creating tokio runtime for service...");
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            log::error!("Failed to create tokio runtime: {}", e);
            return;
        }
    };

    rt.block_on(async {
        if let Err(e) = service_main_res(&base_url).await {
            log::error!("Service error: {}", e);
        }
    });

    log::info!("Windows service main exiting");
}

#[cfg(windows)]
async fn service_main_res(base_url: &str) -> Result<()> {
    use std::sync::Arc;
    use tokio::sync::Notify;

    // Create shutdown notification channel
    let shutdown = Arc::new(Notify::new());
    let shutdown_clone = shutdown.clone();

    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop => {
                log::info!("Received stop signal from SCM");
                shutdown_clone.notify_one();
                ServiceControlHandlerResult::NoError
            }
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
            transport_mode,
            network_id,
            daemon,
            as_exit_node,
            exit_node,
            exit_node_v6,
            port,
            secret,
            ..
        } if daemon => match mode {
            RunMode::Nucleus => {
                log::info!("Starting nucleus-only signaling server on port {}", port);
                // Run nucleus with shutdown signal
                tokio::select! {
                    result = service::run_nucleus_only(port, secret.as_deref().unwrap_or("")) => {
                        if let Err(e) = result {
                            log::error!("Nucleus server failed: {}", e);
                            return Err(e);
                        }
                    }
                    _ = shutdown.notified() => {
                        log::info!("Nucleus server stopping due to SCM stop signal");
                    }
                }
            }
            RunMode::Edge | RunMode::Dual => {
                let vn_id = network_id.context("Network ID required")?;
                log::info!(
                    "Starting background worker for network {} in {:?} mode (transport: {:?})",
                    vn_id,
                    mode,
                    transport_mode
                );
                // Run worker with shutdown signal
                tokio::select! {
                    result = service::run_worker(
                        base_url,
                        &vn_id,
                        mode,
                        transport_mode,
                        as_exit_node,
                        exit_node,
                        exit_node_v6,
                        port,
                        secret,
                    ) => {
                        if let Err(e) = result {
                            log::error!("Worker failed: {}", e);
                            return Err(e);
                        }
                    }
                    _ = shutdown.notified() => {
                        log::info!("Worker stopping due to SCM stop signal");
                    }
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
