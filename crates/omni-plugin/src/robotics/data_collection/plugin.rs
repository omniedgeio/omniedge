//! Data collection plugin implementation
//!
//! Main plugin that orchestrates all data collection components:
//! - Ring buffer management for continuous data capture
//! - Trigger evaluation for episode initiation
//! - Episode packaging with privacy filters
//! - Local storage with retention policies
//! - Cloud upload for data pipeline integration

use super::buffer::{BufferManager, RingBuffer};
use super::metadata::EpisodeMetadata;
use super::packager::{EpisodePackager, PackageResult, PackagerConfig, PackagerError};
use super::privacy::PrivacyConfig;
use super::storage::{StorageConfig, StorageError, StorageManager};
use super::streams::StreamConfig;
use super::triggers::{Trigger, TriggerContext, TriggerEvent, TriggerManager};
use super::types::{DataSample, EpisodeId, StreamId, TimestampNs};
use super::upload::{UploadConfig, UploadError, UploadManager};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use thiserror::Error;

/// Plugin-related errors
#[derive(Debug, Error)]
pub enum PluginError {
    #[error("Plugin not initialized")]
    NotInitialized,

    #[error("Plugin already running")]
    AlreadyRunning,

    #[error("Plugin not running")]
    NotRunning,

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Buffer error: {0}")]
    Buffer(String),

    #[error("Packager error: {0}")]
    Packager(#[from] PackagerError),

    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),

    #[error("Upload error: {0}")]
    Upload(#[from] UploadError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Episode in progress")]
    EpisodeInProgress,

    #[error("No episode in progress")]
    NoEpisodeInProgress,

    #[error("Stream not found: {0}")]
    StreamNotFound(String),
}

/// Plugin state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginState {
    /// Plugin created but not initialized
    Created,
    /// Plugin initialized and ready
    Initialized,
    /// Plugin running and collecting data
    Running,
    /// Plugin recording an episode
    Recording,
    /// Plugin packaging an episode
    Packaging,
    /// Plugin stopped
    Stopped,
    /// Plugin in error state
    Error,
}

impl Default for PluginState {
    fn default() -> Self {
        Self::Created
    }
}

/// Buffer settings for plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferSettings {
    /// Memory limit in bytes
    #[serde(default = "default_memory_limit")]
    pub memory_limit_bytes: u64,
    /// Default capacity per stream
    #[serde(default = "default_capacity")]
    pub default_capacity: usize,
    /// Maximum age in seconds
    #[serde(default = "default_max_age")]
    pub max_age_seconds: f32,
}

fn default_memory_limit() -> u64 {
    1024 * 1024 * 1024 // 1 GB
}

fn default_capacity() -> usize {
    1000
}

fn default_max_age() -> f32 {
    60.0
}

impl Default for BufferSettings {
    fn default() -> Self {
        Self {
            memory_limit_bytes: default_memory_limit(),
            default_capacity: default_capacity(),
            max_age_seconds: default_max_age(),
        }
    }
}

/// Plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataCollectionConfig {
    /// Plugin name/ID
    pub name: String,
    /// Robot identifier
    pub robot_id: String,
    /// Fleet identifier (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fleet_id: Option<String>,
    /// Stream configurations
    pub streams: Vec<StreamConfig>,
    /// Buffer configuration
    pub buffer: BufferSettings,
    /// Storage configuration
    pub storage: StorageConfig,
    /// Packager configuration
    pub packager: PackagerConfig,
    /// Privacy configuration
    pub privacy: PrivacyConfig,
    /// Upload configuration (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upload: Option<UploadConfig>,
    /// Whether to auto-upload after packaging
    #[serde(default)]
    pub auto_upload: bool,
    /// Whether to auto-start on initialization
    #[serde(default)]
    pub auto_start: bool,
    /// Default episode duration in seconds (if not specified by trigger)
    #[serde(default = "default_episode_duration")]
    pub default_episode_duration_secs: f32,
    /// Trigger cooldown in seconds
    #[serde(default = "default_trigger_cooldown")]
    pub trigger_cooldown_secs: f32,
}

fn default_episode_duration() -> f32 {
    60.0 // 1 minute
}

fn default_trigger_cooldown() -> f32 {
    5.0 // 5 seconds
}

impl Default for DataCollectionConfig {
    fn default() -> Self {
        Self {
            name: "data-collection".to_string(),
            robot_id: "robot-001".to_string(),
            fleet_id: None,
            streams: Vec::new(),
            buffer: BufferSettings::default(),
            storage: StorageConfig::default(),
            packager: PackagerConfig::default(),
            privacy: PrivacyConfig::default(),
            upload: None,
            auto_upload: false,
            auto_start: false,
            default_episode_duration_secs: default_episode_duration(),
            trigger_cooldown_secs: default_trigger_cooldown(),
        }
    }
}

impl DataCollectionConfig {
    /// Create a new configuration
    pub fn new(robot_id: impl Into<String>) -> Self {
        Self {
            robot_id: robot_id.into(),
            ..Default::default()
        }
    }

    /// Set name
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Add a stream configuration
    pub fn with_stream(mut self, stream: StreamConfig) -> Self {
        self.streams.push(stream);
        self
    }

    /// Set storage configuration
    pub fn with_storage(mut self, config: StorageConfig) -> Self {
        self.storage = config;
        self
    }

    /// Set upload configuration
    pub fn with_upload(mut self, config: UploadConfig) -> Self {
        self.upload = Some(config);
        self
    }

    /// Enable auto-upload
    pub fn with_auto_upload(mut self, enabled: bool) -> Self {
        self.auto_upload = enabled;
        self
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), PluginError> {
        if self.robot_id.is_empty() {
            return Err(PluginError::Config("Robot ID is required".into()));
        }

        if self.streams.is_empty() {
            return Err(PluginError::Config(
                "At least one stream is required".into(),
            ));
        }

        if let Some(ref upload) = self.upload {
            upload.validate().map_err(PluginError::Upload)?;
        }

        Ok(())
    }
}

/// Active episode being recorded
struct ActiveEpisode {
    /// Episode ID
    episode_id: EpisodeId,
    /// Start timestamp
    start_time_ns: TimestampNs,
    /// Expected end timestamp
    expected_end_ns: TimestampNs,
    /// Trigger that initiated this episode
    trigger_event: TriggerEvent,
    /// Metadata being built
    metadata: EpisodeMetadata,
}

/// Data collection plugin
///
/// Main plugin that coordinates all data collection functionality.
pub struct DataCollectionPlugin {
    /// Configuration
    config: DataCollectionConfig,
    /// Current state
    state: PluginState,
    /// Buffer manager for all streams
    buffer_manager: BufferManager,
    /// Trigger manager
    trigger_manager: TriggerManager,
    /// Episode packager
    packager: EpisodePackager,
    /// Storage manager
    storage_manager: Option<StorageManager>,
    /// Upload manager (optional)
    upload_manager: Option<UploadManager>,
    /// Currently active episode (if recording)
    active_episode: Option<ActiveEpisode>,
    /// Statistics
    stats: PluginStats,
    /// Whether plugin is enabled
    enabled: AtomicBool,
    /// Running flag
    running: AtomicBool,
}

impl DataCollectionPlugin {
    /// Create a new plugin with configuration
    pub fn new(config: DataCollectionConfig) -> Result<Self, PluginError> {
        config.validate()?;

        // Create buffer manager with streams
        let buffer_manager = BufferManager::new(config.buffer.memory_limit_bytes);
        for stream in &config.streams {
            buffer_manager.register_stream(
                stream.stream_id.clone(),
                Some(config.buffer.default_capacity),
                Some(config.buffer.max_age_seconds),
            );
        }

        // Create trigger manager
        let cooldown_ns = (config.trigger_cooldown_secs * 1_000_000_000.0) as u64;
        let trigger_manager = TriggerManager::new().with_cooldown(cooldown_ns);

        // Create packager
        let packager_config = PackagerConfig {
            output_dir: config.storage.root_dir.clone(),
            privacy_config: config.privacy.clone(),
            streams: config.streams.clone(),
            ..config.packager.clone()
        };
        let packager = EpisodePackager::new(packager_config);

        Ok(Self {
            config,
            state: PluginState::Created,
            buffer_manager,
            trigger_manager,
            packager,
            storage_manager: None,
            upload_manager: None,
            active_episode: None,
            stats: PluginStats::default(),
            enabled: AtomicBool::new(true),
            running: AtomicBool::new(false),
        })
    }

    /// Initialize the plugin
    pub fn initialize(&mut self) -> Result<(), PluginError> {
        if self.state != PluginState::Created {
            return Err(PluginError::Config("Plugin already initialized".into()));
        }

        // Create storage manager
        self.storage_manager = Some(StorageManager::new(self.config.storage.clone())?);

        // Create upload manager if configured
        if let Some(ref upload_config) = self.config.upload {
            self.upload_manager = Some(UploadManager::new(upload_config.clone())?);
        }

        self.state = PluginState::Initialized;

        // Auto-start if configured
        if self.config.auto_start {
            self.start()?;
        }

        Ok(())
    }

    /// Start the plugin
    pub fn start(&mut self) -> Result<(), PluginError> {
        if self.state == PluginState::Running || self.state == PluginState::Recording {
            return Err(PluginError::AlreadyRunning);
        }

        if self.state == PluginState::Created {
            self.initialize()?;
        }

        self.running.store(true, Ordering::SeqCst);
        self.state = PluginState::Running;

        Ok(())
    }

    /// Stop the plugin
    pub fn stop(&mut self) -> Result<(), PluginError> {
        if !self.running.load(Ordering::SeqCst) {
            return Err(PluginError::NotRunning);
        }

        // Finish any active episode
        if self.active_episode.is_some() {
            let now = current_timestamp_ns();
            self.finish_episode(now)?;
        }

        self.running.store(false, Ordering::SeqCst);
        self.state = PluginState::Stopped;

        // Flush storage
        if let Some(ref mut storage) = self.storage_manager {
            storage.flush()?;
        }

        Ok(())
    }

    /// Get current state
    pub fn state(&self) -> PluginState {
        self.state
    }

    /// Get configuration
    pub fn config(&self) -> &DataCollectionConfig {
        &self.config
    }

    /// Check if running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Check if recording
    pub fn is_recording(&self) -> bool {
        self.active_episode.is_some()
    }

    /// Register a trigger
    pub fn register_trigger(&mut self, trigger: Arc<dyn Trigger>) -> Result<(), PluginError> {
        self.trigger_manager
            .register(trigger)
            .map_err(|e| PluginError::Config(e.to_string()))
    }

    /// Push a data sample to the buffer
    pub fn push_sample(&mut self, sample: DataSample) -> Result<(), PluginError> {
        if !self.is_running() {
            return Err(PluginError::NotRunning);
        }

        self.buffer_manager.push(sample);
        self.stats.samples_received += 1;

        Ok(())
    }

    /// Push multiple samples
    pub fn push_samples(&mut self, samples: Vec<DataSample>) -> Result<(), PluginError> {
        if !self.is_running() {
            return Err(PluginError::NotRunning);
        }

        for sample in samples {
            self.buffer_manager.push(sample);
            self.stats.samples_received += 1;
        }

        Ok(())
    }

    /// Process triggers and check if an episode should start
    pub fn process_triggers(
        &mut self,
        context: &TriggerContext,
    ) -> Result<Vec<TriggerEvent>, PluginError> {
        if !self.is_running() {
            return Err(PluginError::NotRunning);
        }

        // Skip if already recording
        if self.is_recording() {
            return Ok(Vec::new());
        }

        let events = self.trigger_manager.evaluate(context);

        if !events.is_empty() {
            // Record trigger fired
            self.trigger_manager.record_trigger_fired(&events);

            // Start episode with highest priority trigger
            if let Some(event) = events.first() {
                self.start_episode_from_trigger(event.clone())?;
            }
        }

        Ok(events)
    }

    /// Start recording an episode from a trigger event
    fn start_episode_from_trigger(
        &mut self,
        trigger_event: TriggerEvent,
    ) -> Result<EpisodeId, PluginError> {
        let episode_id = EpisodeId::new();
        let now = trigger_event.timestamp_ns;

        // Calculate start time (with pre-roll)
        let start_time = now.saturating_sub(trigger_event.pre_roll_ns);

        // Calculate expected end time
        let expected_end = now + trigger_event.post_roll_ns;

        // Create trigger info for metadata
        let trigger_info = super::metadata::TriggerInfo {
            trigger_type: super::metadata::EpisodeTriggerType::Manual, // Default, should map from trigger_event
            trigger_time_ns: now,
            confidence: None,
            pre_buffer_seconds: trigger_event.pre_roll_ns as f32 / 1_000_000_000.0,
            post_buffer_seconds: trigger_event.post_roll_ns as f32 / 1_000_000_000.0,
            priority: super::types::Priority::Normal,
            details: trigger_event
                .metadata
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                .collect(),
        };

        // Create metadata
        let mut metadata =
            EpisodeMetadata::new(episode_id.clone(), &self.config.robot_id, trigger_info);
        metadata.start_time_ns = start_time;
        metadata.fleet_id = self.config.fleet_id.clone();

        // Create active episode
        self.active_episode = Some(ActiveEpisode {
            episode_id: episode_id.clone(),
            start_time_ns: start_time,
            expected_end_ns: expected_end,
            trigger_event,
            metadata,
        });

        self.state = PluginState::Recording;
        self.stats.episodes_started += 1;

        Ok(episode_id)
    }

    /// Manually start an episode
    pub fn start_episode_manual(
        &mut self,
        reason: impl Into<String>,
    ) -> Result<EpisodeId, PluginError> {
        if !self.is_running() {
            return Err(PluginError::NotRunning);
        }

        if self.is_recording() {
            return Err(PluginError::EpisodeInProgress);
        }

        let now = current_timestamp_ns();
        let pre_roll_ns =
            (self.config.default_episode_duration_secs * 0.25 * 1_000_000_000.0) as u64;
        let post_roll_ns =
            (self.config.default_episode_duration_secs * 0.75 * 1_000_000_000.0) as u64;

        let trigger_event = TriggerEvent::new(
            super::triggers::TriggerId::new("manual"),
            super::triggers::TriggerType::Manual,
            now,
            reason,
        )
        .with_pre_roll(pre_roll_ns)
        .with_post_roll(post_roll_ns);

        self.start_episode_from_trigger(trigger_event)
    }

    /// Finish the current episode
    pub fn finish_episode(
        &mut self,
        end_time_ns: TimestampNs,
    ) -> Result<PackageResult, PluginError> {
        let episode = self
            .active_episode
            .take()
            .ok_or(PluginError::NoEpisodeInProgress)?;

        self.state = PluginState::Packaging;

        // Update metadata with end time
        let mut metadata = episode.metadata;
        metadata.set_end_time(end_time_ns);

        // Package the episode
        let result = self.packager.package_episode(
            episode.episode_id,
            &self.buffer_manager,
            episode.start_time_ns,
            end_time_ns,
            metadata.clone(),
        )?;

        // Store in local storage
        if let Some(ref mut storage) = self.storage_manager {
            storage.store_episode(&result, &metadata)?;
        }

        // Auto-upload if configured
        if self.config.auto_upload {
            if let Some(ref mut upload) = self.upload_manager {
                if let Some(ref storage) = self.storage_manager {
                    if let Some(entry) = storage.get_episode(result.episode_id.as_str()) {
                        let _ = upload.upload_episode(entry, &self.config.storage.root_dir, None);
                    }
                }
            }
        }

        self.state = PluginState::Running;
        self.stats.episodes_completed += 1;
        self.stats.bytes_packaged += result.file_size_bytes;

        Ok(result)
    }

    /// Check if current episode should end
    pub fn check_episode_timeout(&mut self) -> Result<Option<PackageResult>, PluginError> {
        let now = current_timestamp_ns();

        let should_finish = if let Some(ref episode) = self.active_episode {
            now >= episode.expected_end_ns
        } else {
            false
        };

        if should_finish {
            Ok(Some(self.finish_episode(now)?))
        } else {
            Ok(None)
        }
    }

    /// Get buffer manager
    pub fn buffer_manager(&self) -> &BufferManager {
        &self.buffer_manager
    }

    /// Get mutable buffer manager
    pub fn buffer_manager_mut(&mut self) -> &mut BufferManager {
        &mut self.buffer_manager
    }

    /// Get trigger manager
    pub fn trigger_manager(&self) -> &TriggerManager {
        &self.trigger_manager
    }

    /// Get mutable trigger manager
    pub fn trigger_manager_mut(&mut self) -> &mut TriggerManager {
        &mut self.trigger_manager
    }

    /// Get storage manager
    pub fn storage_manager(&self) -> Option<&StorageManager> {
        self.storage_manager.as_ref()
    }

    /// Get upload manager
    pub fn upload_manager(&self) -> Option<&UploadManager> {
        self.upload_manager.as_ref()
    }

    /// Get statistics
    pub fn stats(&self) -> &PluginStats {
        &self.stats
    }

    /// Get current episode ID (if recording)
    pub fn current_episode_id(&self) -> Option<&EpisodeId> {
        self.active_episode.as_ref().map(|e| &e.episode_id)
    }

    /// Get active episode info
    pub fn active_episode_info(&self) -> Option<ActiveEpisodeInfo> {
        self.active_episode.as_ref().map(|e| ActiveEpisodeInfo {
            episode_id: e.episode_id.clone(),
            start_time_ns: e.start_time_ns,
            expected_end_ns: e.expected_end_ns,
            trigger_type: e.trigger_event.trigger_type,
            elapsed_ns: current_timestamp_ns().saturating_sub(e.start_time_ns),
            remaining_ns: e.expected_end_ns.saturating_sub(current_timestamp_ns()),
        })
    }

    /// List stream IDs
    pub fn list_streams(&self) -> Vec<StreamId> {
        self.buffer_manager.stream_ids()
    }

    /// Get buffer for a stream
    pub fn get_buffer(&self, stream_id: &StreamId) -> Option<Arc<RingBuffer>> {
        self.buffer_manager.get_buffer(stream_id)
    }
}

/// Get current timestamp in nanoseconds
fn current_timestamp_ns() -> TimestampNs {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

/// Plugin statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginStats {
    /// Total samples received
    pub samples_received: u64,
    /// Episodes started
    pub episodes_started: u64,
    /// Episodes completed
    pub episodes_completed: u64,
    /// Episodes failed
    pub episodes_failed: u64,
    /// Total bytes packaged
    pub bytes_packaged: u64,
    /// Total bytes uploaded
    pub bytes_uploaded: u64,
    /// Triggers evaluated
    pub triggers_evaluated: u64,
    /// Triggers fired
    pub triggers_fired: u64,
}

/// Active episode information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveEpisodeInfo {
    /// Episode ID
    pub episode_id: EpisodeId,
    /// Start timestamp
    pub start_time_ns: TimestampNs,
    /// Expected end timestamp
    pub expected_end_ns: TimestampNs,
    /// Trigger type
    pub trigger_type: super::triggers::TriggerType,
    /// Elapsed time in nanoseconds
    pub elapsed_ns: u64,
    /// Remaining time in nanoseconds
    pub remaining_ns: u64,
}

impl ActiveEpisodeInfo {
    /// Get elapsed time in seconds
    pub fn elapsed_seconds(&self) -> f64 {
        self.elapsed_ns as f64 / 1_000_000_000.0
    }

    /// Get remaining time in seconds
    pub fn remaining_seconds(&self) -> f64 {
        self.remaining_ns as f64 / 1_000_000_000.0
    }

    /// Get progress percentage (0-100)
    pub fn progress_percent(&self) -> f32 {
        let total = self.expected_end_ns.saturating_sub(self.start_time_ns);
        if total > 0 {
            (self.elapsed_ns as f32 / total as f32 * 100.0).min(100.0)
        } else {
            100.0
        }
    }
}

/// Plugin builder for convenient configuration
pub struct DataCollectionPluginBuilder {
    config: DataCollectionConfig,
}

impl DataCollectionPluginBuilder {
    /// Create a new builder
    pub fn new(robot_id: impl Into<String>) -> Self {
        Self {
            config: DataCollectionConfig::new(robot_id),
        }
    }

    /// Set plugin name
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.config.name = name.into();
        self
    }

    /// Set fleet ID
    pub fn fleet_id(mut self, fleet_id: impl Into<String>) -> Self {
        self.config.fleet_id = Some(fleet_id.into());
        self
    }

    /// Add a stream
    pub fn stream(mut self, stream: StreamConfig) -> Self {
        self.config.streams.push(stream);
        self
    }

    /// Add multiple streams
    pub fn streams(mut self, streams: Vec<StreamConfig>) -> Self {
        self.config.streams.extend(streams);
        self
    }

    /// Set buffer configuration
    pub fn buffer(mut self, config: BufferSettings) -> Self {
        self.config.buffer = config;
        self
    }

    /// Set storage configuration
    pub fn storage(mut self, config: StorageConfig) -> Self {
        self.config.storage = config;
        self
    }

    /// Set storage directory
    pub fn storage_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.config.storage.root_dir = dir.into();
        self
    }

    /// Set privacy configuration
    pub fn privacy(mut self, config: PrivacyConfig) -> Self {
        self.config.privacy = config;
        self
    }

    /// Set upload configuration
    pub fn upload(mut self, config: UploadConfig) -> Self {
        self.config.upload = Some(config);
        self
    }

    /// Enable auto-upload
    pub fn auto_upload(mut self, enabled: bool) -> Self {
        self.config.auto_upload = enabled;
        self
    }

    /// Enable auto-start
    pub fn auto_start(mut self, enabled: bool) -> Self {
        self.config.auto_start = enabled;
        self
    }

    /// Set default episode duration
    pub fn default_episode_duration(mut self, seconds: f32) -> Self {
        self.config.default_episode_duration_secs = seconds;
        self
    }

    /// Set trigger cooldown
    pub fn trigger_cooldown(mut self, seconds: f32) -> Self {
        self.config.trigger_cooldown_secs = seconds;
        self
    }

    /// Build the plugin
    pub fn build(self) -> Result<DataCollectionPlugin, PluginError> {
        DataCollectionPlugin::new(self.config)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::super::streams::StreamType;
    use super::*;

    fn create_test_stream() -> StreamConfig {
        StreamConfig::new(
            "test_stream",
            StreamType::JointState {
                joint_names: vec!["joint1".to_string()],
                include_velocities: false,
                include_efforts: false,
            },
            100.0,
        )
    }

    #[test]
    fn test_config_default() {
        let config = DataCollectionConfig::default();
        assert_eq!(config.name, "data-collection");
        assert!(!config.auto_upload);
        assert!(!config.auto_start);
    }

    #[test]
    fn test_config_builder() {
        let config = DataCollectionConfig::new("robot-001")
            .with_name("my-collector")
            .with_auto_upload(true);

        assert_eq!(config.robot_id, "robot-001");
        assert_eq!(config.name, "my-collector");
        assert!(config.auto_upload);
    }

    #[test]
    fn test_config_validation() {
        let config = DataCollectionConfig::default();
        assert!(config.validate().is_err()); // Empty streams

        let config = DataCollectionConfig::new("").with_stream(create_test_stream());
        assert!(config.validate().is_err()); // Empty robot ID

        let config = DataCollectionConfig::new("robot-001").with_stream(create_test_stream());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_plugin_creation() {
        let config = DataCollectionConfig::new("robot-001").with_stream(create_test_stream());

        let plugin = DataCollectionPlugin::new(config);
        assert!(plugin.is_ok());

        let plugin = plugin.unwrap();
        assert_eq!(plugin.state(), PluginState::Created);
        assert!(!plugin.is_running());
    }

    #[test]
    fn test_plugin_lifecycle() {
        let config = DataCollectionConfig::new("robot-001").with_stream(create_test_stream());

        let mut plugin = DataCollectionPlugin::new(config).unwrap();
        assert_eq!(plugin.state(), PluginState::Created);

        // Initialize
        plugin.initialize().unwrap();
        assert_eq!(plugin.state(), PluginState::Initialized);

        // Start
        plugin.start().unwrap();
        assert_eq!(plugin.state(), PluginState::Running);
        assert!(plugin.is_running());

        // Can't start again
        assert!(plugin.start().is_err());

        // Stop
        plugin.stop().unwrap();
        assert_eq!(plugin.state(), PluginState::Stopped);
        assert!(!plugin.is_running());
    }

    #[test]
    fn test_plugin_builder() {
        let plugin = DataCollectionPluginBuilder::new("robot-001")
            .name("test-plugin")
            .stream(create_test_stream())
            .auto_start(false)
            .default_episode_duration(120.0)
            .build();

        assert!(plugin.is_ok());
        let plugin = plugin.unwrap();
        assert_eq!(plugin.config().name, "test-plugin");
        assert_eq!(plugin.config().default_episode_duration_secs, 120.0);
    }

    #[test]
    fn test_push_sample_requires_running() {
        let config = DataCollectionConfig::new("robot-001").with_stream(create_test_stream());

        let mut plugin = DataCollectionPlugin::new(config).unwrap();

        let sample = DataSample::new(
            StreamId::new("test_stream"),
            1000,
            super::super::types::SampleData::Binary(vec![0u8; 100]),
        );

        // Should fail - not running
        assert!(plugin.push_sample(sample.clone()).is_err());

        // Initialize and start
        plugin.initialize().unwrap();
        plugin.start().unwrap();

        // Should succeed now
        assert!(plugin.push_sample(sample).is_ok());
        assert_eq!(plugin.stats().samples_received, 1);
    }

    #[test]
    fn test_manual_episode() {
        let config = DataCollectionConfig::new("robot-001").with_stream(create_test_stream());

        let mut plugin = DataCollectionPlugin::new(config).unwrap();
        plugin.initialize().unwrap();
        plugin.start().unwrap();

        // Start manual episode
        let episode_id = plugin.start_episode_manual("Test episode").unwrap();
        assert!(plugin.is_recording());
        assert_eq!(plugin.current_episode_id(), Some(&episode_id));

        // Can't start another while recording
        assert!(plugin.start_episode_manual("Another").is_err());
    }

    #[test]
    fn test_active_episode_info() {
        let config = DataCollectionConfig::new("robot-001").with_stream(create_test_stream());

        let mut plugin = DataCollectionPlugin::new(config).unwrap();
        plugin.initialize().unwrap();
        plugin.start().unwrap();

        assert!(plugin.active_episode_info().is_none());

        plugin.start_episode_manual("Test").unwrap();

        let info = plugin.active_episode_info();
        assert!(info.is_some());

        let info = info.unwrap();
        assert!(info.elapsed_ns > 0 || info.elapsed_seconds() >= 0.0);
        assert!(info.remaining_seconds() >= 0.0);
    }

    #[test]
    fn test_plugin_state() {
        assert_eq!(PluginState::default(), PluginState::Created);
        assert_ne!(PluginState::Running, PluginState::Stopped);
    }

    #[test]
    fn test_plugin_stats() {
        let stats = PluginStats::default();
        assert_eq!(stats.samples_received, 0);
        assert_eq!(stats.episodes_completed, 0);
    }

    #[test]
    fn test_list_streams() {
        let config = DataCollectionConfig::new("robot-001").with_stream(create_test_stream());

        let plugin = DataCollectionPlugin::new(config).unwrap();
        let streams = plugin.list_streams();
        assert_eq!(streams.len(), 1);
        assert_eq!(streams[0].as_str(), "test_stream");
    }
}
