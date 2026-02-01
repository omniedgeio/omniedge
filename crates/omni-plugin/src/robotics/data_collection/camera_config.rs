//! Camera Device Configuration System
//!
//! Provides stable camera identification across reboots using multiple matching
//! strategies. Linux `/dev/videoX` indices are non-deterministic, so we support:
//!
//! - USB path matching (recommended)
//! - Serial number matching (most reliable)
//! - udev symlink matching
//! - USB VID:PID with port matching
//!
//! # Example Configuration
//!
//! ```yaml
//! cameras:
//!   - camera_id: head_rgb
//!     position: head
//!     device_match:
//!       type: usb_path
//!       path: "1-2.3:1.0"
//!     camera_type: rgb
//!     capture:
//!       width: 1920
//!       height: 1080
//!       fps: 30.0
//!     urdf_link: head_camera_link
//!     link_to_optical_frame:
//!       translation: [0.0, 0.0, 0.0]
//!       rotation:
//!         type: quaternion
//!         x: 0.0
//!         y: 0.0
//!         z: 0.0
//!         w: 1.0
//! ```

use super::transform::Transform3D;
use serde::{Deserialize, Serialize};

/// Root configuration for all cameras on the robot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraSystemConfig {
    /// Individual camera configurations
    pub cameras: Vec<CameraDeviceConfig>,

    /// Stereo pair definitions
    #[serde(default)]
    pub stereo_pairs: Vec<StereoPairConfig>,

    /// Hardware synchronization groups
    #[serde(default)]
    pub sync_groups: Vec<SyncGroupConfig>,

    /// Default settings applied to all cameras
    #[serde(default)]
    pub defaults: CameraDefaults,
}

impl CameraSystemConfig {
    /// Create empty configuration
    pub fn new() -> Self {
        Self {
            cameras: Vec::new(),
            stereo_pairs: Vec::new(),
            sync_groups: Vec::new(),
            defaults: CameraDefaults::default(),
        }
    }

    /// Add a camera configuration
    pub fn add_camera(mut self, camera: CameraDeviceConfig) -> Self {
        self.cameras.push(camera);
        self
    }

    /// Get camera by ID
    pub fn get_camera(&self, camera_id: &str) -> Option<&CameraDeviceConfig> {
        self.cameras.iter().find(|c| c.camera_id == camera_id)
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        // Check for duplicate camera IDs
        let mut seen_ids = std::collections::HashSet::new();
        for camera in &self.cameras {
            if !seen_ids.insert(&camera.camera_id) {
                return Err(ConfigValidationError::DuplicateCameraId(
                    camera.camera_id.clone(),
                ));
            }
        }

        // Validate stereo pairs reference valid cameras
        for pair in &self.stereo_pairs {
            if self.get_camera(&pair.left_camera_id).is_none() {
                return Err(ConfigValidationError::InvalidCameraReference(
                    pair.left_camera_id.clone(),
                ));
            }
            if self.get_camera(&pair.right_camera_id).is_none() {
                return Err(ConfigValidationError::InvalidCameraReference(
                    pair.right_camera_id.clone(),
                ));
            }
        }

        // Validate sync groups reference valid cameras
        for group in &self.sync_groups {
            for cam_id in &group.camera_ids {
                if self.get_camera(cam_id).is_none() {
                    return Err(ConfigValidationError::InvalidCameraReference(
                        cam_id.clone(),
                    ));
                }
            }
        }

        // Validate individual cameras
        for camera in &self.cameras {
            camera.validate()?;
        }

        Ok(())
    }

    /// Get camera IDs
    pub fn camera_ids(&self) -> Vec<&str> {
        self.cameras.iter().map(|c| c.camera_id.as_str()).collect()
    }
}

impl Default for CameraSystemConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for a single camera device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraDeviceConfig {
    /// Unique identifier for this camera (e.g., "head_rgb", "wrist_left")
    pub camera_id: String,

    /// Semantic position on the robot
    pub position: CameraPosition,

    /// Strategy for matching this camera to a physical device
    pub device_match: DeviceMatchStrategy,

    /// Camera type (RGB, depth, stereo, etc.)
    pub camera_type: CameraType,

    /// Capture settings (resolution, FPS, format)
    #[serde(default)]
    pub capture: CaptureSettings,

    /// URDF link this camera is mounted to
    #[serde(skip_serializing_if = "Option::is_none")]
    pub urdf_link: Option<String>,

    /// Static transform from URDF link frame to camera optical frame
    /// Optical frame convention: Z forward, X right, Y down
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_to_optical_frame: Option<Transform3D>,

    /// Camera intrinsic parameters (if known)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intrinsics: Option<CameraIntrinsics>,

    /// Whether this camera is enabled for capture
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Additional camera-specific settings
    #[serde(default)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

fn default_true() -> bool {
    true
}

impl CameraDeviceConfig {
    /// Create a new camera configuration
    pub fn new(
        camera_id: impl Into<String>,
        position: CameraPosition,
        device_match: DeviceMatchStrategy,
        camera_type: CameraType,
    ) -> Self {
        Self {
            camera_id: camera_id.into(),
            position,
            device_match,
            camera_type,
            capture: CaptureSettings::default(),
            urdf_link: None,
            link_to_optical_frame: None,
            intrinsics: None,
            enabled: true,
            extra: std::collections::HashMap::new(),
        }
    }

    /// Set URDF link
    pub fn with_urdf_link(mut self, link: impl Into<String>) -> Self {
        self.urdf_link = Some(link.into());
        self
    }

    /// Set link-to-optical transform
    pub fn with_optical_transform(mut self, transform: Transform3D) -> Self {
        self.link_to_optical_frame = Some(transform);
        self
    }

    /// Set intrinsics
    pub fn with_intrinsics(mut self, intrinsics: CameraIntrinsics) -> Self {
        self.intrinsics = Some(intrinsics);
        self
    }

    /// Validate this camera configuration
    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        if self.camera_id.is_empty() {
            return Err(ConfigValidationError::EmptyCameraId);
        }

        // Warn about non-deterministic strategies (not an error, but logged)
        if matches!(self.device_match, DeviceMatchStrategy::DeviceIndex { .. }) {
            log::warn!(
                "Camera '{}' uses DeviceIndex matching which is non-deterministic across reboots",
                self.camera_id
            );
        }

        // Validate capture settings
        if self.capture.width == 0 || self.capture.height == 0 {
            return Err(ConfigValidationError::InvalidCaptureSettings(
                self.camera_id.clone(),
                "Resolution must be non-zero".to_string(),
            ));
        }

        if self.capture.fps <= 0.0 {
            return Err(ConfigValidationError::InvalidCaptureSettings(
                self.camera_id.clone(),
                "FPS must be positive".to_string(),
            ));
        }

        Ok(())
    }

    /// Check if this camera has URDF integration
    pub fn has_urdf_integration(&self) -> bool {
        self.urdf_link.is_some()
    }

    /// Get the link-to-optical transform, or identity if not specified
    pub fn get_link_to_optical(&self) -> Transform3D {
        self.link_to_optical_frame
            .clone()
            .unwrap_or_else(Transform3D::identity)
    }
}

/// Strategy for matching a camera to a physical device
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DeviceMatchStrategy {
    /// Match by device index (NOT recommended - non-deterministic across reboots)
    DeviceIndex {
        /// Device index (e.g., 0 for /dev/video0)
        index: u32,
    },

    /// Match by device path (e.g., "/dev/video0")
    /// Note: Still somewhat fragile as paths can change
    DevicePath {
        /// Full device path
        path: String,
    },

    /// Match by USB bus path (RECOMMENDED for most cases)
    /// Example: "1-2.3:1.0" representing USB bus topology
    UsbPath {
        /// USB path in sysfs format
        path: String,
    },

    /// Match by camera serial number (MOST RELIABLE)
    /// Not all cameras expose serial numbers
    SerialNumber {
        /// Serial number string
        serial: String,
    },

    /// Match by device name pattern (substring or regex)
    DeviceName {
        /// Name pattern to match
        name_pattern: String,
        /// Whether pattern is a regex
        #[serde(default)]
        is_regex: bool,
    },

    /// Match by USB Vendor ID and Product ID
    UsbVidPid {
        /// USB Vendor ID
        vendor_id: u16,
        /// USB Product ID
        product_id: u16,
        /// Optional USB port path for disambiguation
        #[serde(skip_serializing_if = "Option::is_none")]
        usb_port: Option<String>,
    },

    /// Match by udev symlink (requires udev rules setup)
    /// Example: "/dev/camera_head" created by custom udev rule
    UdevSymlink {
        /// Symlink path
        symlink: String,
    },

    /// Match by V4L2 device capabilities
    V4l2Capabilities {
        /// Required capabilities (e.g., "video_capture")
        capabilities: Vec<String>,
        /// Optional driver name filter
        #[serde(skip_serializing_if = "Option::is_none")]
        driver: Option<String>,
    },
}

impl DeviceMatchStrategy {
    /// Check if this strategy is deterministic across reboots
    pub fn is_deterministic(&self) -> bool {
        match self {
            DeviceMatchStrategy::DeviceIndex { .. } => false,
            DeviceMatchStrategy::DevicePath { .. } => false, // Paths can change
            DeviceMatchStrategy::UsbPath { .. } => true,
            DeviceMatchStrategy::SerialNumber { .. } => true,
            DeviceMatchStrategy::DeviceName { .. } => true, // Usually stable
            DeviceMatchStrategy::UsbVidPid { usb_port, .. } => usb_port.is_some(),
            DeviceMatchStrategy::UdevSymlink { .. } => true,
            DeviceMatchStrategy::V4l2Capabilities { .. } => false, // Multiple matches possible
        }
    }

    /// Get a description of this strategy for logging
    pub fn description(&self) -> String {
        match self {
            DeviceMatchStrategy::DeviceIndex { index } => {
                format!("device index {}", index)
            }
            DeviceMatchStrategy::DevicePath { path } => {
                format!("device path '{}'", path)
            }
            DeviceMatchStrategy::UsbPath { path } => {
                format!("USB path '{}'", path)
            }
            DeviceMatchStrategy::SerialNumber { serial } => {
                format!("serial number '{}'", serial)
            }
            DeviceMatchStrategy::DeviceName { name_pattern, .. } => {
                format!("device name matching '{}'", name_pattern)
            }
            DeviceMatchStrategy::UsbVidPid {
                vendor_id,
                product_id,
                usb_port,
            } => {
                let port_info = usb_port
                    .as_ref()
                    .map(|p| format!(" at port {}", p))
                    .unwrap_or_default();
                format!("USB {:04x}:{:04x}{}", vendor_id, product_id, port_info)
            }
            DeviceMatchStrategy::UdevSymlink { symlink } => {
                format!("udev symlink '{}'", symlink)
            }
            DeviceMatchStrategy::V4l2Capabilities { capabilities, .. } => {
                format!("V4L2 capabilities: {:?}", capabilities)
            }
        }
    }
}

/// Semantic camera position on the robot
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CameraPosition {
    /// Head-mounted camera
    Head,
    /// Chest/torso camera
    Chest,
    /// Left wrist/gripper camera
    WristLeft,
    /// Right wrist/gripper camera
    WristRight,
    /// External/room camera
    External,
    /// Shoulder-mounted
    ShoulderLeft,
    ShoulderRight,
    /// Eye cameras (for humanoids)
    EyeLeft,
    EyeRight,
    /// Custom position
    Custom(String),
}

impl std::fmt::Display for CameraPosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CameraPosition::Head => write!(f, "head"),
            CameraPosition::Chest => write!(f, "chest"),
            CameraPosition::WristLeft => write!(f, "wrist_left"),
            CameraPosition::WristRight => write!(f, "wrist_right"),
            CameraPosition::External => write!(f, "external"),
            CameraPosition::ShoulderLeft => write!(f, "shoulder_left"),
            CameraPosition::ShoulderRight => write!(f, "shoulder_right"),
            CameraPosition::EyeLeft => write!(f, "eye_left"),
            CameraPosition::EyeRight => write!(f, "eye_right"),
            CameraPosition::Custom(name) => write!(f, "{}", name),
        }
    }
}

/// Camera type/modality
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CameraType {
    /// Standard RGB camera
    Rgb,
    /// Depth camera (e.g., structured light, ToF)
    Depth,
    /// Combined RGB-D camera (e.g., RealSense, Kinect)
    Rgbd,
    /// Stereo camera pair
    Stereo,
    /// Fisheye/wide-angle camera
    Fisheye,
    /// Infrared camera
    Infrared,
    /// Thermal/FLIR camera
    Thermal,
    /// Event camera (DVS)
    Event,
}

/// Capture settings for a camera
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureSettings {
    /// Image width in pixels
    pub width: u32,
    /// Image height in pixels
    pub height: u32,
    /// Frames per second
    pub fps: f32,
    /// Pixel format
    pub format: PixelFormat,
    /// Enable auto-exposure
    #[serde(default = "default_true")]
    pub auto_exposure: bool,
    /// Enable auto white balance
    #[serde(default = "default_true")]
    pub auto_white_balance: bool,
    /// Fixed exposure time in microseconds (if auto_exposure is false)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exposure_us: Option<u32>,
    /// Fixed gain (if auto_exposure is false)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gain: Option<f32>,
}

impl Default for CaptureSettings {
    fn default() -> Self {
        Self {
            width: 640,
            height: 480,
            fps: 30.0,
            format: PixelFormat::Rgb8,
            auto_exposure: true,
            auto_white_balance: true,
            exposure_us: None,
            gain: None,
        }
    }
}

impl CaptureSettings {
    /// Create HD 720p settings
    pub fn hd_720p(fps: f32) -> Self {
        Self {
            width: 1280,
            height: 720,
            fps,
            ..Default::default()
        }
    }

    /// Create Full HD 1080p settings
    pub fn full_hd(fps: f32) -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps,
            ..Default::default()
        }
    }

    /// Create 4K settings
    pub fn uhd_4k(fps: f32) -> Self {
        Self {
            width: 3840,
            height: 2160,
            fps,
            ..Default::default()
        }
    }

    /// Estimate bytes per frame (uncompressed)
    pub fn bytes_per_frame(&self) -> usize {
        let bytes_per_pixel = self.format.bytes_per_pixel();
        self.width as usize * self.height as usize * bytes_per_pixel
    }

    /// Estimate bytes per second (uncompressed)
    pub fn bytes_per_second(&self) -> usize {
        (self.bytes_per_frame() as f32 * self.fps) as usize
    }
}

/// Pixel format
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PixelFormat {
    /// 8-bit RGB
    Rgb8,
    /// 8-bit BGR (OpenCV default)
    Bgr8,
    /// 8-bit RGBA
    Rgba8,
    /// 8-bit grayscale
    Mono8,
    /// 16-bit grayscale (for depth)
    Mono16,
    /// YUYV (packed YUV 4:2:2)
    Yuyv,
    /// MJPEG compressed
    Mjpeg,
    /// H.264 compressed
    H264,
    /// H.265/HEVC compressed
    H265,
    /// 16-bit depth (in millimeters)
    Depth16,
    /// 32-bit float depth (in meters)
    Depth32f,
    /// Bayer pattern RGGB
    BayerRggb8,
    /// Bayer pattern BGGR
    BayerBggr8,
}

impl PixelFormat {
    /// Get bytes per pixel (for uncompressed formats)
    pub fn bytes_per_pixel(&self) -> usize {
        match self {
            PixelFormat::Rgb8 | PixelFormat::Bgr8 => 3,
            PixelFormat::Rgba8 => 4,
            PixelFormat::Mono8 | PixelFormat::BayerRggb8 | PixelFormat::BayerBggr8 => 1,
            PixelFormat::Mono16 | PixelFormat::Depth16 | PixelFormat::Yuyv => 2,
            PixelFormat::Depth32f => 4,
            // Compressed formats - estimate
            PixelFormat::Mjpeg | PixelFormat::H264 | PixelFormat::H265 => 1,
        }
    }

    /// Check if format is compressed
    pub fn is_compressed(&self) -> bool {
        matches!(
            self,
            PixelFormat::Mjpeg | PixelFormat::H264 | PixelFormat::H265
        )
    }

    /// Check if format is a depth format
    pub fn is_depth(&self) -> bool {
        matches!(self, PixelFormat::Depth16 | PixelFormat::Depth32f)
    }
}

/// Stereo camera pair configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StereoPairConfig {
    /// Pair identifier
    pub pair_id: String,
    /// Left camera ID
    pub left_camera_id: String,
    /// Right camera ID
    pub right_camera_id: String,
    /// Baseline distance in meters
    pub baseline_meters: f64,
    /// Whether cameras are hardware synchronized
    #[serde(default)]
    pub hardware_sync: bool,
}

/// Hardware synchronization group configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncGroupConfig {
    /// Group identifier
    pub group_id: String,
    /// Camera IDs in this sync group
    pub camera_ids: Vec<String>,
    /// Synchronization mode
    pub sync_mode: SyncMode,
    /// Master camera (for master-slave sync)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub master_camera_id: Option<String>,
}

/// Synchronization modes
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncMode {
    /// Hardware trigger synchronization
    Hardware,
    /// Software timestamp synchronization
    Software,
    /// No synchronization
    None,
    /// PTP (Precision Time Protocol)
    Ptp,
}

/// Default camera settings
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CameraDefaults {
    /// Default capture settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture: Option<CaptureSettings>,
    /// Default compression settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression: Option<CompressionSettings>,
    /// Default buffer duration in seconds
    #[serde(default = "default_buffer_duration")]
    pub buffer_duration_seconds: f32,
}

fn default_buffer_duration() -> f32 {
    60.0
}

/// Compression settings for stored video
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionSettings {
    /// Codec to use
    pub codec: VideoCodec,
    /// Quality setting (0-100, codec-dependent)
    pub quality: u8,
    /// Target bitrate in kbps (optional, for rate control)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitrate_kbps: Option<u32>,
}

/// Video compression codecs
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VideoCodec {
    /// No compression (raw frames)
    None,
    /// JPEG compression (per-frame)
    Jpeg,
    /// H.264/AVC
    H264,
    /// H.265/HEVC
    H265,
    /// VP9
    Vp9,
    /// AV1
    Av1,
}

/// Camera intrinsic parameters (pinhole model with distortion)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraIntrinsics {
    /// Focal length X in pixels
    pub fx: f64,
    /// Focal length Y in pixels
    pub fy: f64,
    /// Principal point X in pixels
    pub cx: f64,
    /// Principal point Y in pixels
    pub cy: f64,
    /// Distortion model
    #[serde(default)]
    pub distortion_model: DistortionModel,
    /// Distortion coefficients
    #[serde(default)]
    pub distortion_coeffs: Vec<f64>,
    /// Image width (for validation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    /// Image height (for validation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
}

impl CameraIntrinsics {
    /// Create simple pinhole intrinsics (no distortion)
    pub fn pinhole(fx: f64, fy: f64, cx: f64, cy: f64) -> Self {
        Self {
            fx,
            fy,
            cx,
            cy,
            distortion_model: DistortionModel::None,
            distortion_coeffs: Vec::new(),
            width: None,
            height: None,
        }
    }

    /// Create intrinsics with plumb-bob distortion
    pub fn with_plumb_bob(mut self, k1: f64, k2: f64, p1: f64, p2: f64, k3: f64) -> Self {
        self.distortion_model = DistortionModel::PlumbBob;
        self.distortion_coeffs = vec![k1, k2, p1, p2, k3];
        self
    }

    /// Get camera matrix as 3x3 array
    pub fn camera_matrix(&self) -> [[f64; 3]; 3] {
        [
            [self.fx, 0.0, self.cx],
            [0.0, self.fy, self.cy],
            [0.0, 0.0, 1.0],
        ]
    }

    /// Project 3D point to 2D (without distortion)
    pub fn project(&self, point_3d: [f64; 3]) -> Option<[f64; 2]> {
        if point_3d[2] <= 0.0 {
            return None; // Behind camera
        }
        let x = point_3d[0] / point_3d[2];
        let y = point_3d[1] / point_3d[2];
        Some([self.fx * x + self.cx, self.fy * y + self.cy])
    }

    /// Unproject 2D point to 3D ray direction
    pub fn unproject(&self, point_2d: [f64; 2]) -> [f64; 3] {
        let x = (point_2d[0] - self.cx) / self.fx;
        let y = (point_2d[1] - self.cy) / self.fy;
        // Normalize to unit vector
        let norm = (x * x + y * y + 1.0).sqrt();
        [x / norm, y / norm, 1.0 / norm]
    }
}

/// Distortion models
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistortionModel {
    /// No distortion
    #[default]
    None,
    /// Plumb-bob model (k1, k2, p1, p2, k3)
    PlumbBob,
    /// Rational polynomial model
    Rational,
    /// Equidistant fisheye model
    Equidistant,
    /// Kannala-Brandt fisheye model
    Fisheye,
}

/// Configuration validation errors
#[derive(Debug, Clone, thiserror::Error)]
pub enum ConfigValidationError {
    #[error("Duplicate camera ID: {0}")]
    DuplicateCameraId(String),

    #[error("Invalid camera reference: {0}")]
    InvalidCameraReference(String),

    #[error("Empty camera ID")]
    EmptyCameraId,

    #[error("Invalid capture settings for camera '{0}': {1}")]
    InvalidCaptureSettings(String, String),

    #[error("URDF link not found: {0}")]
    UrdfLinkNotFound(String),

    #[error("Parse error: {0}")]
    ParseError(String),
}

/// Result of attempting to match a camera to a device
#[derive(Debug, Clone)]
pub struct DeviceMatchResult {
    /// Camera ID from config
    pub camera_id: String,
    /// Whether a device was found
    pub matched: bool,
    /// Matched device path (e.g., /dev/video0)
    pub device_path: Option<String>,
    /// USB path if available
    pub usb_path: Option<String>,
    /// Serial number if available
    pub serial: Option<String>,
    /// Device name/description
    pub device_name: Option<String>,
    /// Match strategy used
    pub strategy: String,
    /// Warning message (e.g., non-deterministic strategy)
    pub warning: Option<String>,
}

impl DeviceMatchResult {
    /// Create a successful match result
    pub fn matched(camera_id: impl Into<String>, device_path: impl Into<String>) -> Self {
        Self {
            camera_id: camera_id.into(),
            matched: true,
            device_path: Some(device_path.into()),
            usb_path: None,
            serial: None,
            device_name: None,
            strategy: String::new(),
            warning: None,
        }
    }

    /// Create a failed match result
    pub fn not_found(camera_id: impl Into<String>, strategy: impl Into<String>) -> Self {
        Self {
            camera_id: camera_id.into(),
            matched: false,
            device_path: None,
            usb_path: None,
            serial: None,
            device_name: None,
            strategy: strategy.into(),
            warning: None,
        }
    }

    /// Add a warning
    pub fn with_warning(mut self, warning: impl Into<String>) -> Self {
        self.warning = Some(warning.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camera_config_validation() {
        let config = CameraSystemConfig::new()
            .add_camera(CameraDeviceConfig::new(
                "head_rgb",
                CameraPosition::Head,
                DeviceMatchStrategy::UsbPath {
                    path: "1-2.3:1.0".to_string(),
                },
                CameraType::Rgb,
            ))
            .add_camera(CameraDeviceConfig::new(
                "wrist_left",
                CameraPosition::WristLeft,
                DeviceMatchStrategy::SerialNumber {
                    serial: "ABC123".to_string(),
                },
                CameraType::Rgb,
            ));

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_duplicate_camera_id() {
        let config = CameraSystemConfig::new()
            .add_camera(CameraDeviceConfig::new(
                "camera1",
                CameraPosition::Head,
                DeviceMatchStrategy::DeviceIndex { index: 0 },
                CameraType::Rgb,
            ))
            .add_camera(CameraDeviceConfig::new(
                "camera1", // Duplicate!
                CameraPosition::Chest,
                DeviceMatchStrategy::DeviceIndex { index: 1 },
                CameraType::Rgb,
            ));

        assert!(matches!(
            config.validate(),
            Err(ConfigValidationError::DuplicateCameraId(_))
        ));
    }

    #[test]
    fn test_capture_settings() {
        let settings = CaptureSettings::full_hd(30.0);
        assert_eq!(settings.width, 1920);
        assert_eq!(settings.height, 1080);
        assert_eq!(settings.fps, 30.0);

        // 1080p RGB @ 30fps = ~178 MB/s uncompressed
        let bps = settings.bytes_per_second();
        assert!(bps > 170_000_000 && bps < 190_000_000);
    }

    #[test]
    fn test_intrinsics_projection() {
        let intrinsics = CameraIntrinsics::pinhole(500.0, 500.0, 320.0, 240.0);

        // Project a point 1m in front of camera, centered
        let point_3d = [0.0, 0.0, 1.0];
        let point_2d = intrinsics.project(point_3d).unwrap();
        assert!((point_2d[0] - 320.0).abs() < 1e-10);
        assert!((point_2d[1] - 240.0).abs() < 1e-10);

        // Project a point 1m in front, 1m to the right
        let point_3d = [1.0, 0.0, 1.0];
        let point_2d = intrinsics.project(point_3d).unwrap();
        assert!((point_2d[0] - 820.0).abs() < 1e-10); // 320 + 500*1
        assert!((point_2d[1] - 240.0).abs() < 1e-10);
    }

    #[test]
    fn test_device_match_deterministic() {
        assert!(DeviceMatchStrategy::UsbPath {
            path: "1-2".to_string()
        }
        .is_deterministic());
        assert!(DeviceMatchStrategy::SerialNumber {
            serial: "ABC".to_string()
        }
        .is_deterministic());
        assert!(!DeviceMatchStrategy::DeviceIndex { index: 0 }.is_deterministic());
        assert!(!DeviceMatchStrategy::UsbVidPid {
            vendor_id: 0x1234,
            product_id: 0x5678,
            usb_port: None
        }
        .is_deterministic());
        assert!(DeviceMatchStrategy::UsbVidPid {
            vendor_id: 0x1234,
            product_id: 0x5678,
            usb_port: Some("1-2".to_string())
        }
        .is_deterministic());
    }
}
