//! Local storage manager for robot data collection
//!
//! Manages episode storage on local disk with retention policies,
//! indexing, and cleanup functionality.

use super::metadata::EpisodeMetadata;
use super::packager::PackageResult;
use super::types::{EpisodeId, TimestampNs};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Storage-related errors
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Episode not found: {0}")]
    EpisodeNotFound(String),

    #[error("Index corrupted: {0}")]
    IndexCorrupted(String),

    #[error("Storage full: used {used_bytes} of {max_bytes} bytes")]
    StorageFull { used_bytes: u64, max_bytes: u64 },

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("Episode already exists: {0}")]
    EpisodeExists(String),
}

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Root directory for episode storage
    pub root_dir: PathBuf,
    /// Maximum storage size in bytes (0 = unlimited)
    #[serde(default)]
    pub max_storage_bytes: u64,
    /// Maximum number of episodes to keep (0 = unlimited)
    #[serde(default)]
    pub max_episodes: u32,
    /// Maximum age for episodes in seconds (0 = unlimited)
    #[serde(default)]
    pub max_age_seconds: u64,
    /// Whether to auto-cleanup when limits are exceeded
    #[serde(default = "default_true")]
    pub auto_cleanup: bool,
    /// Cleanup strategy when limits are exceeded
    #[serde(default)]
    pub cleanup_strategy: CleanupStrategy,
    /// Whether to maintain an index file
    #[serde(default = "default_true")]
    pub maintain_index: bool,
    /// Index file name
    #[serde(default = "default_index_file")]
    pub index_file: String,
}

fn default_true() -> bool {
    true
}

fn default_index_file() -> String {
    "episode_index.json".to_string()
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            root_dir: PathBuf::from("episodes"),
            max_storage_bytes: 0, // Unlimited
            max_episodes: 0,      // Unlimited
            max_age_seconds: 0,   // Unlimited
            auto_cleanup: true,
            cleanup_strategy: CleanupStrategy::OldestFirst,
            maintain_index: true,
            index_file: default_index_file(),
        }
    }
}

impl StorageConfig {
    /// Create a new storage config with root directory
    pub fn new(root_dir: impl Into<PathBuf>) -> Self {
        Self {
            root_dir: root_dir.into(),
            ..Default::default()
        }
    }

    /// Set maximum storage size
    pub fn with_max_size(mut self, bytes: u64) -> Self {
        self.max_storage_bytes = bytes;
        self
    }

    /// Set maximum episode count
    pub fn with_max_episodes(mut self, count: u32) -> Self {
        self.max_episodes = count;
        self
    }

    /// Set maximum age in seconds
    pub fn with_max_age(mut self, seconds: u64) -> Self {
        self.max_age_seconds = seconds;
        self
    }

    /// Set cleanup strategy
    pub fn with_cleanup_strategy(mut self, strategy: CleanupStrategy) -> Self {
        self.cleanup_strategy = strategy;
        self
    }
}

/// Strategy for cleaning up old episodes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CleanupStrategy {
    /// Delete oldest episodes first
    #[default]
    OldestFirst,
    /// Delete largest episodes first
    LargestFirst,
    /// Delete lowest quality episodes first
    LowestQualityFirst,
    /// Delete already-uploaded episodes first
    UploadedFirst,
}

/// Episode index entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodeIndexEntry {
    /// Episode ID
    pub episode_id: EpisodeId,
    /// Path to episode directory (relative to root)
    pub path: PathBuf,
    /// MCAP file path (relative to episode directory)
    pub mcap_file: String,
    /// Metadata file path (optional)
    pub metadata_file: Option<String>,
    /// Episode start timestamp
    pub start_time_ns: TimestampNs,
    /// Episode end timestamp
    pub end_time_ns: TimestampNs,
    /// Duration in seconds
    pub duration_seconds: f64,
    /// Total size in bytes
    pub size_bytes: u64,
    /// Sample count
    pub sample_count: u64,
    /// Robot ID
    pub robot_id: String,
    /// Whether episode has been uploaded
    #[serde(default)]
    pub uploaded: bool,
    /// Upload timestamp (if uploaded)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uploaded_at: Option<TimestampNs>,
    /// Upload destination (if uploaded)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upload_destination: Option<String>,
    /// Quality score
    #[serde(default = "default_quality")]
    pub quality_score: f32,
    /// Created at timestamp
    pub created_at: TimestampNs,
    /// Labels/tags
    #[serde(default)]
    pub labels: HashMap<String, String>,
}

fn default_quality() -> f32 {
    1.0
}

impl EpisodeIndexEntry {
    /// Create from package result and metadata
    pub fn from_package_result(
        result: &PackageResult,
        metadata: &EpisodeMetadata,
        root_dir: &Path,
    ) -> Self {
        let path = result
            .mcap_path
            .parent()
            .and_then(|p| p.strip_prefix(root_dir).ok())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from(result.episode_id.as_str()));

        let mcap_file = result
            .mcap_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("episode.mcap")
            .to_string();

        let metadata_file = result
            .metadata_path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(|s| s.to_string());

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        Self {
            episode_id: result.episode_id.clone(),
            path,
            mcap_file,
            metadata_file,
            start_time_ns: metadata.start_time_ns,
            end_time_ns: metadata.end_time_ns,
            duration_seconds: metadata.duration_seconds,
            size_bytes: result.file_size_bytes,
            sample_count: result.total_samples,
            robot_id: metadata.robot_id.clone(),
            uploaded: false,
            uploaded_at: None,
            upload_destination: None,
            quality_score: metadata.quality.overall_score,
            created_at: now,
            labels: metadata.labels.clone(),
        }
    }

    /// Get full path to MCAP file
    pub fn mcap_path(&self, root_dir: &Path) -> PathBuf {
        root_dir.join(&self.path).join(&self.mcap_file)
    }

    /// Get full path to metadata file (if exists)
    pub fn metadata_path(&self, root_dir: &Path) -> Option<PathBuf> {
        self.metadata_file
            .as_ref()
            .map(|f| root_dir.join(&self.path).join(f))
    }

    /// Mark as uploaded
    pub fn mark_uploaded(&mut self, destination: impl Into<String>) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        self.uploaded = true;
        self.uploaded_at = Some(now);
        self.upload_destination = Some(destination.into());
    }

    /// Get age in seconds
    pub fn age_seconds(&self) -> u64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        (now.saturating_sub(self.created_at)) / 1_000_000_000
    }
}

/// Episode index for quick lookup
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EpisodeIndex {
    /// Schema version
    #[serde(default = "default_index_version")]
    pub version: String,
    /// Last updated timestamp
    pub last_updated: TimestampNs,
    /// Total size of all episodes
    pub total_size_bytes: u64,
    /// Episode count
    pub episode_count: u32,
    /// Episodes indexed by ID
    pub episodes: HashMap<String, EpisodeIndexEntry>,
}

fn default_index_version() -> String {
    "1.0".to_string()
}

impl EpisodeIndex {
    /// Create a new empty index
    pub fn new() -> Self {
        Self {
            version: default_index_version(),
            last_updated: 0,
            total_size_bytes: 0,
            episode_count: 0,
            episodes: HashMap::new(),
        }
    }

    /// Add an episode entry
    pub fn add(&mut self, entry: EpisodeIndexEntry) {
        self.total_size_bytes += entry.size_bytes;
        self.episodes
            .insert(entry.episode_id.as_str().to_string(), entry);
        self.episode_count = self.episodes.len() as u32;
        self.update_timestamp();
    }

    /// Remove an episode entry
    pub fn remove(&mut self, episode_id: &str) -> Option<EpisodeIndexEntry> {
        if let Some(entry) = self.episodes.remove(episode_id) {
            self.total_size_bytes = self.total_size_bytes.saturating_sub(entry.size_bytes);
            self.episode_count = self.episodes.len() as u32;
            self.update_timestamp();
            Some(entry)
        } else {
            None
        }
    }

    /// Get an episode entry
    pub fn get(&self, episode_id: &str) -> Option<&EpisodeIndexEntry> {
        self.episodes.get(episode_id)
    }

    /// Get mutable episode entry
    pub fn get_mut(&mut self, episode_id: &str) -> Option<&mut EpisodeIndexEntry> {
        self.episodes.get_mut(episode_id)
    }

    /// Update timestamp
    fn update_timestamp(&mut self) {
        self.last_updated = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
    }

    /// Get episodes sorted by creation time (oldest first)
    pub fn sorted_by_age(&self) -> Vec<&EpisodeIndexEntry> {
        let mut entries: Vec<_> = self.episodes.values().collect();
        entries.sort_by_key(|e| e.created_at);
        entries
    }

    /// Get episodes sorted by size (largest first)
    pub fn sorted_by_size(&self) -> Vec<&EpisodeIndexEntry> {
        let mut entries: Vec<_> = self.episodes.values().collect();
        entries.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
        entries
    }

    /// Get episodes sorted by quality (lowest first)
    pub fn sorted_by_quality(&self) -> Vec<&EpisodeIndexEntry> {
        let mut entries: Vec<_> = self.episodes.values().collect();
        entries.sort_by(|a, b| {
            a.quality_score
                .partial_cmp(&b.quality_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        entries
    }

    /// Get uploaded episodes sorted by age
    pub fn uploaded_episodes(&self) -> Vec<&EpisodeIndexEntry> {
        let mut entries: Vec<_> = self.episodes.values().filter(|e| e.uploaded).collect();
        entries.sort_by_key(|e| e.created_at);
        entries
    }

    /// Get episodes pending upload
    pub fn pending_upload(&self) -> Vec<&EpisodeIndexEntry> {
        self.episodes.values().filter(|e| !e.uploaded).collect()
    }

    /// Filter episodes by robot ID
    pub fn by_robot(&self, robot_id: &str) -> Vec<&EpisodeIndexEntry> {
        self.episodes
            .values()
            .filter(|e| e.robot_id == robot_id)
            .collect()
    }

    /// Filter episodes by label
    pub fn by_label(&self, key: &str, value: &str) -> Vec<&EpisodeIndexEntry> {
        self.episodes
            .values()
            .filter(|e| e.labels.get(key).map(|v| v == value).unwrap_or(false))
            .collect()
    }

    /// Get episodes within time range
    pub fn in_time_range(
        &self,
        start_ns: TimestampNs,
        end_ns: TimestampNs,
    ) -> Vec<&EpisodeIndexEntry> {
        self.episodes
            .values()
            .filter(|e| e.start_time_ns >= start_ns && e.end_time_ns <= end_ns)
            .collect()
    }
}

/// Local storage manager
///
/// Manages episode storage with retention policies and indexing.
pub struct StorageManager {
    /// Configuration
    config: StorageConfig,
    /// Episode index
    index: EpisodeIndex,
    /// Whether index needs saving
    index_dirty: bool,
}

impl StorageManager {
    /// Create a new storage manager
    pub fn new(config: StorageConfig) -> Result<Self, StorageError> {
        // Create root directory if it doesn't exist
        fs::create_dir_all(&config.root_dir)?;

        // Load or create index
        let index = Self::load_index(&config)?;

        Ok(Self {
            config,
            index,
            index_dirty: false,
        })
    }

    /// Load index from disk
    fn load_index(config: &StorageConfig) -> Result<EpisodeIndex, StorageError> {
        if !config.maintain_index {
            return Ok(EpisodeIndex::new());
        }

        let index_path = config.root_dir.join(&config.index_file);

        if index_path.exists() {
            let file = File::open(&index_path)?;
            let reader = BufReader::new(file);
            let index: EpisodeIndex = serde_json::from_reader(reader)?;
            Ok(index)
        } else {
            Ok(EpisodeIndex::new())
        }
    }

    /// Save index to disk
    pub fn save_index(&mut self) -> Result<(), StorageError> {
        if !self.config.maintain_index {
            return Ok(());
        }

        let index_path = self.config.root_dir.join(&self.config.index_file);
        let file = File::create(&index_path)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, &self.index)?;
        self.index_dirty = false;
        Ok(())
    }

    /// Get configuration
    pub fn config(&self) -> &StorageConfig {
        &self.config
    }

    /// Get index
    pub fn index(&self) -> &EpisodeIndex {
        &self.index
    }

    /// Get mutable index
    pub fn index_mut(&mut self) -> &mut EpisodeIndex {
        self.index_dirty = true;
        &mut self.index
    }

    /// Store a packaged episode
    pub fn store_episode(
        &mut self,
        result: &PackageResult,
        metadata: &EpisodeMetadata,
    ) -> Result<EpisodeIndexEntry, StorageError> {
        // Check if episode already exists
        if self.index.episodes.contains_key(result.episode_id.as_str()) {
            return Err(StorageError::EpisodeExists(result.episode_id.to_string()));
        }

        // Check storage limits and cleanup if needed
        if self.config.auto_cleanup {
            self.enforce_limits(result.file_size_bytes)?;
        } else {
            // Just check limits
            self.check_limits(result.file_size_bytes)?;
        }

        // Create index entry
        let entry = EpisodeIndexEntry::from_package_result(result, metadata, &self.config.root_dir);

        // Add to index
        self.index.add(entry.clone());
        self.index_dirty = true;

        // Save index
        if self.config.maintain_index {
            self.save_index()?;
        }

        Ok(entry)
    }

    /// Check storage limits without cleanup
    fn check_limits(&self, additional_bytes: u64) -> Result<(), StorageError> {
        // Check size limit
        if self.config.max_storage_bytes > 0 {
            let new_total = self.index.total_size_bytes + additional_bytes;
            if new_total > self.config.max_storage_bytes {
                return Err(StorageError::StorageFull {
                    used_bytes: new_total,
                    max_bytes: self.config.max_storage_bytes,
                });
            }
        }

        // Check episode count
        if self.config.max_episodes > 0 && self.index.episode_count >= self.config.max_episodes {
            return Err(StorageError::StorageFull {
                used_bytes: self.index.total_size_bytes,
                max_bytes: self.config.max_storage_bytes,
            });
        }

        Ok(())
    }

    /// Enforce storage limits by cleaning up old episodes
    fn enforce_limits(&mut self, additional_bytes: u64) -> Result<(), StorageError> {
        // Cleanup expired episodes first
        self.cleanup_expired()?;

        // Check size limit
        while self.config.max_storage_bytes > 0 {
            let new_total = self.index.total_size_bytes + additional_bytes;
            if new_total <= self.config.max_storage_bytes {
                break;
            }
            if !self.cleanup_one()? {
                return Err(StorageError::StorageFull {
                    used_bytes: new_total,
                    max_bytes: self.config.max_storage_bytes,
                });
            }
        }

        // Check episode count
        while self.config.max_episodes > 0 && self.index.episode_count >= self.config.max_episodes {
            if !self.cleanup_one()? {
                return Err(StorageError::StorageFull {
                    used_bytes: self.index.total_size_bytes,
                    max_bytes: self.config.max_storage_bytes,
                });
            }
        }

        Ok(())
    }

    /// Cleanup one episode based on strategy
    fn cleanup_one(&mut self) -> Result<bool, StorageError> {
        let to_delete = match self.config.cleanup_strategy {
            CleanupStrategy::OldestFirst => self
                .index
                .sorted_by_age()
                .first()
                .map(|e| e.episode_id.clone()),
            CleanupStrategy::LargestFirst => self
                .index
                .sorted_by_size()
                .first()
                .map(|e| e.episode_id.clone()),
            CleanupStrategy::LowestQualityFirst => self
                .index
                .sorted_by_quality()
                .first()
                .map(|e| e.episode_id.clone()),
            CleanupStrategy::UploadedFirst => {
                // Prefer uploaded episodes, then fall back to oldest
                let uploaded = self.index.uploaded_episodes();
                if let Some(entry) = uploaded.first() {
                    Some(entry.episode_id.clone())
                } else {
                    self.index
                        .sorted_by_age()
                        .first()
                        .map(|e| e.episode_id.clone())
                }
            }
        };

        if let Some(episode_id) = to_delete {
            self.delete_episode(episode_id.as_str())?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Cleanup expired episodes
    pub fn cleanup_expired(&mut self) -> Result<u32, StorageError> {
        if self.config.max_age_seconds == 0 {
            return Ok(0);
        }

        let mut deleted = 0;
        let expired: Vec<_> = self
            .index
            .episodes
            .values()
            .filter(|e| e.age_seconds() > self.config.max_age_seconds)
            .map(|e| e.episode_id.clone())
            .collect();

        for episode_id in expired {
            self.delete_episode(episode_id.as_str())?;
            deleted += 1;
        }

        Ok(deleted)
    }

    /// Delete an episode
    pub fn delete_episode(&mut self, episode_id: &str) -> Result<(), StorageError> {
        let entry = self
            .index
            .remove(episode_id)
            .ok_or_else(|| StorageError::EpisodeNotFound(episode_id.to_string()))?;

        // Delete files
        let episode_dir = self.config.root_dir.join(&entry.path);
        if episode_dir.exists() {
            fs::remove_dir_all(&episode_dir)?;
        }

        self.index_dirty = true;

        // Save index
        if self.config.maintain_index {
            self.save_index()?;
        }

        Ok(())
    }

    /// Get episode by ID
    pub fn get_episode(&self, episode_id: &str) -> Option<&EpisodeIndexEntry> {
        self.index.get(episode_id)
    }

    /// Get episode metadata from disk
    pub fn load_episode_metadata(&self, episode_id: &str) -> Result<EpisodeMetadata, StorageError> {
        let entry = self
            .index
            .get(episode_id)
            .ok_or_else(|| StorageError::EpisodeNotFound(episode_id.to_string()))?;

        let metadata_path = entry.metadata_path(&self.config.root_dir).ok_or_else(|| {
            StorageError::EpisodeNotFound("Metadata file not available".to_string())
        })?;

        let file = File::open(&metadata_path)?;
        let reader = BufReader::new(file);
        let metadata: EpisodeMetadata = serde_json::from_reader(reader)?;
        Ok(metadata)
    }

    /// Mark episode as uploaded
    pub fn mark_uploaded(
        &mut self,
        episode_id: &str,
        destination: impl Into<String>,
    ) -> Result<(), StorageError> {
        let entry = self
            .index
            .get_mut(episode_id)
            .ok_or_else(|| StorageError::EpisodeNotFound(episode_id.to_string()))?;

        entry.mark_uploaded(destination);
        self.index_dirty = true;

        if self.config.maintain_index {
            self.save_index()?;
        }

        Ok(())
    }

    /// Get total storage size
    pub fn total_size_bytes(&self) -> u64 {
        self.index.total_size_bytes
    }

    /// Get episode count
    pub fn episode_count(&self) -> u32 {
        self.index.episode_count
    }

    /// Get storage usage statistics
    pub fn stats(&self) -> StorageStats {
        let pending_upload = self.index.pending_upload().len() as u32;
        let uploaded = self.index.episode_count.saturating_sub(pending_upload);

        StorageStats {
            total_episodes: self.index.episode_count,
            total_size_bytes: self.index.total_size_bytes,
            pending_upload,
            uploaded,
            max_size_bytes: self.config.max_storage_bytes,
            max_episodes: self.config.max_episodes,
            usage_percent: if self.config.max_storage_bytes > 0 {
                (self.index.total_size_bytes as f32 / self.config.max_storage_bytes as f32) * 100.0
            } else {
                0.0
            },
        }
    }

    /// List all episode IDs
    pub fn list_episodes(&self) -> Vec<&EpisodeId> {
        self.index
            .episodes
            .values()
            .map(|e| &e.episode_id)
            .collect()
    }

    /// Scan directory and rebuild index
    pub fn rebuild_index(&mut self) -> Result<u32, StorageError> {
        let mut found = 0;
        self.index = EpisodeIndex::new();

        // Scan for episode directories
        for entry in fs::read_dir(&self.config.root_dir)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            // Skip index file
            if path.file_name().and_then(|n| n.to_str()) == Some(&self.config.index_file) {
                continue;
            }

            // Look for metadata file
            if let Some(episode_entry) = self.scan_episode_dir(&path)? {
                self.index.add(episode_entry);
                found += 1;
            }
        }

        self.index_dirty = true;
        self.save_index()?;

        Ok(found)
    }

    /// Scan an episode directory for metadata
    fn scan_episode_dir(&self, dir: &Path) -> Result<Option<EpisodeIndexEntry>, StorageError> {
        // Find MCAP file
        let mcap_files: Vec<_> = fs::read_dir(dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("mcap"))
            .collect();

        if mcap_files.is_empty() {
            return Ok(None);
        }

        let mcap_path = mcap_files[0].path();
        let mcap_file = mcap_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("episode.mcap")
            .to_string();

        // Find metadata file
        let metadata_files: Vec<_> = fs::read_dir(dir)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path().extension().and_then(|s| s.to_str()) == Some("json")
                    && e.path()
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.ends_with("_metadata"))
                        .unwrap_or(false)
            })
            .collect();

        let metadata_file = metadata_files.first().and_then(|e| {
            let path = e.path();
            path.file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
        });

        // Get episode ID from directory name
        let episode_id = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Get file size
        let size_bytes = fs::metadata(&mcap_path)?.len();

        // Try to load metadata for more info
        let (_metadata, start_time, end_time, duration, robot_id, quality_score, labels) =
            if let Some(ref meta_file) = metadata_file {
                let meta_path = dir.join(meta_file);
                if meta_path.exists() {
                    match File::open(&meta_path) {
                        Ok(f) => {
                            let reader = BufReader::new(f);
                            match serde_json::from_reader::<_, EpisodeMetadata>(reader) {
                                Ok(m) => (
                                    Some(m.clone()),
                                    m.start_time_ns,
                                    m.end_time_ns,
                                    m.duration_seconds,
                                    m.robot_id,
                                    m.quality.overall_score,
                                    m.labels,
                                ),
                                Err(_) => {
                                    (None, 0, 0, 0.0, "unknown".to_string(), 1.0, HashMap::new())
                                }
                            }
                        }
                        Err(_) => (None, 0, 0, 0.0, "unknown".to_string(), 1.0, HashMap::new()),
                    }
                } else {
                    (None, 0, 0, 0.0, "unknown".to_string(), 1.0, HashMap::new())
                }
            } else {
                (None, 0, 0, 0.0, "unknown".to_string(), 1.0, HashMap::new())
            };

        let path = dir
            .strip_prefix(&self.config.root_dir)
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|_| PathBuf::from(&episode_id));

        // Get created time from directory
        let created_at = fs::metadata(dir)?
            .created()
            .or_else(|_| fs::metadata(dir)?.modified())
            .map(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos() as u64
            })
            .unwrap_or(0);

        Ok(Some(EpisodeIndexEntry {
            episode_id: EpisodeId::from_string(episode_id),
            path,
            mcap_file,
            metadata_file,
            start_time_ns: start_time,
            end_time_ns: end_time,
            duration_seconds: duration,
            size_bytes,
            sample_count: 0, // Not available without parsing MCAP
            robot_id,
            uploaded: false,
            uploaded_at: None,
            upload_destination: None,
            quality_score,
            created_at,
            labels,
        }))
    }

    /// Flush any pending changes to disk
    pub fn flush(&mut self) -> Result<(), StorageError> {
        if self.index_dirty {
            self.save_index()?;
        }
        Ok(())
    }
}

impl Drop for StorageManager {
    fn drop(&mut self) {
        // Try to save index on drop
        let _ = self.flush();
    }
}

/// Storage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStats {
    /// Total episodes stored
    pub total_episodes: u32,
    /// Total storage size in bytes
    pub total_size_bytes: u64,
    /// Episodes pending upload
    pub pending_upload: u32,
    /// Episodes already uploaded
    pub uploaded: u32,
    /// Maximum storage size (0 = unlimited)
    pub max_size_bytes: u64,
    /// Maximum episodes (0 = unlimited)
    pub max_episodes: u32,
    /// Usage percentage (0-100)
    pub usage_percent: f32,
}

impl StorageStats {
    /// Get formatted total size
    pub fn formatted_size(&self) -> String {
        format_bytes(self.total_size_bytes)
    }

    /// Get formatted max size
    pub fn formatted_max_size(&self) -> String {
        if self.max_size_bytes == 0 {
            "Unlimited".to_string()
        } else {
            format_bytes(self.max_size_bytes)
        }
    }
}

/// Format bytes as human-readable string
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_config_default() {
        let config = StorageConfig::default();
        assert_eq!(config.max_storage_bytes, 0);
        assert_eq!(config.max_episodes, 0);
        assert!(config.auto_cleanup);
        assert!(config.maintain_index);
    }

    #[test]
    fn test_storage_config_builder() {
        let config = StorageConfig::new("/tmp/episodes")
            .with_max_size(10 * 1024 * 1024 * 1024) // 10 GB
            .with_max_episodes(1000)
            .with_max_age(86400 * 30) // 30 days
            .with_cleanup_strategy(CleanupStrategy::UploadedFirst);

        assert_eq!(config.root_dir, PathBuf::from("/tmp/episodes"));
        assert_eq!(config.max_storage_bytes, 10 * 1024 * 1024 * 1024);
        assert_eq!(config.max_episodes, 1000);
        assert_eq!(config.max_age_seconds, 86400 * 30);
        assert_eq!(config.cleanup_strategy, CleanupStrategy::UploadedFirst);
    }

    #[test]
    fn test_episode_index_operations() {
        let mut index = EpisodeIndex::new();
        assert_eq!(index.episode_count, 0);

        let entry = EpisodeIndexEntry {
            episode_id: EpisodeId::from_string("test-001"),
            path: PathBuf::from("test-001"),
            mcap_file: "test-001.mcap".to_string(),
            metadata_file: Some("test-001_metadata.json".to_string()),
            start_time_ns: 1000,
            end_time_ns: 2000,
            duration_seconds: 1.0,
            size_bytes: 1024,
            sample_count: 100,
            robot_id: "robot-001".to_string(),
            uploaded: false,
            uploaded_at: None,
            upload_destination: None,
            quality_score: 0.95,
            created_at: 0,
            labels: HashMap::new(),
        };

        index.add(entry.clone());
        assert_eq!(index.episode_count, 1);
        assert_eq!(index.total_size_bytes, 1024);

        let retrieved = index.get("test-001");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().robot_id, "robot-001");

        let removed = index.remove("test-001");
        assert!(removed.is_some());
        assert_eq!(index.episode_count, 0);
        assert_eq!(index.total_size_bytes, 0);
    }

    #[test]
    fn test_episode_index_sorting() {
        let mut index = EpisodeIndex::new();

        // Add episodes with different sizes and creation times
        for i in 0..5 {
            let entry = EpisodeIndexEntry {
                episode_id: EpisodeId::from_string(format!("ep-{}", i)),
                path: PathBuf::from(format!("ep-{}", i)),
                mcap_file: format!("ep-{}.mcap", i),
                metadata_file: None,
                start_time_ns: 0,
                end_time_ns: 0,
                duration_seconds: 0.0,
                size_bytes: (i + 1) as u64 * 1000,
                sample_count: 0,
                robot_id: "robot".to_string(),
                uploaded: i % 2 == 0,
                uploaded_at: None,
                upload_destination: None,
                quality_score: (5 - i) as f32 / 5.0,
                created_at: i as u64 * 1000,
                labels: HashMap::new(),
            };
            index.add(entry);
        }

        // Test sorting by age
        let by_age = index.sorted_by_age();
        assert_eq!(by_age[0].episode_id.as_str(), "ep-0");
        assert_eq!(by_age[4].episode_id.as_str(), "ep-4");

        // Test sorting by size
        let by_size = index.sorted_by_size();
        assert_eq!(by_size[0].episode_id.as_str(), "ep-4"); // Largest
        assert_eq!(by_size[4].episode_id.as_str(), "ep-0"); // Smallest

        // Test sorting by quality
        let by_quality = index.sorted_by_quality();
        assert_eq!(by_quality[0].episode_id.as_str(), "ep-4"); // Lowest quality

        // Test uploaded filter
        let uploaded = index.uploaded_episodes();
        assert_eq!(uploaded.len(), 3); // ep-0, ep-2, ep-4
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.00 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.00 GB");
        assert_eq!(format_bytes(1024 * 1024 * 1024 * 1024), "1.00 TB");
    }

    #[test]
    fn test_cleanup_strategy() {
        assert_eq!(CleanupStrategy::default(), CleanupStrategy::OldestFirst);
    }

    #[test]
    fn test_storage_stats() {
        let stats = StorageStats {
            total_episodes: 100,
            total_size_bytes: 5 * 1024 * 1024 * 1024, // 5 GB
            pending_upload: 20,
            uploaded: 80,
            max_size_bytes: 10 * 1024 * 1024 * 1024, // 10 GB
            max_episodes: 500,
            usage_percent: 50.0,
        };

        assert_eq!(stats.formatted_size(), "5.00 GB");
        assert_eq!(stats.formatted_max_size(), "10.00 GB");
    }

    #[test]
    fn test_mark_uploaded() {
        let mut entry = EpisodeIndexEntry {
            episode_id: EpisodeId::from_string("test-001"),
            path: PathBuf::from("test-001"),
            mcap_file: "test-001.mcap".to_string(),
            metadata_file: None,
            start_time_ns: 0,
            end_time_ns: 0,
            duration_seconds: 0.0,
            size_bytes: 1024,
            sample_count: 0,
            robot_id: "robot".to_string(),
            uploaded: false,
            uploaded_at: None,
            upload_destination: None,
            quality_score: 1.0,
            created_at: 0,
            labels: HashMap::new(),
        };

        assert!(!entry.uploaded);
        entry.mark_uploaded("s3://bucket/path");
        assert!(entry.uploaded);
        assert!(entry.uploaded_at.is_some());
        assert_eq!(
            entry.upload_destination,
            Some("s3://bucket/path".to_string())
        );
    }

    #[test]
    fn test_episode_paths() {
        let entry = EpisodeIndexEntry {
            episode_id: EpisodeId::from_string("test-001"),
            path: PathBuf::from("test-001"),
            mcap_file: "test-001.mcap".to_string(),
            metadata_file: Some("test-001_metadata.json".to_string()),
            start_time_ns: 0,
            end_time_ns: 0,
            duration_seconds: 0.0,
            size_bytes: 1024,
            sample_count: 0,
            robot_id: "robot".to_string(),
            uploaded: false,
            uploaded_at: None,
            upload_destination: None,
            quality_score: 1.0,
            created_at: 0,
            labels: HashMap::new(),
        };

        let root = PathBuf::from("/data/episodes");
        assert_eq!(
            entry.mcap_path(&root),
            PathBuf::from("/data/episodes/test-001/test-001.mcap")
        );
        assert_eq!(
            entry.metadata_path(&root),
            Some(PathBuf::from(
                "/data/episodes/test-001/test-001_metadata.json"
            ))
        );
    }

    #[test]
    fn test_index_filters() {
        let mut index = EpisodeIndex::new();

        let mut entry1 = EpisodeIndexEntry {
            episode_id: EpisodeId::from_string("ep-1"),
            path: PathBuf::from("ep-1"),
            mcap_file: "ep-1.mcap".to_string(),
            metadata_file: None,
            start_time_ns: 1000,
            end_time_ns: 2000,
            duration_seconds: 1.0,
            size_bytes: 1024,
            sample_count: 100,
            robot_id: "robot-A".to_string(),
            uploaded: false,
            uploaded_at: None,
            upload_destination: None,
            quality_score: 0.9,
            created_at: 0,
            labels: HashMap::new(),
        };
        entry1.labels.insert("task".to_string(), "pick".to_string());
        index.add(entry1);

        let mut entry2 = EpisodeIndexEntry {
            episode_id: EpisodeId::from_string("ep-2"),
            path: PathBuf::from("ep-2"),
            mcap_file: "ep-2.mcap".to_string(),
            metadata_file: None,
            start_time_ns: 3000,
            end_time_ns: 4000,
            duration_seconds: 1.0,
            size_bytes: 2048,
            sample_count: 200,
            robot_id: "robot-B".to_string(),
            uploaded: false,
            uploaded_at: None,
            upload_destination: None,
            quality_score: 0.8,
            created_at: 1000,
            labels: HashMap::new(),
        };
        entry2
            .labels
            .insert("task".to_string(), "place".to_string());
        index.add(entry2);

        // Test by_robot
        let robot_a = index.by_robot("robot-A");
        assert_eq!(robot_a.len(), 1);
        assert_eq!(robot_a[0].episode_id.as_str(), "ep-1");

        // Test by_label
        let pick_tasks = index.by_label("task", "pick");
        assert_eq!(pick_tasks.len(), 1);

        // Test time range
        let range = index.in_time_range(500, 2500);
        assert_eq!(range.len(), 1);
        assert_eq!(range[0].episode_id.as_str(), "ep-1");
    }
}
