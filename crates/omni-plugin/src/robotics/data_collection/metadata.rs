//! Episode metadata types
//!
//! Comprehensive metadata for captured episodes, including trigger information,
//! task context, environment details, and quality metrics.

use super::streams::StreamConfig;
use super::types::{EpisodeId, Priority, StreamId, TimestampNs};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Complete episode metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodeMetadata {
    /// Unique episode identifier
    pub episode_id: EpisodeId,
    /// Robot identifier
    pub robot_id: String,
    /// Fleet/organization identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fleet_id: Option<String>,
    /// Episode start time (nanoseconds since epoch)
    pub start_time_ns: TimestampNs,
    /// Episode end time
    pub end_time_ns: TimestampNs,
    /// Duration in seconds
    pub duration_seconds: f64,
    /// What triggered this episode capture
    pub trigger: TriggerInfo,
    /// Task being performed (if known)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<TaskInfo>,
    /// Environment information
    pub environment: EnvironmentInfo,
    /// Operator information (anonymized)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operator: Option<OperatorInfo>,
    /// Data quality metrics
    pub quality: EpisodeQualityMetrics,
    /// Streams included in this episode
    pub streams: Vec<StreamSummary>,
    /// Custom labels/tags for filtering
    #[serde(default)]
    pub labels: HashMap<String, String>,
    /// Privacy processing applied
    pub privacy: PrivacyInfo,
    /// Schema version for forward compatibility
    #[serde(default = "default_schema_version")]
    pub schema_version: String,
}

fn default_schema_version() -> String {
    "1.0".to_string()
}

impl EpisodeMetadata {
    /// Create new episode metadata
    pub fn new(episode_id: EpisodeId, robot_id: impl Into<String>, trigger: TriggerInfo) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        Self {
            episode_id,
            robot_id: robot_id.into(),
            fleet_id: None,
            start_time_ns: now,
            end_time_ns: now,
            duration_seconds: 0.0,
            trigger,
            task: None,
            environment: EnvironmentInfo::default(),
            operator: None,
            quality: EpisodeQualityMetrics::default(),
            streams: Vec::new(),
            labels: HashMap::new(),
            privacy: PrivacyInfo::default(),
            schema_version: default_schema_version(),
        }
    }

    /// Set end time and compute duration
    pub fn set_end_time(&mut self, end_time_ns: TimestampNs) {
        self.end_time_ns = end_time_ns;
        self.duration_seconds = (end_time_ns - self.start_time_ns) as f64 / 1_000_000_000.0;
    }

    /// Add a label
    pub fn add_label(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.labels.insert(key.into(), value.into());
    }

    /// Add a stream summary
    pub fn add_stream(&mut self, stream: StreamSummary) {
        self.streams.push(stream);
    }

    /// Get total data size across all streams
    pub fn total_size_bytes(&self) -> u64 {
        self.streams.iter().map(|s| s.bytes_total).sum()
    }

    /// Get compressed size if available
    pub fn compressed_size_bytes(&self) -> u64 {
        self.streams.iter().map(|s| s.bytes_compressed).sum()
    }

    /// Compute compression ratio
    pub fn compression_ratio(&self) -> f64 {
        let total = self.total_size_bytes();
        let compressed = self.compressed_size_bytes();
        if compressed > 0 {
            total as f64 / compressed as f64
        } else {
            1.0
        }
    }
}

/// Trigger information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerInfo {
    /// Type of trigger
    pub trigger_type: EpisodeTriggerType,
    /// When the trigger fired
    pub trigger_time_ns: TimestampNs,
    /// Confidence score (for ML-based triggers)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,
    /// Pre-trigger buffer duration captured (seconds)
    pub pre_buffer_seconds: f32,
    /// Post-trigger buffer duration captured (seconds)
    pub post_buffer_seconds: f32,
    /// Trigger priority
    #[serde(default)]
    pub priority: Priority,
    /// Additional trigger-specific details
    #[serde(default)]
    pub details: HashMap<String, serde_json::Value>,
}

impl TriggerInfo {
    /// Create a manual trigger
    pub fn manual() -> Self {
        Self {
            trigger_type: EpisodeTriggerType::Manual,
            trigger_time_ns: 0,
            confidence: None,
            pre_buffer_seconds: 30.0,
            post_buffer_seconds: 10.0,
            priority: Priority::Normal,
            details: HashMap::new(),
        }
    }

    /// Create a teleoperation trigger
    pub fn teleoperation() -> Self {
        Self {
            trigger_type: EpisodeTriggerType::Teleoperation,
            trigger_time_ns: 0,
            confidence: Some(1.0),
            pre_buffer_seconds: 5.0,
            post_buffer_seconds: 0.0, // Continue until teleop ends
            priority: Priority::High,
            details: HashMap::new(),
        }
    }

    /// Set trigger timestamp
    pub fn with_timestamp(mut self, timestamp_ns: TimestampNs) -> Self {
        self.trigger_time_ns = timestamp_ns;
        self
    }

    /// Set buffer durations
    pub fn with_buffer(mut self, pre_seconds: f32, post_seconds: f32) -> Self {
        self.pre_buffer_seconds = pre_seconds;
        self.post_buffer_seconds = post_seconds;
        self
    }
}

/// Episode trigger types (stored in episode metadata)
///
/// This enum represents the type of trigger that initiated an episode capture.
/// It differs from triggers::TriggerType which is used for the trigger system itself.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EpisodeTriggerType {
    /// Manual trigger via API or button
    Manual,
    /// Teleoperation session
    Teleoperation,
    /// Autonomous task execution
    AutonomousTask,
    /// Failure/error detected
    FailureDetected {
        /// Failure code
        #[serde(skip_serializing_if = "Option::is_none")]
        code: Option<String>,
        /// Failure message
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
    /// Novel/out-of-distribution situation
    NovelSituation {
        /// Novelty score
        score: f32,
    },
    /// Periodic/scheduled capture
    Periodic {
        /// Interval in seconds
        interval_seconds: u32,
    },
    /// Anomaly score threshold exceeded
    AnomalyScore {
        /// Anomaly score
        score: f32,
        /// Threshold that was exceeded
        threshold: f32,
    },
    /// External trigger (from another system)
    External {
        /// Source system
        source: String,
    },
    /// Human intervention occurred
    Intervention,
    /// Safety event (e-stop, collision, etc.)
    SafetyEvent {
        /// Event type
        event_type: String,
    },
}

/// Task information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskInfo {
    /// Task identifier
    pub task_id: String,
    /// Task type/category
    pub task_type: String,
    /// Human-readable task name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_name: Option<String>,
    /// Whether task completed successfully
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success: Option<bool>,
    /// Task-specific metrics
    #[serde(default)]
    pub metrics: HashMap<String, f64>,
    /// Task parameters
    #[serde(default)]
    pub parameters: HashMap<String, serde_json::Value>,
}

impl TaskInfo {
    /// Create new task info
    pub fn new(task_id: impl Into<String>, task_type: impl Into<String>) -> Self {
        Self {
            task_id: task_id.into(),
            task_type: task_type.into(),
            task_name: None,
            success: None,
            metrics: HashMap::new(),
            parameters: HashMap::new(),
        }
    }

    /// Set success status
    pub fn with_success(mut self, success: bool) -> Self {
        self.success = Some(success);
        self
    }

    /// Add a metric
    pub fn with_metric(mut self, name: impl Into<String>, value: f64) -> Self {
        self.metrics.insert(name.into(), value);
        self
    }
}

/// Environment information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentInfo {
    /// Site/location identifier
    pub site_id: String,
    /// Zone within site
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zone: Option<String>,
    /// Lighting conditions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lighting: Option<LightingCondition>,
    /// Floor/ground type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub floor_type: Option<String>,
    /// Whether obstacles are present
    #[serde(default)]
    pub obstacles_present: bool,
    /// Whether humans are present in the scene
    #[serde(default)]
    pub humans_present: bool,
    /// Weather conditions (for outdoor)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weather: Option<String>,
    /// Temperature in Celsius
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature_c: Option<f32>,
    /// Additional environment properties
    #[serde(default)]
    pub properties: HashMap<String, serde_json::Value>,
}

impl Default for EnvironmentInfo {
    fn default() -> Self {
        Self {
            site_id: "unknown".to_string(),
            zone: None,
            lighting: None,
            floor_type: None,
            obstacles_present: false,
            humans_present: false,
            weather: None,
            temperature_c: None,
            properties: HashMap::new(),
        }
    }
}

/// Lighting conditions
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LightingCondition {
    /// Natural daylight
    Daylight,
    /// Artificial indoor lighting
    Artificial,
    /// Mixed natural and artificial
    Mixed,
    /// Low light conditions
    Low,
    /// Dark/night conditions
    Dark,
    /// Variable/changing lighting
    Variable,
}

/// Operator information (anonymized)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorInfo {
    /// Anonymized operator identifier (hash)
    pub operator_hash: String,
    /// Experience level
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experience_level: Option<ExperienceLevel>,
    /// Input device used
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_device: Option<String>,
    /// Session duration before this episode (for fatigue tracking)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_duration_minutes: Option<u32>,
}

impl OperatorInfo {
    /// Create from operator ID (will be hashed)
    pub fn from_id(operator_id: &str) -> Self {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(operator_id.as_bytes());
        let hash = hex::encode(hasher.finalize());

        Self {
            operator_hash: hash[..16].to_string(), // First 16 chars
            experience_level: None,
            input_device: None,
            session_duration_minutes: None,
        }
    }
}

/// Experience level
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExperienceLevel {
    /// New operator (< 10 hours)
    Novice,
    /// Some experience (10-100 hours)
    Intermediate,
    /// Experienced operator (> 100 hours)
    Expert,
}

/// Episode quality metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodeQualityMetrics {
    /// Overall quality score (0.0 - 1.0)
    pub overall_score: f32,
    /// Percentage of frames with valid camera poses
    pub pose_coverage: f32,
    /// Percentage of frames passing sharpness threshold
    pub sharpness_pass_rate: f32,
    /// Data synchronization quality (0.0 - 1.0)
    pub sync_quality: f32,
    /// Any data gaps detected
    #[serde(default)]
    pub data_gaps: Vec<DataGap>,
    /// Streams with quality issues
    #[serde(default)]
    pub stream_issues: Vec<StreamIssue>,
    /// Whether episode is usable for training
    #[serde(default = "default_true")]
    pub usable_for_training: bool,
    /// Reason if not usable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejection_reason: Option<String>,
}

fn default_true() -> bool {
    true
}

impl Default for EpisodeQualityMetrics {
    fn default() -> Self {
        Self {
            overall_score: 1.0,
            pose_coverage: 1.0,
            sharpness_pass_rate: 1.0,
            sync_quality: 1.0,
            data_gaps: Vec::new(),
            stream_issues: Vec::new(),
            usable_for_training: true,
            rejection_reason: None,
        }
    }
}

impl EpisodeQualityMetrics {
    /// Mark as not usable with reason
    pub fn reject(&mut self, reason: impl Into<String>) {
        self.usable_for_training = false;
        self.rejection_reason = Some(reason.into());
    }

    /// Add a data gap
    pub fn add_gap(&mut self, gap: DataGap) {
        self.data_gaps.push(gap);
        // Reduce quality score based on gap
        self.overall_score *= 0.95;
    }
}

/// Data gap in a stream
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataGap {
    /// Stream that has the gap
    pub stream_id: StreamId,
    /// Gap start time
    pub start_ns: TimestampNs,
    /// Gap end time
    pub end_ns: TimestampNs,
    /// Expected number of samples missing
    pub expected_samples: u32,
    /// Reason for gap (if known)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl DataGap {
    /// Get gap duration in milliseconds
    pub fn duration_ms(&self) -> f64 {
        (self.end_ns - self.start_ns) as f64 / 1_000_000.0
    }
}

/// Quality issue with a stream
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamIssue {
    /// Affected stream
    pub stream_id: StreamId,
    /// Issue type
    pub issue_type: StreamIssueType,
    /// Severity
    pub severity: IssueSeverity,
    /// Description
    pub description: String,
}

/// Types of stream issues
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamIssueType {
    /// Data rate lower than expected
    LowDataRate,
    /// Data rate higher than expected
    HighDataRate,
    /// High latency/jitter
    HighLatency,
    /// Compression artifacts
    CompressionArtifacts,
    /// Sensor malfunction
    SensorMalfunction,
    /// Synchronization drift
    SyncDrift,
    /// Other issue
    Other,
}

/// Issue severity
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueSeverity {
    /// Informational, no impact on quality
    Info,
    /// Minor issue, slightly reduced quality
    Warning,
    /// Significant issue, quality impacted
    Error,
    /// Critical issue, data may be unusable
    Critical,
}

/// Stream summary in episode metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamSummary {
    /// Stream identifier
    pub stream_id: StreamId,
    /// Stream type name
    pub stream_type: String,
    /// Number of messages/samples
    pub message_count: u64,
    /// Total uncompressed bytes
    pub bytes_total: u64,
    /// Compressed bytes (if applicable)
    pub bytes_compressed: u64,
    /// First sample timestamp
    pub first_timestamp_ns: TimestampNs,
    /// Last sample timestamp
    pub last_timestamp_ns: TimestampNs,
    /// Actual data rate achieved
    pub actual_hz: f32,
    /// Compression ratio achieved
    pub compression_ratio: f32,
}

impl StreamSummary {
    /// Create from stream config with initial values
    pub fn from_config(config: &StreamConfig) -> Self {
        Self {
            stream_id: config.stream_id.clone(),
            stream_type: config.stream_type.type_name().to_string(),
            message_count: 0,
            bytes_total: 0,
            bytes_compressed: 0,
            first_timestamp_ns: 0,
            last_timestamp_ns: 0,
            actual_hz: 0.0,
            compression_ratio: 1.0,
        }
    }

    /// Compute actual data rate
    pub fn compute_rate(&mut self) {
        if self.first_timestamp_ns > 0 && self.last_timestamp_ns > self.first_timestamp_ns {
            let duration_secs =
                (self.last_timestamp_ns - self.first_timestamp_ns) as f64 / 1_000_000_000.0;
            if duration_secs > 0.0 {
                self.actual_hz = self.message_count as f32 / duration_secs as f32;
            }
        }
        if self.bytes_compressed > 0 {
            self.compression_ratio = self.bytes_total as f32 / self.bytes_compressed as f32;
        }
    }
}

/// Privacy processing information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyInfo {
    /// Faces were blurred
    #[serde(default)]
    pub faces_blurred: bool,
    /// License plates were blurred
    #[serde(default)]
    pub license_plates_blurred: bool,
    /// PII was removed from text/audio
    #[serde(default)]
    pub pii_removed: bool,
    /// Audio was processed for privacy
    #[serde(default)]
    pub audio_processed: bool,
    /// Number of faces detected and blurred
    #[serde(default)]
    pub faces_detected: u32,
    /// Privacy filter version used
    #[serde(default = "default_privacy_version")]
    pub processing_version: String,
    /// Additional privacy processing applied
    #[serde(default)]
    pub additional_filters: Vec<String>,
}

fn default_privacy_version() -> String {
    "1.0".to_string()
}

impl Default for PrivacyInfo {
    fn default() -> Self {
        Self {
            faces_blurred: false,
            license_plates_blurred: false,
            pii_removed: false,
            audio_processed: false,
            faces_detected: 0,
            processing_version: default_privacy_version(),
            additional_filters: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_episode_metadata_creation() {
        let trigger = TriggerInfo::manual().with_timestamp(1_000_000_000);
        let metadata = EpisodeMetadata::new(EpisodeId::new(), "robot-001", trigger);

        assert_eq!(metadata.robot_id, "robot-001");
        assert!(metadata.duration_seconds >= 0.0);
    }

    #[test]
    fn test_set_end_time() {
        let trigger = TriggerInfo::manual();
        let mut metadata = EpisodeMetadata::new(EpisodeId::new(), "robot-001", trigger);
        metadata.start_time_ns = 1_000_000_000;
        metadata.set_end_time(2_000_000_000);

        assert_eq!(metadata.duration_seconds, 1.0);
    }

    #[test]
    fn test_task_info() {
        let task = TaskInfo::new("task-123", "pick_and_place")
            .with_success(true)
            .with_metric("duration_seconds", 5.5);

        assert_eq!(task.task_id, "task-123");
        assert_eq!(task.success, Some(true));
        assert_eq!(task.metrics.get("duration_seconds"), Some(&5.5));
    }

    #[test]
    fn test_operator_anonymization() {
        let op1 = OperatorInfo::from_id("user@example.com");
        let op2 = OperatorInfo::from_id("user@example.com");
        let op3 = OperatorInfo::from_id("other@example.com");

        // Same user should produce same hash
        assert_eq!(op1.operator_hash, op2.operator_hash);
        // Different users should produce different hashes
        assert_ne!(op1.operator_hash, op3.operator_hash);
    }

    #[test]
    fn test_data_gap() {
        let gap = DataGap {
            stream_id: StreamId::new("camera"),
            start_ns: 1_000_000_000,
            end_ns: 1_100_000_000,
            expected_samples: 3,
            reason: Some("sensor dropout".to_string()),
        };

        assert_eq!(gap.duration_ms(), 100.0);
    }

    #[test]
    fn test_quality_rejection() {
        let mut quality = EpisodeQualityMetrics::default();
        assert!(quality.usable_for_training);

        quality.reject("Too much motion blur");
        assert!(!quality.usable_for_training);
        assert_eq!(
            quality.rejection_reason,
            Some("Too much motion blur".to_string())
        );
    }
}
