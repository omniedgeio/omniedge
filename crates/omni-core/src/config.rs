use anyhow::{Context, Result};
use omni_api::types::{AuthResp, JoinVirtualNetworkResponse, ScanResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct CliConfig {
    pub auth_file_default_path: String,
    pub scan_result_default_path: String,
    pub auth_response: Option<AuthResp>,
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
}

impl CliConfig {
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(Self::default_with_paths()?);
        }
        let content = fs::read_to_string(path)?;
        let config = serde_json::from_str(&content)?;
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
            let mut path = dirs::home_dir()
                .context("Could not find home directory")?
                .join(".omniedge");
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
        let path = dirs::home_dir()
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
