//! Core types for the robot data collection plugin
//!
//! Defines the fundamental data structures for episode capture, sensor streams,
//! and metadata.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Timestamp in nanoseconds since Unix epoch
pub type TimestampNs = u64;

/// Episode identifier (UUID-based)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EpisodeId(pub String);

impl EpisodeId {
    /// Create a new random episode ID
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    /// Create from existing string
    pub fn from_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Get the string representation
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for EpisodeId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for EpisodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Stream identifier for sensor data channels
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StreamId(pub String);

impl StreamId {
    /// Create a new stream ID
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Get the string representation
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for StreamId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Data sample with metadata
#[derive(Debug, Clone)]
pub struct DataSample {
    /// Stream this sample belongs to
    pub stream_id: StreamId,
    /// Timestamp in nanoseconds
    pub timestamp_ns: TimestampNs,
    /// Sequence number within stream
    pub sequence: u64,
    /// Sample data
    pub data: SampleData,
    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl DataSample {
    /// Create a new data sample
    pub fn new(stream_id: StreamId, timestamp_ns: TimestampNs, data: SampleData) -> Self {
        Self {
            stream_id,
            timestamp_ns,
            sequence: 0,
            data,
            metadata: HashMap::new(),
        }
    }

    /// Set sequence number
    pub fn with_sequence(mut self, seq: u64) -> Self {
        self.sequence = seq;
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Get approximate size in bytes
    pub fn size_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + self.data.size_bytes()
    }
}

/// Sample data variants
#[derive(Debug, Clone)]
pub enum SampleData {
    /// Raw binary data (images, point clouds, encoded data)
    Binary(Vec<u8>),
    /// Joint state observation
    JointState(JointState),
    /// Robot command
    Command(RobotCommand),
    /// Sensor reading
    Sensor(SensorReading),
    /// Event marker
    Event(EventMarker),
    /// Teleoperation input
    TeleopInput(TeleopInput),
}

impl SampleData {
    /// Get approximate size in bytes
    pub fn size_bytes(&self) -> usize {
        match self {
            SampleData::Binary(data) => data.len(),
            SampleData::JointState(js) => {
                js.names.len() * 32 + js.positions.len() * 8 * 3 // rough estimate
            }
            SampleData::Command(cmd) => std::mem::size_of_val(cmd) + 256, // estimate
            SampleData::Sensor(sr) => sr.values.len() * 8 + 64,
            SampleData::Event(_) => 256,
            SampleData::TeleopInput(_) => 256,
        }
    }

    /// Convert to bytes for serialization
    ///
    /// For Binary data, returns the raw bytes directly.
    /// For other types, serializes to JSON.
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            SampleData::Binary(data) => data.clone(),
            SampleData::JointState(js) => serde_json::to_vec(js).unwrap_or_default(),
            SampleData::Command(cmd) => serde_json::to_vec(cmd).unwrap_or_default(),
            SampleData::Sensor(sr) => serde_json::to_vec(sr).unwrap_or_default(),
            SampleData::Event(ev) => serde_json::to_vec(ev).unwrap_or_default(),
            SampleData::TeleopInput(ti) => serde_json::to_vec(ti).unwrap_or_default(),
        }
    }

    /// Get as binary slice if this is Binary data
    pub fn as_binary(&self) -> Option<&[u8]> {
        match self {
            SampleData::Binary(data) => Some(data),
            _ => None,
        }
    }
}

/// Joint state observation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JointState {
    /// Joint names
    pub names: Vec<String>,
    /// Joint positions (radians for revolute, meters for prismatic)
    pub positions: Vec<f64>,
    /// Joint velocities (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub velocities: Option<Vec<f64>>,
    /// Joint efforts/torques (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub efforts: Option<Vec<f64>>,
}

impl JointState {
    /// Create a new joint state
    pub fn new(names: Vec<String>, positions: Vec<f64>) -> Self {
        Self {
            names,
            positions,
            velocities: None,
            efforts: None,
        }
    }

    /// Add velocities
    pub fn with_velocities(mut self, velocities: Vec<f64>) -> Self {
        self.velocities = Some(velocities);
        self
    }

    /// Add efforts
    pub fn with_efforts(mut self, efforts: Vec<f64>) -> Self {
        self.efforts = Some(efforts);
        self
    }

    /// Get position for a joint by name
    pub fn get_position(&self, name: &str) -> Option<f64> {
        self.names
            .iter()
            .position(|n| n == name)
            .map(|i| self.positions[i])
    }

    /// Convert to HashMap for easy lookup
    pub fn as_map(&self) -> HashMap<String, f64> {
        self.names
            .iter()
            .cloned()
            .zip(self.positions.iter().copied())
            .collect()
    }
}

/// Robot command (sent to actuators)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RobotCommand {
    /// Command type identifier
    pub command_type: String,
    /// Target joint positions (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_positions: Option<Vec<f64>>,
    /// Target joint velocities (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_velocities: Option<Vec<f64>>,
    /// Target joint efforts (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_efforts: Option<Vec<f64>>,
    /// Additional command parameters
    #[serde(default)]
    pub parameters: HashMap<String, serde_json::Value>,
}

/// Generic sensor reading
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorReading {
    /// Sensor type (e.g., "force_torque", "imu", "temperature")
    pub sensor_type: String,
    /// Sensor ID
    pub sensor_id: String,
    /// Numeric values
    pub values: Vec<f64>,
    /// Units (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub units: Option<String>,
    /// Coordinate frame (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame_id: Option<String>,
}

/// Event marker for significant occurrences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMarker {
    /// Event type
    pub event_type: EventType,
    /// Human-readable description
    pub description: String,
    /// Severity level
    pub severity: Severity,
    /// Additional event data
    #[serde(default)]
    pub data: HashMap<String, serde_json::Value>,
}

/// Event types for markers
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    /// Teleoperation session started
    TeleopStart,
    /// Teleoperation session ended
    TeleopEnd,
    /// Task execution started
    TaskStart,
    /// Task completed successfully
    TaskComplete,
    /// Task failed
    TaskFailed,
    /// Collision detected
    Collision,
    /// Emergency stop activated
    EStop,
    /// Human intervention required/occurred
    Intervention,
    /// Novel/out-of-distribution situation detected
    NovelSituation,
    /// Error condition
    Error,
    /// Custom event type
    Custom(String),
}

/// Severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Informational
    Info,
    /// Warning
    Warning,
    /// Error
    Error,
    /// Critical/Fatal
    Critical,
}

/// Teleoperation input data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeleopInput {
    /// Input device type
    pub device_type: super::streams::TeleopDeviceType,
    /// Device ID
    pub device_id: String,
    /// Button states (bitmask or named)
    #[serde(default)]
    pub buttons: HashMap<String, bool>,
    /// Axis values (normalized -1 to 1 or 0 to 1)
    #[serde(default)]
    pub axes: HashMap<String, f64>,
    /// Pose inputs (for VR controllers)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pose: Option<super::Transform3D>,
}

/// Priority levels for data and triggers
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    /// Low priority (background collection)
    Low = 0,
    /// Normal priority
    #[default]
    Normal = 1,
    /// High priority
    High = 2,
    /// Critical priority (always capture)
    Critical = 3,
}

/// Quality assessment for data samples
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityMetrics {
    /// Overall quality score (0.0 - 1.0)
    pub overall_score: f32,
    /// Sharpness score for images (0.0 - 1.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sharpness: Option<f32>,
    /// Exposure quality for images (0.0 - 1.0, 0.5 is ideal)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exposure: Option<f32>,
    /// Motion blur detected
    #[serde(default)]
    pub motion_blur: bool,
    /// Data is usable for training
    #[serde(default = "default_true")]
    pub usable: bool,
    /// Reason if not usable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejection_reason: Option<String>,
}

fn default_true() -> bool {
    true
}

impl Default for QualityMetrics {
    fn default() -> Self {
        Self {
            overall_score: 1.0,
            sharpness: None,
            exposure: None,
            motion_blur: false,
            usable: true,
            rejection_reason: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_episode_id() {
        let id1 = EpisodeId::new();
        let id2 = EpisodeId::new();
        assert_ne!(id1, id2);
        assert_eq!(id1.as_str().len(), 36); // UUID format
    }

    #[test]
    fn test_joint_state() {
        let js = JointState::new(
            vec!["joint1".to_string(), "joint2".to_string()],
            vec![1.0, 2.0],
        );
        assert_eq!(js.get_position("joint1"), Some(1.0));
        assert_eq!(js.get_position("joint2"), Some(2.0));
        assert_eq!(js.get_position("joint3"), None);
    }

    #[test]
    fn test_joint_state_map() {
        let js = JointState::new(vec!["a".to_string(), "b".to_string()], vec![1.5, 2.5]);
        let map = js.as_map();
        assert_eq!(map.get("a"), Some(&1.5));
        assert_eq!(map.get("b"), Some(&2.5));
    }

    #[test]
    fn test_sample_data_size() {
        let binary = SampleData::Binary(vec![0u8; 1024]);
        assert!(binary.size_bytes() >= 1024);

        let js = SampleData::JointState(JointState::new(vec!["j1".to_string()], vec![1.0]));
        assert!(js.size_bytes() > 0);
    }
}
