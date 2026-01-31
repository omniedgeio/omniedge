//! WASM plugin loader
//!
//! Handles loading, validation, and verification of plugin WASM modules.

use crate::error::{PluginError, PluginResult};
use crate::manifest::PluginManifest;
use crate::sandbox::{PluginInstance, PluginSandbox};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Plugin package containing manifest and WASM module
pub struct PluginPackage {
    /// Plugin manifest
    pub manifest: PluginManifest,
    /// Path to the plugin directory
    pub path: PathBuf,
    /// Path to the WASM module
    pub wasm_path: PathBuf,
    /// SHA256 hash of the WASM module
    pub wasm_hash: String,
    /// Optional signature
    pub signature: Option<PluginSignature>,
}

/// Plugin signature for verification
#[derive(Debug, Clone)]
pub struct PluginSignature {
    /// Signer public key (hex encoded)
    pub public_key: String,
    /// Signature (hex encoded)
    pub signature: String,
    /// Algorithm used
    pub algorithm: SignatureAlgorithm,
}

/// Signature algorithms supported
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureAlgorithm {
    Ed25519,
}

impl std::fmt::Display for SignatureAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignatureAlgorithm::Ed25519 => write!(f, "ed25519"),
        }
    }
}

/// Plugin loader for discovering and loading plugins
pub struct PluginLoader {
    /// Base directory for plugins
    plugins_dir: PathBuf,
    /// Whether to require signatures
    require_signatures: bool,
    /// Trusted signer public keys
    trusted_signers: Vec<String>,
}

impl PluginLoader {
    /// Create a new plugin loader
    pub fn new(plugins_dir: impl Into<PathBuf>) -> Self {
        Self {
            plugins_dir: plugins_dir.into(),
            require_signatures: false,
            trusted_signers: Vec::new(),
        }
    }

    /// Set whether signatures are required
    pub fn require_signatures(mut self, require: bool) -> Self {
        self.require_signatures = require;
        self
    }

    /// Add a trusted signer
    pub fn add_trusted_signer(mut self, public_key: impl Into<String>) -> Self {
        self.trusted_signers.push(public_key.into());
        self
    }

    /// Set trusted signers
    pub fn with_trusted_signers(mut self, signers: Vec<String>) -> Self {
        self.trusted_signers = signers;
        self
    }

    /// Get the plugins directory
    pub fn plugins_dir(&self) -> &Path {
        &self.plugins_dir
    }

    /// Discover all plugins in the plugins directory
    pub fn discover_plugins(&self) -> PluginResult<Vec<PluginPackage>> {
        let mut plugins = Vec::new();

        if !self.plugins_dir.exists() {
            debug!("Plugins directory does not exist: {:?}", self.plugins_dir);
            return Ok(plugins);
        }

        let entries = std::fs::read_dir(&self.plugins_dir).map_err(|e| PluginError::IoError(e))?;

        for entry in entries.flatten() {
            let path = entry.path();

            if path.is_dir() {
                match self.load_plugin_package(&path) {
                    Ok(package) => {
                        info!(
                            "Discovered plugin: {} v{}",
                            package.manifest.name, package.manifest.version
                        );
                        plugins.push(package);
                    }
                    Err(e) => {
                        warn!("Failed to load plugin from {:?}: {}", path, e);
                    }
                }
            }
        }

        Ok(plugins)
    }

    /// Load a plugin package from a directory
    pub fn load_plugin_package(&self, plugin_dir: &Path) -> PluginResult<PluginPackage> {
        // Look for manifest.json
        let manifest_path = plugin_dir.join("manifest.json");
        if !manifest_path.exists() {
            return Err(PluginError::InvalidManifest(format!(
                "manifest.json not found in {:?}",
                plugin_dir
            )));
        }

        // Parse manifest
        let manifest_content = std::fs::read_to_string(&manifest_path)?;
        let manifest: PluginManifest = serde_json::from_str(&manifest_content)
            .map_err(|e| PluginError::InvalidManifest(e.to_string()))?;

        // Validate manifest
        manifest
            .validate()
            .map_err(|e| PluginError::InvalidManifest(e.to_string()))?;

        // Find WASM module
        let wasm_filename = manifest
            .entry_points
            .wasm
            .as_deref()
            .unwrap_or("plugin.wasm");
        let wasm_path = plugin_dir.join(wasm_filename);

        if !wasm_path.exists() {
            return Err(PluginError::NotFound(format!(
                "WASM module not found: {:?}",
                wasm_path
            )));
        }

        // Compute hash
        let wasm_bytes = std::fs::read(&wasm_path)?;
        let wasm_hash = compute_sha256(&wasm_bytes);

        // Load signature if present
        let signature = self.load_signature(plugin_dir)?;

        // Verify signature if required
        if self.require_signatures {
            match &signature {
                Some(sig) => {
                    self.verify_signature(&wasm_bytes, sig)?;
                }
                None => {
                    return Err(PluginError::SignatureRequired);
                }
            }
        }

        Ok(PluginPackage {
            manifest,
            path: plugin_dir.to_path_buf(),
            wasm_path,
            wasm_hash,
            signature,
        })
    }

    /// Load plugin signature from directory
    fn load_signature(&self, plugin_dir: &Path) -> PluginResult<Option<PluginSignature>> {
        let sig_path = plugin_dir.join("plugin.sig");

        if !sig_path.exists() {
            return Ok(None);
        }

        let sig_content = std::fs::read_to_string(&sig_path)?;
        let sig_data: serde_json::Value = serde_json::from_str(&sig_content)
            .map_err(|e| PluginError::SignatureError(e.to_string()))?;

        let public_key = sig_data
            .get("public_key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| PluginError::SignatureError("Missing public_key".to_string()))?
            .to_string();

        let signature = sig_data
            .get("signature")
            .and_then(|v| v.as_str())
            .ok_or_else(|| PluginError::SignatureError("Missing signature".to_string()))?
            .to_string();

        let algorithm = sig_data
            .get("algorithm")
            .and_then(|v| v.as_str())
            .unwrap_or("ed25519");

        let algorithm = match algorithm {
            "ed25519" => SignatureAlgorithm::Ed25519,
            other => {
                return Err(PluginError::SignatureError(format!(
                    "Unsupported algorithm: {}",
                    other
                )))
            }
        };

        Ok(Some(PluginSignature {
            public_key,
            signature,
            algorithm,
        }))
    }

    /// Verify a plugin signature
    fn verify_signature(&self, wasm_bytes: &[u8], sig: &PluginSignature) -> PluginResult<()> {
        // Check if signer is trusted
        if !self.trusted_signers.is_empty() && !self.trusted_signers.contains(&sig.public_key) {
            return Err(PluginError::SignatureError(format!(
                "Signer not trusted: {}",
                sig.public_key
            )));
        }

        // TODO: Implement actual Ed25519 signature verification
        // For now, we just check that the signature format is valid
        match sig.algorithm {
            SignatureAlgorithm::Ed25519 => {
                // Signature should be 128 hex chars (64 bytes)
                if sig.signature.len() != 128 {
                    return Err(PluginError::SignatureError(
                        "Invalid signature length".to_string(),
                    ));
                }
                // Public key should be 64 hex chars (32 bytes)
                if sig.public_key.len() != 64 {
                    return Err(PluginError::SignatureError(
                        "Invalid public key length".to_string(),
                    ));
                }
            }
        }

        // Placeholder: actual verification would use ed25519-dalek or similar
        debug!(
            "Signature verification placeholder - would verify {} bytes with key {}",
            wasm_bytes.len(),
            &sig.public_key[..16]
        );

        Ok(())
    }

    /// Load a plugin into the sandbox
    pub fn load_into_sandbox(
        &self,
        package: &PluginPackage,
        sandbox: &PluginSandbox,
    ) -> PluginResult<PluginInstance> {
        let wasm_bytes = std::fs::read(&package.wasm_path)?;

        // Verify hash hasn't changed
        let current_hash = compute_sha256(&wasm_bytes);
        if current_hash != package.wasm_hash {
            return Err(PluginError::HashMismatch {
                expected: package.wasm_hash.clone(),
                actual: current_hash,
            });
        }

        // Compile the module
        let module = sandbox.compile_module(&wasm_bytes)?;

        Ok(PluginInstance::new(module, package.manifest.id.clone()))
    }

    /// Install a plugin from a zip file
    pub fn install_from_zip(&self, zip_path: &Path) -> PluginResult<PluginPackage> {
        // Read zip file
        let zip_bytes = std::fs::read(zip_path)?;
        self.install_from_bytes(&zip_bytes)
    }

    /// Install a plugin from bytes (zip archive)
    pub fn install_from_bytes(&self, zip_bytes: &[u8]) -> PluginResult<PluginPackage> {
        use std::io::{Cursor, Read};

        let cursor = Cursor::new(zip_bytes);
        let mut archive = zip::ZipArchive::new(cursor)
            .map_err(|e| PluginError::InvalidManifest(format!("Invalid zip: {}", e)))?;

        // First, find and parse the manifest to get the plugin ID
        let manifest: PluginManifest = {
            let mut manifest_file = archive.by_name("manifest.json").map_err(|_| {
                PluginError::InvalidManifest("manifest.json not found in archive".to_string())
            })?;

            let mut content = String::new();
            manifest_file.read_to_string(&mut content)?;

            serde_json::from_str(&content)
                .map_err(|e| PluginError::InvalidManifest(e.to_string()))?
        };

        manifest
            .validate()
            .map_err(|e| PluginError::InvalidManifest(e.to_string()))?;

        // Create plugin directory
        let plugin_dir = self.plugins_dir.join(manifest.slug());
        std::fs::create_dir_all(&plugin_dir)?;

        // Extract all files
        let mut archive = zip::ZipArchive::new(Cursor::new(zip_bytes))
            .map_err(|e| PluginError::InvalidManifest(format!("Invalid zip: {}", e)))?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i).map_err(|e| {
                PluginError::IoError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                ))
            })?;

            let outpath = plugin_dir.join(file.name());

            if file.is_dir() {
                std::fs::create_dir_all(&outpath)?;
            } else {
                if let Some(parent) = outpath.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut outfile = std::fs::File::create(&outpath)?;
                std::io::copy(&mut file, &mut outfile)?;
            }
        }

        // Load the installed package
        self.load_plugin_package(&plugin_dir)
    }

    /// Uninstall a plugin
    pub fn uninstall(&self, plugin_id: &str) -> PluginResult<()> {
        // Find the plugin directory
        let entries = std::fs::read_dir(&self.plugins_dir)?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let manifest_path = path.join("manifest.json");
                if manifest_path.exists() {
                    if let Ok(content) = std::fs::read_to_string(&manifest_path) {
                        if let Ok(manifest) = serde_json::from_str::<PluginManifest>(&content) {
                            if manifest.id == plugin_id {
                                std::fs::remove_dir_all(&path)?;
                                info!("Uninstalled plugin: {}", plugin_id);
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }

        Err(PluginError::NotFound(plugin_id.to_string()))
    }
}

/// Compute SHA256 hash of bytes
fn compute_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    hex::encode(result)
}

/// Validate WASM module structure
pub fn validate_wasm(wasm_bytes: &[u8]) -> PluginResult<()> {
    // Check WASM magic number
    if wasm_bytes.len() < 8 {
        return Err(PluginError::InvalidManifest("WASM too small".to_string()));
    }

    // WASM magic: 0x00 0x61 0x73 0x6D ("\0asm")
    if &wasm_bytes[0..4] != b"\0asm" {
        return Err(PluginError::InvalidManifest(
            "Invalid WASM magic number".to_string(),
        ));
    }

    // Check version (should be 1)
    let version = u32::from_le_bytes([wasm_bytes[4], wasm_bytes[5], wasm_bytes[6], wasm_bytes[7]]);
    if version != 1 {
        return Err(PluginError::InvalidManifest(format!(
            "Unsupported WASM version: {}",
            version
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_compute_sha256() {
        let hash = compute_sha256(b"hello world");
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_validate_wasm() {
        // Valid WASM header
        let valid_wasm = [0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];
        assert!(validate_wasm(&valid_wasm).is_ok());

        // Invalid magic
        let invalid_magic = [0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00];
        assert!(validate_wasm(&invalid_magic).is_err());

        // Too short
        let too_short = [0x00, 0x61, 0x73];
        assert!(validate_wasm(&too_short).is_err());
    }

    #[test]
    fn test_plugin_loader_creation() {
        let dir = tempdir().unwrap();
        let loader = PluginLoader::new(dir.path())
            .require_signatures(true)
            .add_trusted_signer("abc123");

        assert!(loader.require_signatures);
        assert_eq!(loader.trusted_signers.len(), 1);
    }
}
