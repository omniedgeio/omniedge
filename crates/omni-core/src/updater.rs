//! OmniEdge Auto-Updater Module
//!
//! Provides version checking and self-update functionality using GitHub Releases API.
//!
//! # Features
//! - Check for new versions via GitHub Releases
//! - Download and install updates (self-update for CLI)
//! - Cross-platform support (Windows, macOS, Linux)
//!
//! # Usage
//! ```rust,ignore
//! use omni_core::updater::{Updater, UpdaterConfig, Product};
//!
//! let updater = Updater::new(UpdaterConfig::default());
//! let release = updater.check_for_update(Product::Cli, "2.5.0").await?;
//! if let Some(release) = release {
//!     println!("New version available: {}", release.version);
//!     updater.download_and_install(&release, Product::Cli).await?;
//! }
//! ```

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// GitHub repository for OmniEdge releases
const GITHUB_REPO: &str = "omniedgeio/omniedge";
const GITHUB_API_BASE: &str = "https://api.github.com";

/// Product type for update checking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Product {
    /// CLI binary (omniedge)
    Cli,
    /// Desktop application
    Desktop,
}

impl Product {
    /// Get the asset name pattern for this product on the current platform
    pub fn asset_pattern(&self) -> &'static str {
        match self {
            Product::Cli => {
                #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
                {
                    "omniedge-cli-windows-x86_64"
                }
                #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
                {
                    "omniedge-cli-windows-aarch64"
                }
                #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
                {
                    "omniedge-cli-macos-x86_64"
                }
                #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
                {
                    "omniedge-cli-macos-aarch64"
                }
                #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
                {
                    "omniedge-cli-linux-x86_64"
                }
                #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
                {
                    "omniedge-cli-linux-aarch64"
                }
                #[cfg(not(any(
                    all(target_os = "windows", target_arch = "x86_64"),
                    all(target_os = "windows", target_arch = "aarch64"),
                    all(target_os = "macos", target_arch = "x86_64"),
                    all(target_os = "macos", target_arch = "aarch64"),
                    all(target_os = "linux", target_arch = "x86_64"),
                    all(target_os = "linux", target_arch = "aarch64"),
                )))]
                {
                    "omniedge-cli-unknown"
                }
            }
            Product::Desktop => {
                #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
                {
                    "OmniEdge_x64-setup.exe"
                }
                #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
                {
                    "OmniEdge_arm64-setup.exe"
                }
                #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
                {
                    "OmniEdge_x64.dmg"
                }
                #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
                {
                    "OmniEdge_aarch64.dmg"
                }
                #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
                {
                    "omniedge_amd64.deb"
                }
                #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
                {
                    "omniedge_arm64.deb"
                }
                #[cfg(not(any(
                    all(target_os = "windows", target_arch = "x86_64"),
                    all(target_os = "windows", target_arch = "aarch64"),
                    all(target_os = "macos", target_arch = "x86_64"),
                    all(target_os = "macos", target_arch = "aarch64"),
                    all(target_os = "linux", target_arch = "x86_64"),
                    all(target_os = "linux", target_arch = "aarch64"),
                )))]
                {
                    "omniedge-desktop-unknown"
                }
            }
        }
    }

    /// Get the executable extension for this platform
    pub fn exe_extension() -> &'static str {
        #[cfg(windows)]
        {
            ".exe"
        }
        #[cfg(not(windows))]
        {
            ""
        }
    }
}

/// Configuration for the updater
#[derive(Debug, Clone)]
pub struct UpdaterConfig {
    /// GitHub repository (owner/repo format)
    pub repo: String,
    /// Whether to include pre-release versions
    pub include_prerelease: bool,
    /// Custom download directory (defaults to temp)
    pub download_dir: Option<PathBuf>,
}

impl Default for UpdaterConfig {
    fn default() -> Self {
        Self {
            repo: GITHUB_REPO.to_string(),
            include_prerelease: false,
            download_dir: None,
        }
    }
}

/// GitHub release information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseInfo {
    /// Release version (without 'v' prefix)
    pub version: String,
    /// Release tag name (e.g., "v2.5.0")
    pub tag_name: String,
    /// Release title/name
    pub name: String,
    /// Release notes (markdown)
    pub body: String,
    /// Whether this is a pre-release
    pub prerelease: bool,
    /// Publication date
    pub published_at: String,
    /// HTML URL to the release page
    pub html_url: String,
    /// Available assets
    pub assets: Vec<ReleaseAsset>,
}

/// GitHub release asset (downloadable file)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseAsset {
    /// Asset name (filename)
    pub name: String,
    /// Download URL
    pub browser_download_url: String,
    /// File size in bytes
    pub size: u64,
    /// Content type
    pub content_type: String,
}

/// GitHub API response for releases
#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    name: Option<String>,
    body: Option<String>,
    prerelease: bool,
    published_at: String,
    html_url: String,
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
    content_type: String,
}

/// Update check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCheckResult {
    /// Current version
    pub current_version: String,
    /// Latest available version (if newer)
    pub latest_release: Option<ReleaseInfo>,
    /// Whether an update is available
    pub update_available: bool,
    /// Download URL for the current platform (if update available)
    pub download_url: Option<String>,
    /// Asset name for the current platform
    pub asset_name: Option<String>,
}

/// Progress callback for downloads
pub type ProgressCallback = Box<dyn Fn(u64, u64) + Send + Sync>;

/// OmniEdge Updater
pub struct Updater {
    config: UpdaterConfig,
    client: reqwest::Client,
}

impl Updater {
    /// Create a new updater with the given configuration
    pub fn new(config: UpdaterConfig) -> Self {
        let client = reqwest::Client::builder()
            .user_agent(format!("OmniEdge-Updater/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("Failed to create HTTP client");

        Self { config, client }
    }

    /// Create a new updater with default configuration
    pub fn with_defaults() -> Self {
        Self::new(UpdaterConfig::default())
    }

    /// Check for updates for the specified product
    ///
    /// Returns `Some(ReleaseInfo)` if a newer version is available, `None` otherwise.
    pub async fn check_for_update(
        &self,
        product: Product,
        current_version: &str,
    ) -> Result<UpdateCheckResult> {
        let latest = self.get_latest_release().await?;

        let latest_version = latest.version.clone();
        let update_available = is_newer_version(&latest_version, current_version);

        let (download_url, asset_name) = if update_available {
            let pattern = product.asset_pattern();
            let asset = latest.assets.iter().find(|a| a.name.contains(pattern));

            match asset {
                Some(a) => (Some(a.browser_download_url.clone()), Some(a.name.clone())),
                None => (None, None),
            }
        } else {
            (None, None)
        };

        Ok(UpdateCheckResult {
            current_version: current_version.to_string(),
            latest_release: if update_available { Some(latest) } else { None },
            update_available,
            download_url,
            asset_name,
        })
    }

    /// Get the latest release from GitHub
    pub async fn get_latest_release(&self) -> Result<ReleaseInfo> {
        let url = format!(
            "{}/repos/{}/releases/latest",
            GITHUB_API_BASE, self.config.repo
        );

        let response = self
            .client
            .get(&url)
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await
            .context("Failed to fetch latest release")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("GitHub API error: {} - {}", status, body));
        }

        let release: GitHubRelease = response
            .json()
            .await
            .context("Failed to parse release info")?;

        Ok(self.convert_release(release))
    }

    /// Get all releases (for showing release history)
    pub async fn get_all_releases(&self, limit: usize) -> Result<Vec<ReleaseInfo>> {
        let url = format!(
            "{}/repos/{}/releases?per_page={}",
            GITHUB_API_BASE, self.config.repo, limit
        );

        let response = self
            .client
            .get(&url)
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await
            .context("Failed to fetch releases")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("GitHub API error: {} - {}", status, body));
        }

        let releases: Vec<GitHubRelease> =
            response.json().await.context("Failed to parse releases")?;

        let mut result: Vec<ReleaseInfo> = releases
            .into_iter()
            .filter(|r| self.config.include_prerelease || !r.prerelease)
            .map(|r| self.convert_release(r))
            .collect();

        result.truncate(limit);
        Ok(result)
    }

    /// Download the update to a temporary location
    ///
    /// Returns the path to the downloaded file.
    pub async fn download_update(
        &self,
        release: &ReleaseInfo,
        product: Product,
        progress: Option<ProgressCallback>,
    ) -> Result<PathBuf> {
        let pattern = product.asset_pattern();
        let asset = release
            .assets
            .iter()
            .find(|a| a.name.contains(pattern))
            .ok_or_else(|| {
                anyhow!(
                    "No compatible asset found for {} on this platform (looking for '{}')",
                    match product {
                        Product::Cli => "CLI",
                        Product::Desktop => "Desktop",
                    },
                    pattern
                )
            })?;

        let download_dir = self
            .config
            .download_dir
            .clone()
            .unwrap_or_else(std::env::temp_dir);

        let download_path = download_dir.join(&asset.name);

        log::info!("Downloading {} to {:?}", asset.name, download_path);

        let response = self
            .client
            .get(&asset.browser_download_url)
            .send()
            .await
            .context("Failed to start download")?;

        if !response.status().is_success() {
            return Err(anyhow!("Download failed: HTTP {}", response.status()));
        }

        let total_size = response.content_length().unwrap_or(asset.size);
        let mut downloaded: u64 = 0;

        let mut file = tokio::fs::File::create(&download_path)
            .await
            .context("Failed to create download file")?;

        let mut stream = response.bytes_stream();
        use futures::StreamExt;
        use tokio::io::AsyncWriteExt;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("Error reading download stream")?;
            file.write_all(&chunk)
                .await
                .context("Failed to write to file")?;

            downloaded += chunk.len() as u64;
            if let Some(ref cb) = progress {
                cb(downloaded, total_size);
            }
        }

        file.flush().await?;
        log::info!("Download complete: {:?}", download_path);

        Ok(download_path)
    }

    /// Install the update (CLI self-update)
    ///
    /// This replaces the current executable with the downloaded update.
    /// The current executable is backed up first.
    #[cfg(not(target_os = "windows"))]
    pub async fn install_cli_update(&self, downloaded_path: &Path) -> Result<()> {
        use std::os::unix::fs::PermissionsExt;

        let current_exe =
            std::env::current_exe().context("Failed to get current executable path")?;
        let backup_path = current_exe.with_extension("old");

        log::info!("Installing update...");
        log::info!("  Current: {:?}", current_exe);
        log::info!("  Backup:  {:?}", backup_path);
        log::info!("  New:     {:?}", downloaded_path);

        // Extract if it's an archive
        let binary_path = if downloaded_path
            .extension()
            .map(|e| e == "tar" || e == "gz")
            .unwrap_or(false)
        {
            self.extract_archive(downloaded_path).await?
        } else {
            downloaded_path.to_path_buf()
        };

        // Backup current executable
        if current_exe.exists() {
            tokio::fs::rename(&current_exe, &backup_path)
                .await
                .context("Failed to backup current executable")?;
        }

        // Copy new executable
        tokio::fs::copy(&binary_path, &current_exe)
            .await
            .context("Failed to install new executable")?;

        // Set executable permissions
        let mut perms = tokio::fs::metadata(&current_exe).await?.permissions();
        perms.set_mode(0o755);
        tokio::fs::set_permissions(&current_exe, perms).await?;

        log::info!("Update installed successfully!");
        log::info!("Please restart omniedge to use the new version.");

        Ok(())
    }

    /// Install the update (CLI self-update) - Windows version
    #[cfg(target_os = "windows")]
    pub async fn install_cli_update(&self, downloaded_path: &Path) -> Result<()> {
        let current_exe =
            std::env::current_exe().context("Failed to get current executable path")?;
        let backup_path = current_exe.with_extension("exe.old");
        let temp_new = current_exe.with_extension("exe.new");

        log::info!("Installing update...");
        log::info!("  Current: {:?}", current_exe);
        log::info!("  Backup:  {:?}", backup_path);
        log::info!("  New:     {:?}", downloaded_path);

        // Extract if it's an archive
        let binary_path = if downloaded_path
            .extension()
            .map(|e| e == "zip")
            .unwrap_or(false)
        {
            self.extract_archive(downloaded_path).await?
        } else {
            downloaded_path.to_path_buf()
        };

        // Copy to temp location first
        tokio::fs::copy(&binary_path, &temp_new)
            .await
            .context("Failed to copy new executable")?;

        // On Windows, we can't replace a running executable directly
        // We'll rename the current one and put the new one in place
        if backup_path.exists() {
            tokio::fs::remove_file(&backup_path).await.ok();
        }

        // Rename current to backup
        tokio::fs::rename(&current_exe, &backup_path)
            .await
            .context("Failed to backup current executable")?;

        // Rename new to current
        tokio::fs::rename(&temp_new, &current_exe)
            .await
            .context("Failed to install new executable")?;

        log::info!("Update installed successfully!");
        log::info!("Please restart omniedge to use the new version.");

        Ok(())
    }

    /// Extract an archive and return path to the binary
    async fn extract_archive(&self, archive_path: &Path) -> Result<PathBuf> {
        let extract_dir = archive_path.parent().unwrap().join("omniedge-extract");
        tokio::fs::create_dir_all(&extract_dir).await?;

        let extension = archive_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        match extension {
            "zip" => {
                // Use zip extraction
                let file = std::fs::File::open(archive_path)?;
                let mut archive = zip::ZipArchive::new(file)?;
                archive.extract(&extract_dir)?;
            }
            "gz" | "tar" => {
                // Use tar extraction
                let file = std::fs::File::open(archive_path)?;
                let decoder = flate2::read::GzDecoder::new(file);
                let mut archive = tar::Archive::new(decoder);
                archive.unpack(&extract_dir)?;
            }
            _ => {
                // Assume it's already a binary
                return Ok(archive_path.to_path_buf());
            }
        }

        // Find the omniedge binary in the extracted files
        let binary_name = format!("omniedge{}", Product::exe_extension());
        for entry in walkdir::WalkDir::new(&extract_dir) {
            let entry = entry?;
            if entry.file_name().to_string_lossy().contains("omniedge")
                && entry.file_type().is_file()
            {
                return Ok(entry.path().to_path_buf());
            }
        }

        Err(anyhow!(
            "Could not find {} in extracted archive",
            binary_name
        ))
    }

    /// Convert GitHub API response to our ReleaseInfo
    fn convert_release(&self, release: GitHubRelease) -> ReleaseInfo {
        let version = release.tag_name.trim_start_matches('v').to_string();

        ReleaseInfo {
            version,
            tag_name: release.tag_name,
            name: release.name.unwrap_or_default(),
            body: release.body.unwrap_or_default(),
            prerelease: release.prerelease,
            published_at: release.published_at,
            html_url: release.html_url,
            assets: release
                .assets
                .into_iter()
                .map(|a| ReleaseAsset {
                    name: a.name,
                    browser_download_url: a.browser_download_url,
                    size: a.size,
                    content_type: a.content_type,
                })
                .collect(),
        }
    }
}

/// Compare two semver-like versions
///
/// Returns true if `latest` is newer than `current`.
pub fn is_newer_version(latest: &str, current: &str) -> bool {
    let parse_version = |v: &str| -> (u32, u32, u32, Option<String>) {
        let v = v.trim_start_matches('v');
        let mut parts = v.splitn(2, '-');
        let version_part = parts.next().unwrap_or("0.0.0");
        let prerelease = parts.next().map(String::from);

        let mut nums = version_part.split('.');
        let major = nums.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        let minor = nums.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        let patch = nums.next().and_then(|s| s.parse().ok()).unwrap_or(0);

        (major, minor, patch, prerelease)
    };

    let (l_major, l_minor, l_patch, l_pre) = parse_version(latest);
    let (c_major, c_minor, c_patch, c_pre) = parse_version(current);

    // Compare major.minor.patch
    match (
        l_major.cmp(&c_major),
        l_minor.cmp(&c_minor),
        l_patch.cmp(&c_patch),
    ) {
        (std::cmp::Ordering::Greater, _, _) => return true,
        (std::cmp::Ordering::Less, _, _) => return false,
        (_, std::cmp::Ordering::Greater, _) => return true,
        (_, std::cmp::Ordering::Less, _) => return false,
        (_, _, std::cmp::Ordering::Greater) => return true,
        (_, _, std::cmp::Ordering::Less) => return false,
        _ => {}
    }

    // Same version numbers - compare prerelease
    // A release version (no prerelease) is newer than a prerelease
    match (&l_pre, &c_pre) {
        (None, Some(_)) => true,  // latest is release, current is prerelease
        (Some(_), None) => false, // latest is prerelease, current is release
        _ => false,               // both same type, consider equal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        // Basic comparisons
        assert!(is_newer_version("2.5.0", "2.4.0"));
        assert!(is_newer_version("2.5.0", "2.4.9"));
        assert!(is_newer_version("3.0.0", "2.9.9"));
        assert!(!is_newer_version("2.4.0", "2.5.0"));
        assert!(!is_newer_version("2.5.0", "2.5.0"));

        // With 'v' prefix
        assert!(is_newer_version("v2.5.0", "2.4.0"));
        assert!(is_newer_version("2.5.0", "v2.4.0"));

        // Prerelease handling
        assert!(is_newer_version("2.5.0", "2.5.0-dev.1"));
        assert!(!is_newer_version("2.5.0-dev.1", "2.5.0"));
        assert!(is_newer_version("2.5.0", "2.5.0-beta"));
    }
}
