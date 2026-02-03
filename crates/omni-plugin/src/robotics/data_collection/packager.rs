//! Episode packager for robot data collection
//!
//! Combines data from multiple streams into a complete episode package,
//! applying privacy filters and writing to MCAP format.

use super::buffer::BufferManager;
use super::mcap_writer::{McapWriter, McapWriterConfig, Schema};
use super::metadata::EpisodeMetadata;
use super::privacy::{PrivacyConfig, PrivacyManager};
use super::streams::StreamConfig;
use super::types::{DataSample, EpisodeId, StreamId, TimestampNs};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use thiserror::Error;

/// Packager-related errors
#[derive(Debug, Error)]
pub enum PackagerError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("MCAP error: {0}")]
    Mcap(#[from] super::mcap_writer::McapError),

    #[error("Privacy error: {0}")]
    Privacy(#[from] super::privacy::PrivacyError),

    #[error("No data available for time range")]
    NoDataAvailable,

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Episode already exists: {0}")]
    EpisodeExists(String),

    #[error("Packaging in progress")]
    PackagingInProgress,

    #[error("Serialization error: {0}")]
    SerializationError(String),
}

/// Episode package configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackagerConfig {
    /// Output directory for episodes
    pub output_dir: PathBuf,
    /// MCAP writer configuration
    #[serde(skip)]
    pub mcap_config: Option<McapWriterConfig>,
    /// Privacy configuration
    pub privacy_config: PrivacyConfig,
    /// Streams to include in episodes
    pub streams: Vec<StreamConfig>,
    /// Whether to include metadata file
    #[serde(default = "default_true")]
    pub include_metadata: bool,
    /// Whether to include URDF as attachment
    #[serde(default = "default_true")]
    pub include_urdf: bool,
    /// Whether to include calibration data
    #[serde(default = "default_true")]
    pub include_calibration: bool,
    /// Maximum episode duration in seconds
    #[serde(default = "default_max_duration")]
    pub max_episode_duration_secs: f32,
    /// Minimum samples required per stream
    #[serde(default = "default_min_samples")]
    pub min_samples_per_stream: u32,
}

fn default_true() -> bool {
    true
}

fn default_max_duration() -> f32 {
    300.0 // 5 minutes
}

fn default_min_samples() -> u32 {
    10
}

impl Default for PackagerConfig {
    fn default() -> Self {
        Self {
            output_dir: PathBuf::from("episodes"),
            mcap_config: None,
            privacy_config: PrivacyConfig::default(),
            streams: Vec::new(),
            include_metadata: true,
            include_urdf: true,
            include_calibration: true,
            max_episode_duration_secs: 300.0,
            min_samples_per_stream: 10,
        }
    }
}

impl PackagerConfig {
    /// Create a new packager config
    pub fn new(output_dir: impl Into<PathBuf>) -> Self {
        Self {
            output_dir: output_dir.into(),
            ..Default::default()
        }
    }

    /// Add a stream configuration
    pub fn with_stream(mut self, stream: StreamConfig) -> Self {
        self.streams.push(stream);
        self
    }

    /// Set privacy configuration
    pub fn with_privacy(mut self, config: PrivacyConfig) -> Self {
        self.privacy_config = config;
        self
    }

    /// Set MCAP configuration
    pub fn with_mcap_config(mut self, config: McapWriterConfig) -> Self {
        self.mcap_config = Some(config);
        self
    }
}

/// Result of episode packaging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageResult {
    /// Episode ID
    pub episode_id: EpisodeId,
    /// Path to the MCAP file
    pub mcap_path: PathBuf,
    /// Path to metadata JSON file (if created)
    pub metadata_path: Option<PathBuf>,
    /// Total samples written
    pub total_samples: u64,
    /// Samples per stream
    pub samples_per_stream: HashMap<String, u64>,
    /// Episode duration in nanoseconds
    pub duration_ns: u64,
    /// Total file size in bytes
    pub file_size_bytes: u64,
    /// Privacy detections count
    pub privacy_detections: u64,
    /// Packaging time in milliseconds
    pub packaging_time_ms: u64,
}

/// Packaging progress callback
pub type ProgressCallback = Box<dyn Fn(PackagingProgress) + Send + Sync>;

/// Packaging progress information
#[derive(Debug, Clone)]
pub struct PackagingProgress {
    /// Current phase
    pub phase: PackagingPhase,
    /// Progress percentage (0-100)
    pub progress_percent: f32,
    /// Samples processed
    pub samples_processed: u64,
    /// Total samples
    pub total_samples: u64,
    /// Current stream being processed
    pub current_stream: Option<StreamId>,
}

/// Packaging phases
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackagingPhase {
    /// Initializing
    Initializing,
    /// Collecting samples from buffers
    CollectingSamples,
    /// Applying privacy filters
    ApplyingPrivacy,
    /// Writing MCAP file
    WritingMcap,
    /// Writing metadata
    WritingMetadata,
    /// Finalizing
    Finalizing,
    /// Complete
    Complete,
}

/// Episode packager
///
/// Collects data from ring buffers, applies privacy filters,
/// and writes to MCAP format.
pub struct EpisodePackager {
    /// Configuration
    config: PackagerConfig,
    /// Privacy manager
    privacy_manager: PrivacyManager,
    /// URDF data (optional attachment)
    urdf_data: Option<Vec<u8>>,
    /// Calibration data (optional attachment)
    calibration_data: Option<HashMap<String, Vec<u8>>>,
    /// Stream schemas (topic -> schema)
    stream_schemas: HashMap<StreamId, Schema>,
    /// Is currently packaging
    is_packaging: bool,
}

impl EpisodePackager {
    /// Create a new episode packager
    pub fn new(config: PackagerConfig) -> Self {
        let privacy_manager = PrivacyManager::with_default_filters(config.privacy_config.clone());

        Self {
            config,
            privacy_manager,
            urdf_data: None,
            calibration_data: None,
            stream_schemas: HashMap::new(),
            is_packaging: false,
        }
    }

    /// Set URDF data for attachment
    pub fn set_urdf(&mut self, urdf: Vec<u8>) {
        self.urdf_data = Some(urdf);
    }

    /// Set calibration data for a camera
    pub fn set_calibration(&mut self, camera_id: impl Into<String>, data: Vec<u8>) {
        if self.calibration_data.is_none() {
            self.calibration_data = Some(HashMap::new());
        }
        self.calibration_data
            .as_mut()
            .unwrap()
            .insert(camera_id.into(), data);
    }

    /// Register a schema for a stream
    pub fn register_schema(&mut self, stream_id: StreamId, schema: Schema) {
        self.stream_schemas.insert(stream_id, schema);
    }

    /// Get configuration
    pub fn config(&self) -> &PackagerConfig {
        &self.config
    }

    /// Package an episode from buffer data
    ///
    /// # Arguments
    /// * `episode_id` - Unique episode identifier
    /// * `buffer_manager` - Buffer manager containing stream data
    /// * `start_time` - Start timestamp for the episode
    /// * `end_time` - End timestamp for the episode
    /// * `metadata` - Episode metadata
    pub fn package_episode(
        &mut self,
        episode_id: EpisodeId,
        buffer_manager: &BufferManager,
        start_time: TimestampNs,
        end_time: TimestampNs,
        metadata: EpisodeMetadata,
    ) -> Result<PackageResult, PackagerError> {
        self.package_episode_with_progress(
            episode_id,
            buffer_manager,
            start_time,
            end_time,
            metadata,
            None,
        )
    }

    /// Package an episode with progress callback
    pub fn package_episode_with_progress(
        &mut self,
        episode_id: EpisodeId,
        buffer_manager: &BufferManager,
        start_time: TimestampNs,
        end_time: TimestampNs,
        metadata: EpisodeMetadata,
        progress_callback: Option<ProgressCallback>,
    ) -> Result<PackageResult, PackagerError> {
        if self.is_packaging {
            return Err(PackagerError::PackagingInProgress);
        }

        self.is_packaging = true;
        let start_instant = std::time::Instant::now();

        let result = self.do_package(
            episode_id,
            buffer_manager,
            start_time,
            end_time,
            metadata,
            progress_callback,
        );

        self.is_packaging = false;
        result.map(|mut r| {
            r.packaging_time_ms = start_instant.elapsed().as_millis() as u64;
            r
        })
    }

    fn do_package(
        &mut self,
        episode_id: EpisodeId,
        buffer_manager: &BufferManager,
        start_time: TimestampNs,
        end_time: TimestampNs,
        metadata: EpisodeMetadata,
        progress_callback: Option<ProgressCallback>,
    ) -> Result<PackageResult, PackagerError> {
        // Report progress
        let report_progress = |phase: PackagingPhase,
                               percent: f32,
                               samples: u64,
                               total: u64,
                               stream: Option<StreamId>| {
            if let Some(ref cb) = progress_callback {
                cb(PackagingProgress {
                    phase,
                    progress_percent: percent,
                    samples_processed: samples,
                    total_samples: total,
                    current_stream: stream,
                });
            }
        };

        report_progress(PackagingPhase::Initializing, 0.0, 0, 0, None);

        // Create output directory
        std::fs::create_dir_all(&self.config.output_dir)?;

        // Create episode directory
        let episode_dir = self.config.output_dir.join(episode_id.as_str());
        std::fs::create_dir_all(&episode_dir)?;

        // Collect samples from all streams
        report_progress(PackagingPhase::CollectingSamples, 5.0, 0, 0, None);

        let mut all_samples: Vec<DataSample> = Vec::new();
        let mut samples_per_stream: HashMap<String, u64> = HashMap::new();

        for stream_id in buffer_manager.stream_ids() {
            if let Some(buffer) = buffer_manager.get_buffer(&stream_id) {
                let samples = buffer.get_range(start_time, end_time);
                let count = samples.len() as u64;
                samples_per_stream.insert(stream_id.as_str().to_string(), count);
                all_samples.extend(samples);
            }
        }

        if all_samples.is_empty() {
            return Err(PackagerError::NoDataAvailable);
        }

        // Sort samples by timestamp
        all_samples.sort_by_key(|s| s.timestamp_ns);

        let total_samples = all_samples.len() as u64;

        // Apply privacy filters
        report_progress(
            PackagingPhase::ApplyingPrivacy,
            20.0,
            0,
            total_samples,
            None,
        );

        let mut privacy_detections: u64 = 0;
        let mut processed_samples = Vec::with_capacity(all_samples.len());

        for (i, mut sample) in all_samples.into_iter().enumerate() {
            // Check if this is an image stream that needs privacy processing
            if self.is_image_stream(&sample.stream_id) {
                // Get image dimensions from metadata or use defaults
                let (width, height) = self.get_image_dimensions(&sample);

                // Get binary data for privacy processing
                let image_data = sample.data.to_bytes();
                let result = self.privacy_manager.process_image(
                    &sample.stream_id,
                    &image_data,
                    width,
                    height,
                )?;

                if result.modified {
                    sample.data = super::types::SampleData::Binary(result.data);
                    privacy_detections += result.detections.len() as u64;
                }
            }

            processed_samples.push(sample);

            if i % 100 == 0 {
                let percent = 20.0 + (i as f32 / total_samples as f32) * 30.0;
                report_progress(
                    PackagingPhase::ApplyingPrivacy,
                    percent,
                    i as u64,
                    total_samples,
                    None,
                );
            }
        }

        // Write MCAP file
        report_progress(PackagingPhase::WritingMcap, 50.0, 0, total_samples, None);

        let mcap_path = episode_dir.join(format!("{}.mcap", episode_id.as_str()));
        let mcap_file = File::create(&mcap_path)?;
        let mcap_writer = BufWriter::new(mcap_file);

        let mcap_config = self.config.mcap_config.clone().unwrap_or_default();

        let mut writer = McapWriter::new(mcap_writer, mcap_config);
        writer.start()?;

        // Register schemas and channels
        let mut channel_map: HashMap<StreamId, u16> = HashMap::new();

        for stream_id in samples_per_stream.keys() {
            let stream_id_obj = StreamId::new(stream_id);

            // Create or use existing schema
            let schema_id = if let Some(schema) = self.stream_schemas.get(&stream_id_obj) {
                writer.register_schema(schema.clone())?
            } else {
                // Create a default JSON schema
                writer.add_schema(
                    format!("omniedge/{}", stream_id),
                    "jsonschema",
                    b"{}".to_vec(),
                )?
            };

            // Create channel
            let channel_id = writer.add_channel(schema_id, format!("/{}", stream_id), "json")?;
            channel_map.insert(stream_id_obj.clone(), channel_id);
            writer.register_stream(stream_id_obj, channel_id)?;
        }

        // Write samples
        for (i, sample) in processed_samples.iter().enumerate() {
            writer.write_sample(sample)?;

            if i % 100 == 0 {
                let percent = 50.0 + (i as f32 / total_samples as f32) * 30.0;
                report_progress(
                    PackagingPhase::WritingMcap,
                    percent,
                    i as u64,
                    total_samples,
                    Some(sample.stream_id.clone()),
                );
            }
        }

        // Add attachments
        if self.config.include_urdf {
            if let Some(ref urdf) = self.urdf_data {
                writer.add_attachment("robot.urdf", "application/xml", urdf)?;
            }
        }

        if self.config.include_calibration {
            if let Some(ref cal_data) = self.calibration_data {
                for (camera_id, data) in cal_data {
                    writer.add_attachment(
                        format!("calibration_{}.yaml", camera_id),
                        "application/x-yaml",
                        data,
                    )?;
                }
            }
        }

        // Add episode metadata as MCAP metadata
        let mut meta_map = HashMap::new();
        meta_map.insert("episode_id".into(), episode_id.as_str().to_string());
        meta_map.insert("start_time_ns".into(), start_time.to_string());
        meta_map.insert("end_time_ns".into(), end_time.to_string());
        meta_map.insert("sample_count".into(), total_samples.to_string());
        writer.add_metadata("episode_info", &meta_map)?;

        let _mcap_stats = writer.finish()?;

        // Write metadata JSON file
        report_progress(
            PackagingPhase::WritingMetadata,
            85.0,
            total_samples,
            total_samples,
            None,
        );

        let metadata_path = if self.config.include_metadata {
            let path = episode_dir.join(format!("{}_metadata.json", episode_id.as_str()));
            let metadata_json = serde_json::to_string_pretty(&metadata)
                .map_err(|e| PackagerError::SerializationError(e.to_string()))?;
            std::fs::write(&path, metadata_json)?;
            Some(path)
        } else {
            None
        };

        // Get file size
        let file_size = std::fs::metadata(&mcap_path)?.len();

        report_progress(
            PackagingPhase::Finalizing,
            95.0,
            total_samples,
            total_samples,
            None,
        );

        let duration_ns = end_time.saturating_sub(start_time);

        report_progress(
            PackagingPhase::Complete,
            100.0,
            total_samples,
            total_samples,
            None,
        );

        Ok(PackageResult {
            episode_id,
            mcap_path,
            metadata_path,
            total_samples,
            samples_per_stream,
            duration_ns,
            file_size_bytes: file_size,
            privacy_detections,
            packaging_time_ms: 0, // Set by caller
        })
    }

    /// Check if a stream contains image data
    fn is_image_stream(&self, stream_id: &StreamId) -> bool {
        // Check stream configuration
        for config in &self.config.streams {
            if config.stream_id == *stream_id {
                return matches!(
                    config.stream_type,
                    super::streams::StreamType::RgbCamera { .. }
                        | super::streams::StreamType::DepthCamera { .. }
                );
            }
        }

        // Heuristic: check stream name
        let name = stream_id.as_str().to_lowercase();
        name.contains("camera")
            || name.contains("image")
            || name.contains("rgb")
            || name.contains("depth")
    }

    /// Get image dimensions for a sample
    fn get_image_dimensions(&self, sample: &DataSample) -> (u32, u32) {
        // Check stream configuration
        for config in &self.config.streams {
            if config.stream_id == sample.stream_id {
                if let super::streams::StreamType::RgbCamera { width, height, .. } =
                    &config.stream_type
                {
                    return (*width, *height);
                }
                if let super::streams::StreamType::DepthCamera { width, height, .. } =
                    &config.stream_type
                {
                    return (*width, *height);
                }
            }
        }

        // Default dimensions
        (640, 480)
    }
}

/// Package episodes in batch
pub struct BatchPackager {
    /// Packager instance
    packager: EpisodePackager,
    /// Queue of episodes to package
    queue: Vec<PendingEpisode>,
}

/// Pending episode in queue
#[derive(Debug, Clone)]
pub struct PendingEpisode {
    /// Episode ID
    pub episode_id: EpisodeId,
    /// Start timestamp
    pub start_time: TimestampNs,
    /// End timestamp
    pub end_time: TimestampNs,
    /// Metadata
    pub metadata: EpisodeMetadata,
}

impl BatchPackager {
    /// Create a new batch packager
    pub fn new(config: PackagerConfig) -> Self {
        Self {
            packager: EpisodePackager::new(config),
            queue: Vec::new(),
        }
    }

    /// Add an episode to the queue
    pub fn enqueue(&mut self, episode: PendingEpisode) {
        self.queue.push(episode);
    }

    /// Get queue length
    pub fn queue_len(&self) -> usize {
        self.queue.len()
    }

    /// Process all queued episodes
    pub fn process_all(
        &mut self,
        buffer_manager: &BufferManager,
    ) -> Vec<Result<PackageResult, PackagerError>> {
        let episodes: Vec<_> = self.queue.drain(..).collect();
        let mut results = Vec::with_capacity(episodes.len());

        for episode in episodes {
            let result = self.packager.package_episode(
                episode.episode_id,
                buffer_manager,
                episode.start_time,
                episode.end_time,
                episode.metadata,
            );
            results.push(result);
        }

        results
    }

    /// Get inner packager
    pub fn packager(&self) -> &EpisodePackager {
        &self.packager
    }

    /// Get inner packager mutably
    pub fn packager_mut(&mut self) -> &mut EpisodePackager {
        &mut self.packager
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packager_config_default() {
        let config = PackagerConfig::default();
        assert!(config.include_metadata);
        assert!(config.include_urdf);
        assert_eq!(config.max_episode_duration_secs, 300.0);
    }

    #[test]
    fn test_packager_config_builder() {
        let config = PackagerConfig::new("/tmp/episodes").with_privacy(PrivacyConfig::strict());

        assert_eq!(config.output_dir, PathBuf::from("/tmp/episodes"));
        assert!(config.privacy_config.blur_faces);
        assert!(config.privacy_config.blur_license_plates);
    }

    #[test]
    fn test_packager_creation() {
        let config = PackagerConfig::default();
        let packager = EpisodePackager::new(config);

        assert!(!packager.is_packaging);
    }

    #[test]
    fn test_packager_set_urdf() {
        let config = PackagerConfig::default();
        let mut packager = EpisodePackager::new(config);

        packager.set_urdf(b"<robot></robot>".to_vec());
        assert!(packager.urdf_data.is_some());
    }

    #[test]
    fn test_packager_set_calibration() {
        let config = PackagerConfig::default();
        let mut packager = EpisodePackager::new(config);

        packager.set_calibration("cam0", b"calibration_data".to_vec());
        assert!(packager.calibration_data.is_some());
        assert!(packager
            .calibration_data
            .as_ref()
            .unwrap()
            .contains_key("cam0"));
    }

    #[test]
    fn test_is_image_stream() {
        let config = PackagerConfig::default();
        let packager = EpisodePackager::new(config);

        assert!(packager.is_image_stream(&StreamId::new("rgb_camera_0")));
        assert!(packager.is_image_stream(&StreamId::new("depth_image")));
        assert!(!packager.is_image_stream(&StreamId::new("joint_states")));
    }

    #[test]
    fn test_batch_packager() {
        let config = PackagerConfig::default();
        let mut batch = BatchPackager::new(config);

        assert_eq!(batch.queue_len(), 0);

        batch.enqueue(PendingEpisode {
            episode_id: EpisodeId::new(),
            start_time: 0,
            end_time: 1000,
            metadata: EpisodeMetadata::new(
                EpisodeId::new(),
                "test_robot",
                crate::robotics::data_collection::metadata::TriggerInfo::manual(),
            ),
        });

        assert_eq!(batch.queue_len(), 1);
    }

    #[test]
    fn test_packaging_phase() {
        assert_ne!(PackagingPhase::Initializing, PackagingPhase::Complete);
        assert_eq!(PackagingPhase::Complete, PackagingPhase::Complete);
    }

    #[test]
    fn test_package_result() {
        let result = PackageResult {
            episode_id: EpisodeId::from_string("test-episode"),
            mcap_path: PathBuf::from("/tmp/test.mcap"),
            metadata_path: Some(PathBuf::from("/tmp/test_metadata.json")),
            total_samples: 1000,
            samples_per_stream: HashMap::new(),
            duration_ns: 5_000_000_000,
            file_size_bytes: 1024 * 1024,
            privacy_detections: 5,
            packaging_time_ms: 100,
        };

        assert_eq!(result.episode_id.as_str(), "test-episode");
        assert_eq!(result.total_samples, 1000);
    }
}
