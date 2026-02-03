use anyhow::{Context, Result};
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
}

// Serde default value helpers
fn default_true() -> bool {
    true
}

fn default_ipv6_threshold() -> u32 {
    5 // 5ms is industry proven default
}

fn default_happy_eyeballs_delay() -> u32 {
    250 // RFC 8305 recommendation
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

        if features.is_empty() {
            features.push("Basic Mode".to_string());
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
                // Token exists but no timestamp - assume it might be old
                // Don't force refresh, let it fail naturally if expired
                false
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
            let mut path = dirs::data_local_dir()
                .context("Could not find local app data directory")?
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

    fn default_with_paths() -> Result<Self> {
        #[cfg(target_os = "windows")]
        let path = dirs::data_local_dir()
            .context("Could not find local app data directory")?
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
