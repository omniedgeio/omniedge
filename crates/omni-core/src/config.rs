#[cfg(not(target_os = "windows"))]
use anyhow::Context;
use anyhow::Result;
use omni_api::types::{AuthResp, JoinVirtualNetworkResponse, ScanResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

// ============================================================================
// Network Configuration for NAT Traversal (OmniNervous v0.3.0)
// ============================================================================

/// Network-level configuration for NAT traversal and connectivity
///
/// These settings control low-level networking features provided by OmniNervous v0.3.0:
/// - Relay fallback for symmetric NAT
/// - Automatic port mapping (UPnP/NAT-PMP)
/// - Encrypted signaling
/// - IPv6 dual-stack support
/// - Happy Eyeballs connection racing
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkConfig {
    /// Enable relay fallback when direct P2P fails (default: true)
    ///
    /// When enabled, symmetric NAT scenarios will use relay servers
    /// to ensure connectivity even when hole-punching is impossible.
    #[serde(default = "default_true")]
    pub relay_enabled: bool,

    /// Custom relay server address (format: "host:port")
    ///
    /// If None, uses the Nucleus signaling server as relay.
    /// Set this to use a dedicated relay infrastructure.
    #[serde(default)]
    pub relay_server: Option<String>,

    /// Enable automatic port mapping via UPnP/NAT-PMP (default: true)
    ///
    /// Attempts to open ports on your router automatically.
    /// Improves connectivity but may not work on all networks.
    #[serde(default = "default_true")]
    pub portmap_enabled: bool,

    /// Enable encrypted signaling (default: true)
    ///
    /// Uses X25519 + XSalsa20-Poly1305 to encrypt signaling messages.
    /// Protects against eavesdropping and message tampering.
    #[serde(default = "default_true")]
    pub encrypt_signaling: bool,

    /// Enable IPv6 support (default: true)
    ///
    /// Binds dual-stack sockets for both IPv4 and IPv6.
    /// Disable this on IPv4-only networks.
    #[serde(default = "default_true")]
    pub ipv6_enabled: bool,

    /// Prefer IPv6 over IPv4 when latency is similar (default: true)
    ///
    /// When both IPv4 and IPv6 paths exist, prefer IPv6 if it's
    /// within the preference threshold (see below).
    #[serde(default = "default_true")]
    pub prefer_ipv6: bool,

    /// IPv6 preference threshold in milliseconds (default: 5)
    ///
    /// If IPv6 latency is within this many ms of IPv4, prefer IPv6.
    /// Example: IPv4=20ms, IPv6=24ms, threshold=5 -> use IPv6
    #[serde(default = "default_ipv6_threshold")]
    pub ipv6_preference_threshold_ms: u32,

    /// Happy Eyeballs delay in milliseconds (default: 250)
    ///
    /// Time to wait for IPv6 connection before starting IPv4 attempt.
    /// Per RFC 8305, optimizes for IPv6 without delaying fallback.
    #[serde(default = "default_happy_eyeballs_delay")]
    pub happy_eyeballs_delay_ms: u32,

    /// WireGuard interface MTU (default: 1420)
    ///
    /// Standard WireGuard MTU is 1420. Use 1280 when running behind
    /// another VPN to prevent fragmentation issues.
    #[serde(default = "default_mtu")]
    pub mtu: u16,

    /// Enable automatic MTU detection (default: false)
    ///
    /// When enabled, automatically detects if running behind another VPN
    /// (utun, tun, wg interfaces) and reduces MTU to 1280 for compatibility.
    #[serde(default)]
    pub mtu_auto_detect: bool,
}

// Serde default value helpers
fn default_true() -> bool {
    true
}

fn default_ipv6_threshold() -> u32 {
    5 // 5ms is a proven industry default for IPv6 preference
}

fn default_happy_eyeballs_delay() -> u32 {
    250 // RFC 8305 recommendation
}

fn default_mtu() -> u16 {
    1420 // Standard WireGuard MTU
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            relay_enabled: true,
            relay_server: None,
            portmap_enabled: true,
            encrypt_signaling: true,
            ipv6_enabled: true,
            prefer_ipv6: true,
            ipv6_preference_threshold_ms: 5,
            happy_eyeballs_delay_ms: 250,
            mtu: 1420,
            mtu_auto_detect: false,
        }
    }
}

impl NetworkConfig {
    /// Validate configuration values
    pub fn validate(&self) -> Result<()> {
        if self.ipv6_preference_threshold_ms > 1000 {
            anyhow::bail!("IPv6 preference threshold must be <= 1000ms");
        }

        if self.happy_eyeballs_delay_ms > 2000 {
            anyhow::bail!("Happy Eyeballs delay must be <= 2000ms");
        }

        if let Some(ref server) = self.relay_server {
            if !server.contains(':') {
                anyhow::bail!("Relay server must include port (format: host:port)");
            }
        }

        Ok(())
    }

    /// Get the effective MTU, taking auto-detection into account
    ///
    /// When `mtu_auto_detect` is true, this checks for existing VPN interfaces
    /// and returns 1280 for VPN-over-VPN scenarios, otherwise returns the configured MTU.
    pub fn effective_mtu(&self) -> u16 {
        if self.mtu_auto_detect {
            if detect_vpn_active() {
                log::info!("Auto-MTU: Active VPN detected, using safety MTU 1280");
                1280
            } else {
                log::debug!("Auto-MTU: No VPN detected, using standard MTU {}", self.mtu);
                self.mtu
            }
        } else {
            self.mtu
        }
    }

    /// Get a summary of enabled features for display
    pub fn feature_summary(&self) -> Vec<String> {
        let mut features = Vec::new();

        if self.relay_enabled {
            features.push("Relay Fallback".to_string());
        }
        if self.portmap_enabled {
            features.push("Port Mapping".to_string());
        }
        if self.encrypt_signaling {
            features.push("Encrypted Signaling".to_string());
        }
        if self.ipv6_enabled {
            features.push(if self.prefer_ipv6 {
                "IPv6 (Preferred)".to_string()
            } else {
                "IPv6 (Available)".to_string()
            });
        }
        if self.mtu_auto_detect {
            features.push("Auto-MTU Detection".to_string());
        }

        if features.is_empty() {
            features.push("Basic Mode".to_string());
        }

        features
    }
}

/// Detect if the system is running behind an existing VPN
///
/// Checks for common VPN interface prefixes:
/// - Linux: tun, wg, tap, ppp in /sys/class/net
/// - macOS: utun, ppp via ifconfig
fn detect_vpn_active() -> bool {
    #[cfg(target_os = "linux")]
    {
        if let Ok(entries) = std::fs::read_dir("/sys/class/net") {
            for entry in entries.flatten() {
                if let Ok(name) = entry.file_name().into_string() {
                    // Detect common VPN interface prefixes
                    if name.starts_with("tun")
                        || name.starts_with("wg")
                        || name.starts_with("tap")
                        || name.starts_with("ppp")
                    {
                        log::debug!("VPN interface detected: {}", name);
                        return true;
                    }
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        // On macOS, check for utun or ppp interfaces via ifconfig
        use std::process::Command;
        if let Ok(output) = Command::new("ifconfig").output() {
            let s = String::from_utf8_lossy(&output.stdout);
            // Search for utun (standard) or ppp (legacy)
            // Note: We check lines for interface declarations, not just any mention
            for line in s.lines() {
                if (line.starts_with("utun") || line.starts_with("ppp")) && line.contains(':') {
                    log::debug!("VPN interface detected in ifconfig output");
                    return true;
                }
            }
        }
    }

    // Windows VPN detection could be added in the future
    #[cfg(target_os = "windows")]
    {
        // TODO: Implement Windows VPN detection via Get-NetAdapter or similar
        log::debug!("Windows VPN detection not yet implemented");
    }

    false
}

// ============================================================================
// SSH Configuration
// ============================================================================

/// SSH server and client configuration
///
/// Controls SSH access features including:
/// - Server listening port and authentication
/// - Session recording
/// - Command filtering
/// - SFTP access
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SshConfig {
    /// Enable SSH server (default: true)
    ///
    /// When enabled, other OmniEdge peers can SSH into this node.
    #[serde(default = "default_true")]
    pub server_enabled: bool,

    /// SSH server port (default: 22)
    ///
    /// The port to listen on for SSH connections.
    /// Only accepts connections from OmniEdge VPN addresses.
    #[serde(default = "default_ssh_port")]
    pub server_port: u16,

    /// Enable SFTP subsystem (default: true)
    ///
    /// Allows file transfers via SFTP when SSH server is enabled.
    #[serde(default = "default_true")]
    pub sftp_enabled: bool,

    /// Enable session recording (default: false)
    ///
    /// Records SSH sessions in asciinema format for audit purposes.
    #[serde(default)]
    pub recording_enabled: bool,

    /// Recording storage path (default: ~/.omniedge/recordings/)
    ///
    /// Directory where session recordings are stored.
    #[serde(default)]
    pub recording_path: Option<String>,

    /// Upload recordings to cloud (default: false)
    ///
    /// When enabled, recordings are uploaded to the OmniEdge cloud
    /// for centralized audit and compliance.
    #[serde(default)]
    pub recording_cloud_upload: bool,

    /// Enable command filtering (default: false)
    ///
    /// When enabled, commands are filtered against an allow/block list.
    #[serde(default)]
    pub command_filter_enabled: bool,

    /// Blocked commands (executed when command_filter_enabled is true)
    ///
    /// List of command patterns to block (supports glob patterns).
    /// Example: ["rm -rf *", "shutdown*", "reboot*"]
    #[serde(default)]
    pub blocked_commands: Vec<String>,

    /// Allowed commands (when set, only these commands are allowed)
    ///
    /// If this list is non-empty, only these commands are permitted.
    /// Takes precedence over blocked_commands.
    #[serde(default)]
    pub allowed_commands: Vec<String>,

    /// Read-only mode (default: false)
    ///
    /// When enabled, only read operations are allowed.
    /// Write commands and SFTP writes are blocked.
    #[serde(default)]
    pub read_only: bool,

    /// Maximum concurrent SSH sessions (default: 10)
    ///
    /// Limits the number of simultaneous SSH sessions.
    #[serde(default = "default_max_sessions")]
    pub max_sessions: u32,

    /// Session idle timeout in seconds (default: 3600 = 1 hour)
    ///
    /// Sessions inactive for this duration are automatically closed.
    /// Set to 0 to disable timeout.
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_secs: u64,

    /// Rate limit: max connections per minute per IP (default: 10)
    ///
    /// Limits connection attempts to prevent brute force attacks.
    #[serde(default = "default_rate_limit")]
    pub rate_limit_per_minute: u32,

    /// Allowed SSH users (empty = all users allowed)
    ///
    /// List of OmniEdge user emails that can SSH to this node.
    /// If empty, all users in the network can connect.
    #[serde(default)]
    pub allowed_users: Vec<String>,
}

fn default_ssh_port() -> u16 {
    22
}

fn default_max_sessions() -> u32 {
    10
}

fn default_idle_timeout() -> u64 {
    3600 // 1 hour
}

fn default_rate_limit() -> u32 {
    10
}

impl Default for SshConfig {
    fn default() -> Self {
        Self {
            server_enabled: true,
            server_port: 22,
            sftp_enabled: true,
            recording_enabled: false,
            recording_path: None,
            recording_cloud_upload: false,
            command_filter_enabled: false,
            blocked_commands: Vec::new(),
            allowed_commands: Vec::new(),
            read_only: false,
            max_sessions: 10,
            idle_timeout_secs: 3600,
            rate_limit_per_minute: 10,
            allowed_users: Vec::new(),
        }
    }
}

impl SshConfig {
    /// Validate configuration values
    pub fn validate(&self) -> Result<()> {
        if self.server_port == 0 {
            anyhow::bail!("SSH server port cannot be 0");
        }

        if self.max_sessions == 0 {
            anyhow::bail!("max_sessions must be at least 1");
        }

        if self.rate_limit_per_minute == 0 {
            anyhow::bail!("rate_limit_per_minute must be at least 1");
        }

        Ok(())
    }

    /// Get a summary of enabled features for display
    pub fn feature_summary(&self) -> Vec<String> {
        let mut features = Vec::new();

        if self.server_enabled {
            features.push(format!("SSH Server (port {})", self.server_port));
        }
        if self.sftp_enabled {
            features.push("SFTP".to_string());
        }
        if self.recording_enabled {
            features.push("Session Recording".to_string());
        }
        if self.command_filter_enabled {
            features.push("Command Filtering".to_string());
        }
        if self.read_only {
            features.push("Read-Only Mode".to_string());
        }

        if features.is_empty() {
            features.push("SSH Disabled".to_string());
        }

        features
    }
}

// ============================================================================
// CLI Configuration
// ============================================================================

/// Get the real user's home directory, even when running with sudo
#[cfg(not(target_os = "windows"))]
fn get_real_user_home() -> Option<PathBuf> {
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

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct CliConfig {
    #[serde(default, alias = "auth_default_path")]
    pub auth_file_default_path: String,
    #[serde(default, alias = "scan_default_path")]
    pub scan_result_default_path: String,
    pub auth_response: Option<AuthResp>,
    /// Unix timestamp (seconds) when the token was obtained
    #[serde(default)]
    pub token_obtained_at: Option<i64>,
    pub device_uuid: Option<String>,
    pub device_name: Option<String>,
    pub last_join_info: Option<JoinVirtualNetworkResponse>,
    pub last_network_id: Option<String>,
    #[serde(default)]
    pub scan_ip: Option<String>,
    #[serde(default)]
    pub scan_mac: Option<String>,
    #[serde(default)]
    pub scan_mask: Option<String>,
    #[serde(default)]
    pub scan_results: Option<Vec<ScanResult>>,
    #[serde(default)]
    pub identity_private_key: Option<String>,
    #[serde(default)]
    pub is_exit_node: bool,
    #[serde(default)]
    pub exit_node_ip: Option<String>,
    /// IPv6 address of the selected exit node (dual-stack support)
    #[serde(default)]
    pub exit_node_ip_v6: Option<String>,
    /// Last running mode (edge, nucleus, dual)
    #[serde(default)]
    pub last_run_mode: Option<String>,
    /// Nucleus signaling port (when running in nucleus or dual mode)
    #[serde(default)]
    pub nucleus_port: Option<u16>,
    /// Whether cluster secret is configured (don't store the actual secret)
    #[serde(default)]
    pub has_cluster_secret: bool,

    /// Network configuration for NAT traversal (OmniNervous v0.3.0+)
    #[serde(default)]
    pub network_config: NetworkConfig,

    /// SSH configuration for secure shell access
    #[serde(default)]
    pub ssh_config: SshConfig,
}

impl CliConfig {
    /// Check if the current token is expired or about to expire (within 5 minutes)
    pub fn is_token_expired(&self) -> bool {
        match (&self.auth_response, self.token_obtained_at) {
            (Some(auth), Some(obtained_at)) => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);

                // Token expires at: obtained_at + expires_in
                // We refresh 5 minutes (300 seconds) before expiration
                let expires_at = obtained_at + auth.expires_in as i64;
                let refresh_threshold = expires_at - 300;

                now >= refresh_threshold
            }
            // No token or no timestamp - consider expired to force refresh
            (None, _) => true,
            (Some(_), None) => {
                // Token exists but no timestamp - try to refresh to be safe
                // Old configs may not have token_obtained_at set
                true
            }
        }
    }

    /// Update auth response and record the current time
    pub fn set_auth_response(&mut self, auth: AuthResp) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        self.auth_response = Some(auth);
        self.token_obtained_at = Some(now);
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Self::default_with_paths();
        }
        let content = fs::read_to_string(path)?;
        let mut config: CliConfig = serde_json::from_str(&content)?;

        // Ensure paths are set (for backward compatibility with old configs)
        if config.auth_file_default_path.is_empty() || config.scan_result_default_path.is_empty() {
            let defaults = Self::default_with_paths()?;
            if config.auth_file_default_path.is_empty() {
                config.auth_file_default_path = defaults.auth_file_default_path;
            }
            if config.scan_result_default_path.is_empty() {
                config.scan_result_default_path = defaults.scan_result_default_path;
            }
        }

        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        fs::write(&path, content)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&path)?.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(&path, perms)?;
        }

        Ok(())
    }

    pub fn config_path() -> Result<PathBuf> {
        #[cfg(target_os = "windows")]
        {
            // On Windows, use LocalAppData for user-specific config
            // This is appropriate for desktop app and interactive CLI
            // The CLI service copies auth to ProgramData when needed
            let mut path = dirs::data_local_dir()
                .unwrap_or_else(|| PathBuf::from("C:\\ProgramData"))
                .join("OmniEdge");
            let _ = fs::create_dir_all(&path);
            path.push("auth.json");
            Ok(path)
        }
        #[cfg(not(target_os = "windows"))]
        {
            // When running with sudo, use the original user's home directory
            let home = get_real_user_home()
                .or_else(dirs::home_dir)
                .context("Could not find home directory")?;
            let mut path = home.join(".omniedge");
            let _ = fs::create_dir_all(&path);
            path.push("auth.json");
            Ok(path)
        }
    }

    /// Get the system-wide config path (used by Windows service running as SYSTEM)
    #[cfg(target_os = "windows")]
    pub fn system_config_path() -> Result<PathBuf> {
        let mut path = PathBuf::from("C:\\ProgramData\\OmniEdge");
        let _ = fs::create_dir_all(&path);
        path.push("auth.json");
        Ok(path)
    }

    /// Copy current config to system-wide location for Windows service access
    #[cfg(target_os = "windows")]
    pub fn copy_to_system_config(&self) -> Result<()> {
        let system_path = Self::system_config_path()?;
        if let Some(parent) = system_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        fs::write(&system_path, content)?;
        Ok(())
    }

    /// Load config from system-wide location (for Windows service)
    #[cfg(target_os = "windows")]
    pub fn load_system_config() -> Result<Self> {
        let path = Self::system_config_path()?;
        if !path.exists() {
            return Self::default_with_paths();
        }
        let content = fs::read_to_string(path)?;
        let config: CliConfig = serde_json::from_str(&content)?;
        Ok(config)
    }

    fn default_with_paths() -> Result<Self> {
        #[cfg(target_os = "windows")]
        let path = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("C:\\ProgramData"))
            .join("OmniEdge");
        #[cfg(not(target_os = "windows"))]
        let path = get_real_user_home()
            .or_else(dirs::home_dir)
            .context("Could not find home directory")?
            .join(".omniedge");

        let base = path.to_string_lossy().to_string();
        Ok(Self {
            auth_file_default_path: format!("{}/auth.json", base),
            scan_result_default_path: format!("{}/scan.json", base),
            ..Default::default()
        })
    }
}

pub const DEFAULT_BASE_URL: &str = "https://api.omniedge.io";
pub const API_VERSION: &str = "/api/v2";

pub fn get_api_base_url() -> String {
    let url =
        std::env::var("OMNIEDGE_API_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());

    let mut base = url.trim_end_matches('/').to_string();

    // Repeatedly trim version suffixes to handle potential duplication in source
    while base.ends_with("/api/v2") || base.ends_with("/api/v1") {
        base = base
            .trim_end_matches("/api/v2")
            .trim_end_matches("/api/v1")
            .to_string();
        base = base.trim_end_matches('/').to_string();
    }

    format!("{}{}", base, API_VERSION)
}
