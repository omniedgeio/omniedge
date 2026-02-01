//! Stream configuration types for sensor data channels
//!
//! Defines the configuration for different types of sensor streams
//! that can be captured and stored.

use super::camera_config::PixelFormat;
use super::compression::CompressionAlgorithm;
use super::types::{Priority, StreamId};
use serde::{Deserialize, Serialize};

/// Stream configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamConfig {
    /// Unique stream identifier
    pub stream_id: StreamId,
    /// Stream type and parameters
    pub stream_type: StreamType,
    /// Expected data rate in Hz
    pub frequency_hz: f32,
    /// Buffer duration in seconds (how much history to keep)
    pub buffer_duration_seconds: f32,
    /// Compression settings for this stream
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression: Option<CompressionConfig>,
    /// Priority for bandwidth allocation
    #[serde(default)]
    pub priority: Priority,
    /// Whether this stream is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

impl StreamConfig {
    /// Create a new stream configuration
    pub fn new(stream_id: impl Into<String>, stream_type: StreamType, frequency_hz: f32) -> Self {
        Self {
            stream_id: StreamId::new(stream_id),
            stream_type,
            frequency_hz,
            buffer_duration_seconds: 60.0,
            compression: None,
            priority: Priority::Normal,
            enabled: true,
        }
    }

    /// Set buffer duration
    pub fn with_buffer_duration(mut self, seconds: f32) -> Self {
        self.buffer_duration_seconds = seconds;
        self
    }

    /// Set compression
    pub fn with_compression(mut self, config: CompressionConfig) -> Self {
        self.compression = Some(config);
        self
    }

    /// Set priority
    pub fn with_priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    /// Estimate bytes per second for this stream (uncompressed)
    pub fn estimated_bytes_per_second(&self) -> usize {
        let bytes_per_sample = self.stream_type.estimated_sample_size();
        (bytes_per_sample as f32 * self.frequency_hz) as usize
    }

    /// Estimate buffer size in bytes
    pub fn estimated_buffer_size(&self) -> usize {
        (self.estimated_bytes_per_second() as f32 * self.buffer_duration_seconds) as usize
    }
}

/// Stream type definitions
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamType {
    /// RGB camera stream
    RgbCamera {
        /// Camera ID from camera config
        camera_id: String,
        /// Image width
        width: u32,
        /// Image height
        height: u32,
        /// Pixel encoding
        #[serde(default = "default_rgb8")]
        encoding: PixelFormat,
    },

    /// Depth camera stream
    DepthCamera {
        /// Camera ID
        camera_id: String,
        /// Image width
        width: u32,
        /// Image height
        height: u32,
        /// Minimum depth in meters
        #[serde(default = "default_min_depth")]
        min_depth_m: f32,
        /// Maximum depth in meters
        #[serde(default = "default_max_depth")]
        max_depth_m: f32,
    },

    /// Point cloud stream
    PointCloud {
        /// Source sensor ID
        source_id: String,
        /// Maximum number of points
        max_points: u32,
        /// Whether points have RGB color
        #[serde(default)]
        has_color: bool,
        /// Whether points have normals
        #[serde(default)]
        has_normals: bool,
    },

    /// Joint state stream
    JointState {
        /// Joint names to capture
        joint_names: Vec<String>,
        /// Include velocities
        #[serde(default = "default_true")]
        include_velocities: bool,
        /// Include efforts/torques
        #[serde(default)]
        include_efforts: bool,
    },

    /// Force/torque sensor stream
    ForceTorque {
        /// Sensor ID
        sensor_id: String,
        /// Reference frame
        frame_id: String,
    },

    /// IMU sensor stream
    Imu {
        /// Sensor ID
        sensor_id: String,
        /// Reference frame
        frame_id: String,
        /// Include orientation estimate
        #[serde(default = "default_true")]
        include_orientation: bool,
    },

    /// Wrench (6-axis force/torque) stream
    Wrench {
        /// Sensor ID
        sensor_id: String,
        /// Reference frame
        frame_id: String,
    },

    /// Generic telemetry stream
    Telemetry {
        /// Topic or channel name
        topic: String,
        /// Message type (for documentation)
        message_type: String,
        /// Expected message size in bytes
        #[serde(default)]
        expected_size: usize,
    },

    /// Teleoperation input stream
    TeleopInput {
        /// Device type
        device_type: TeleopDeviceType,
        /// Device ID
        device_id: String,
    },

    /// Audio stream
    Audio {
        /// Microphone ID
        mic_id: String,
        /// Sample rate in Hz
        sample_rate: u32,
        /// Number of channels
        channels: u8,
        /// Bits per sample
        bits_per_sample: u8,
    },

    /// Robot command stream (outgoing commands)
    Command {
        /// Command type identifier
        command_type: String,
    },

    /// Event stream (discrete events)
    Event {
        /// Event source identifier
        source: String,
    },
}

fn default_rgb8() -> PixelFormat {
    PixelFormat::Rgb8
}

fn default_min_depth() -> f32 {
    0.1
}

fn default_max_depth() -> f32 {
    10.0
}

impl StreamType {
    /// Estimate the size of a single sample in bytes
    pub fn estimated_sample_size(&self) -> usize {
        match self {
            StreamType::RgbCamera {
                width,
                height,
                encoding,
                ..
            } => {
                let bpp = encoding.bytes_per_pixel();
                (*width as usize) * (*height as usize) * bpp
            }
            StreamType::DepthCamera { width, height, .. } => {
                // 16-bit depth
                (*width as usize) * (*height as usize) * 2
            }
            StreamType::PointCloud {
                max_points,
                has_color,
                has_normals,
                ..
            } => {
                let mut bytes_per_point = 12; // XYZ float32
                if *has_color {
                    bytes_per_point += 4; // RGBA
                }
                if *has_normals {
                    bytes_per_point += 12; // Normal XYZ float32
                }
                *max_points as usize * bytes_per_point
            }
            StreamType::JointState {
                joint_names,
                include_velocities,
                include_efforts,
                ..
            } => {
                let num_joints = joint_names.len();
                let mut size = num_joints * 8; // positions (f64)
                if *include_velocities {
                    size += num_joints * 8;
                }
                if *include_efforts {
                    size += num_joints * 8;
                }
                size + 64 // overhead for names, timestamp
            }
            StreamType::ForceTorque { .. } => 48 + 32, // 6 f64 values + overhead
            StreamType::Imu {
                include_orientation,
                ..
            } => {
                let mut size = 72; // accel (3) + gyro (3) as f64
                if *include_orientation {
                    size += 32; // quaternion
                }
                size + 32 // overhead
            }
            StreamType::Wrench { .. } => 48 + 32, // 6 f64 values + overhead
            StreamType::Telemetry { expected_size, .. } => {
                if *expected_size > 0 {
                    *expected_size
                } else {
                    256 // default estimate
                }
            }
            StreamType::TeleopInput { .. } => 256, // estimate
            StreamType::Audio {
                sample_rate,
                channels,
                bits_per_sample,
                ..
            } => {
                // Per-chunk size (assuming 20ms chunks)
                let samples_per_chunk = *sample_rate as usize / 50;
                samples_per_chunk * (*channels as usize) * (*bits_per_sample as usize / 8)
            }
            StreamType::Command { .. } => 512, // estimate
            StreamType::Event { .. } => 256,   // estimate
        }
    }

    /// Get a human-readable type name
    pub fn type_name(&self) -> &'static str {
        match self {
            StreamType::RgbCamera { .. } => "rgb_camera",
            StreamType::DepthCamera { .. } => "depth_camera",
            StreamType::PointCloud { .. } => "point_cloud",
            StreamType::JointState { .. } => "joint_state",
            StreamType::ForceTorque { .. } => "force_torque",
            StreamType::Imu { .. } => "imu",
            StreamType::Wrench { .. } => "wrench",
            StreamType::Telemetry { .. } => "telemetry",
            StreamType::TeleopInput { .. } => "teleop_input",
            StreamType::Audio { .. } => "audio",
            StreamType::Command { .. } => "command",
            StreamType::Event { .. } => "event",
        }
    }
}

/// Teleoperation device types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TeleopDeviceType {
    /// 3Dconnexion SpaceMouse
    SpaceMouse,
    /// VR motion controller (Quest, Vive, etc.)
    VrController,
    /// Keyboard input
    Keyboard,
    /// Gamepad/joystick
    Gamepad,
    /// Haptic device (Phantom, etc.)
    HapticDevice,
    /// Leader arm (for bilateral teleoperation)
    LeaderArm,
    /// Custom device
    Custom(String),
}

/// Compression configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    /// Compression algorithm
    pub algorithm: StreamCompression,
    /// Compression level (algorithm-specific, typically 1-22)
    #[serde(default = "default_compression_level")]
    pub level: u8,
}

fn default_compression_level() -> u8 {
    3
}

impl CompressionConfig {
    /// Create zstd compression config
    pub fn zstd(level: u8) -> Self {
        Self {
            algorithm: StreamCompression::Zstd,
            level,
        }
    }

    /// Create lz4 compression config
    pub fn lz4() -> Self {
        Self {
            algorithm: StreamCompression::Lz4,
            level: 1,
        }
    }

    /// Create JPEG compression config for images
    pub fn jpeg(quality: u8) -> Self {
        Self {
            algorithm: StreamCompression::Jpeg,
            level: quality,
        }
    }
}

/// Compression algorithms for streams
/// Note: Uses a subset compatible with compression module
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StreamCompression {
    /// No compression
    None,
    /// Zstandard (good ratio, fast)
    #[default]
    Zstd,
    /// LZ4 (very fast, lower ratio)
    Lz4,
    /// JPEG (for images, lossy)
    Jpeg,
    /// PNG (for images, lossless)
    Png,
    /// H.264 video codec
    H264,
    /// H.265/HEVC video codec
    H265,
}

impl From<StreamCompression> for CompressionAlgorithm {
    fn from(sc: StreamCompression) -> Self {
        match sc {
            StreamCompression::None => CompressionAlgorithm::None,
            StreamCompression::Zstd => CompressionAlgorithm::Zstd,
            StreamCompression::Lz4 => CompressionAlgorithm::Lz4,
            StreamCompression::Jpeg => CompressionAlgorithm::Jpeg,
            StreamCompression::Png => CompressionAlgorithm::Png,
            // Video codecs fall back to none for now
            StreamCompression::H264 | StreamCompression::H265 => CompressionAlgorithm::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_config_creation() {
        let config = StreamConfig::new(
            "joint_states",
            StreamType::JointState {
                joint_names: vec!["j1".to_string(), "j2".to_string()],
                include_velocities: true,
                include_efforts: false,
            },
            1000.0,
        );

        assert_eq!(config.stream_id.as_str(), "joint_states");
        assert_eq!(config.frequency_hz, 1000.0);
        assert!(config.enabled);
    }

    #[test]
    fn test_rgb_camera_size_estimate() {
        let stream_type = StreamType::RgbCamera {
            camera_id: "cam1".to_string(),
            width: 1920,
            height: 1080,
            encoding: PixelFormat::Rgb8,
        };

        let size = stream_type.estimated_sample_size();
        assert_eq!(size, 1920 * 1080 * 3); // ~6.2 MB per frame
    }

    #[test]
    fn test_stream_buffer_size() {
        let config = StreamConfig::new(
            "rgb",
            StreamType::RgbCamera {
                camera_id: "cam1".to_string(),
                width: 640,
                height: 480,
                encoding: PixelFormat::Rgb8,
            },
            30.0,
        )
        .with_buffer_duration(60.0);

        let buffer_size = config.estimated_buffer_size();
        // 640*480*3 * 30 * 60 = ~1.6 GB
        assert!(buffer_size > 1_000_000_000);
    }

    #[test]
    fn test_compression_config() {
        let config = CompressionConfig::zstd(6);
        assert_eq!(config.algorithm, StreamCompression::Zstd);
        assert_eq!(config.level, 6);
    }
}
