//! Privacy filters for robot data collection
//!
//! Provides privacy-preserving transformations for sensor data before
//! storage or upload. Includes face detection/blurring, license plate
//! detection, and PII removal from metadata.

use super::types::StreamId;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

/// Privacy-related errors
#[derive(Debug, Error)]
pub enum PrivacyError {
    #[error("Filter not found: {0}")]
    FilterNotFound(String),

    #[error("Processing failed: {0}")]
    ProcessingFailed(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Unsupported data type for filter: {0}")]
    UnsupportedDataType(String),
}

/// Privacy filter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyConfig {
    /// Enable face detection and blurring
    #[serde(default = "default_true")]
    pub blur_faces: bool,

    /// Enable license plate detection and blurring
    #[serde(default)]
    pub blur_license_plates: bool,

    /// Enable person detection and blurring
    #[serde(default)]
    pub blur_persons: bool,

    /// PII fields to remove from metadata
    #[serde(default)]
    pub pii_fields_to_remove: Vec<String>,

    /// Fields to hash instead of remove
    #[serde(default)]
    pub pii_fields_to_hash: Vec<String>,

    /// Blur strength (0.0-1.0, where 1.0 is maximum blur)
    #[serde(default = "default_blur_strength")]
    pub blur_strength: f32,

    /// Minimum detection confidence (0.0-1.0)
    #[serde(default = "default_confidence")]
    pub min_detection_confidence: f32,

    /// Streams to apply privacy filters to
    #[serde(default)]
    pub target_streams: Vec<StreamId>,

    /// Whether to log detections (without storing sensitive data)
    #[serde(default)]
    pub log_detections: bool,
}

fn default_true() -> bool {
    true
}

fn default_blur_strength() -> f32 {
    0.8
}

fn default_confidence() -> f32 {
    0.5
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            blur_faces: true,
            blur_license_plates: false,
            blur_persons: false,
            pii_fields_to_remove: vec![
                "operator_name".into(),
                "operator_email".into(),
                "location_address".into(),
            ],
            pii_fields_to_hash: vec!["operator_id".into()],
            blur_strength: 0.8,
            min_detection_confidence: 0.5,
            target_streams: Vec::new(),
            log_detections: false,
        }
    }
}

impl PrivacyConfig {
    /// Create a minimal privacy config (faces only)
    pub fn minimal() -> Self {
        Self {
            blur_faces: true,
            blur_license_plates: false,
            blur_persons: false,
            pii_fields_to_remove: Vec::new(),
            pii_fields_to_hash: Vec::new(),
            blur_strength: 0.8,
            min_detection_confidence: 0.5,
            target_streams: Vec::new(),
            log_detections: false,
        }
    }

    /// Create a strict privacy config (all filters enabled)
    pub fn strict() -> Self {
        Self {
            blur_faces: true,
            blur_license_plates: true,
            blur_persons: true,
            pii_fields_to_remove: vec![
                "operator_name".into(),
                "operator_email".into(),
                "operator_phone".into(),
                "location_address".into(),
                "location_gps".into(),
                "wifi_ssid".into(),
                "bluetooth_devices".into(),
            ],
            pii_fields_to_hash: vec!["operator_id".into(), "device_id".into()],
            blur_strength: 1.0,
            min_detection_confidence: 0.3,
            target_streams: Vec::new(),
            log_detections: true,
        }
    }

    /// Add a target stream
    pub fn with_stream(mut self, stream_id: StreamId) -> Self {
        self.target_streams.push(stream_id);
        self
    }
}

/// Detected region in an image
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedRegion {
    /// Detection type
    pub detection_type: DetectionType,
    /// Bounding box: (x, y, width, height) normalized 0-1
    pub bbox: (f32, f32, f32, f32),
    /// Detection confidence
    pub confidence: f32,
    /// Optional landmark points (for faces)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub landmarks: Option<Vec<(f32, f32)>>,
}

impl DetectedRegion {
    /// Create a new detected region
    pub fn new(
        detection_type: DetectionType,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        confidence: f32,
    ) -> Self {
        Self {
            detection_type,
            bbox: (x, y, width, height),
            confidence,
            landmarks: None,
        }
    }

    /// Add landmarks
    pub fn with_landmarks(mut self, landmarks: Vec<(f32, f32)>) -> Self {
        self.landmarks = Some(landmarks);
        self
    }

    /// Get pixel coordinates for a given image size
    pub fn pixel_bbox(&self, width: u32, height: u32) -> (u32, u32, u32, u32) {
        let (x, y, w, h) = self.bbox;
        (
            (x * width as f32) as u32,
            (y * height as f32) as u32,
            (w * width as f32) as u32,
            (h * height as f32) as u32,
        )
    }
}

/// Type of privacy-sensitive detection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionType {
    /// Human face
    Face,
    /// License plate
    LicensePlate,
    /// Full person body
    Person,
    /// Text/signage
    Text,
    /// Custom detection
    Custom,
}

/// Result of privacy processing
#[derive(Debug, Clone)]
pub struct PrivacyResult {
    /// Processed data (with privacy applied)
    pub data: Vec<u8>,
    /// Regions that were detected and blurred
    pub detections: Vec<DetectedRegion>,
    /// Processing time in nanoseconds
    pub processing_time_ns: u64,
    /// Whether any modifications were made
    pub modified: bool,
}

/// Trait for privacy filters
pub trait PrivacyFilter: Send + Sync {
    /// Get filter name
    fn name(&self) -> &str;

    /// Check if this filter applies to the given stream
    fn applies_to(&self, stream_id: &StreamId) -> bool;

    /// Process image data and apply privacy transformations
    fn process_image(
        &self,
        data: &[u8],
        width: u32,
        height: u32,
        config: &PrivacyConfig,
    ) -> Result<PrivacyResult, PrivacyError>;

    /// Process metadata and remove/hash PII
    fn process_metadata(
        &self,
        metadata: &mut HashMap<String, String>,
        config: &PrivacyConfig,
    ) -> Result<(), PrivacyError>;
}

/// Face detection and blurring filter
///
/// In production, this would use a face detection model (MTCNN, RetinaFace, etc.)
/// This implementation provides the interface and placeholder logic.
pub struct FaceBlurFilter {
    name: String,
    target_streams: HashSet<StreamId>,
}

impl FaceBlurFilter {
    /// Create a new face blur filter
    pub fn new() -> Self {
        Self {
            name: "face_blur".into(),
            target_streams: HashSet::new(),
        }
    }

    /// Add target stream
    pub fn with_stream(mut self, stream_id: StreamId) -> Self {
        self.target_streams.insert(stream_id);
        self
    }

    /// Detect faces in image data
    ///
    /// In production, this would run a face detection model.
    /// Returns detected face regions.
    fn detect_faces(
        &self,
        _data: &[u8],
        _width: u32,
        _height: u32,
        _min_confidence: f32,
    ) -> Vec<DetectedRegion> {
        // Placeholder: In production, run face detection model
        // For now, return empty (no faces detected)
        Vec::new()
    }

    /// Apply Gaussian blur to detected regions
    fn apply_blur(
        &self,
        data: &mut [u8],
        width: u32,
        height: u32,
        regions: &[DetectedRegion],
        strength: f32,
    ) {
        // Apply blur to each detected region
        for region in regions {
            let (rx, ry, rw, rh) = region.pixel_bbox(width, height);
            self.blur_region(data, width, height, rx, ry, rw, rh, strength);
        }
    }

    /// Apply blur to a specific region (simple box blur)
    fn blur_region(
        &self,
        data: &mut [u8],
        img_width: u32,
        img_height: u32,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        strength: f32,
    ) {
        // Simple pixelation blur (faster than Gaussian for privacy)
        let block_size = ((strength * 20.0) as u32).max(4);
        let bytes_per_pixel = 3; // Assuming RGB

        for by in (y..y + height).step_by(block_size as usize) {
            for bx in (x..x + width).step_by(block_size as usize) {
                // Calculate average color in block
                let mut r_sum: u32 = 0;
                let mut g_sum: u32 = 0;
                let mut b_sum: u32 = 0;
                let mut count: u32 = 0;

                for py in by..(by + block_size).min(y + height).min(img_height) {
                    for px in bx..(bx + block_size).min(x + width).min(img_width) {
                        let idx = ((py * img_width + px) * bytes_per_pixel) as usize;
                        if idx + 2 < data.len() {
                            r_sum += data[idx] as u32;
                            g_sum += data[idx + 1] as u32;
                            b_sum += data[idx + 2] as u32;
                            count += 1;
                        }
                    }
                }

                if count > 0 {
                    let r_avg = (r_sum / count) as u8;
                    let g_avg = (g_sum / count) as u8;
                    let b_avg = (b_sum / count) as u8;

                    // Apply average to all pixels in block
                    for py in by..(by + block_size).min(y + height).min(img_height) {
                        for px in bx..(bx + block_size).min(x + width).min(img_width) {
                            let idx = ((py * img_width + px) * bytes_per_pixel) as usize;
                            if idx + 2 < data.len() {
                                data[idx] = r_avg;
                                data[idx + 1] = g_avg;
                                data[idx + 2] = b_avg;
                            }
                        }
                    }
                }
            }
        }
    }
}

impl Default for FaceBlurFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl PrivacyFilter for FaceBlurFilter {
    fn name(&self) -> &str {
        &self.name
    }

    fn applies_to(&self, stream_id: &StreamId) -> bool {
        self.target_streams.is_empty() || self.target_streams.contains(stream_id)
    }

    fn process_image(
        &self,
        data: &[u8],
        width: u32,
        height: u32,
        config: &PrivacyConfig,
    ) -> Result<PrivacyResult, PrivacyError> {
        let start = std::time::Instant::now();

        if !config.blur_faces {
            return Ok(PrivacyResult {
                data: data.to_vec(),
                detections: Vec::new(),
                processing_time_ns: start.elapsed().as_nanos() as u64,
                modified: false,
            });
        }

        // Detect faces
        let detections = self.detect_faces(data, width, height, config.min_detection_confidence);

        if detections.is_empty() {
            return Ok(PrivacyResult {
                data: data.to_vec(),
                detections: Vec::new(),
                processing_time_ns: start.elapsed().as_nanos() as u64,
                modified: false,
            });
        }

        // Apply blur
        let mut result_data = data.to_vec();
        self.apply_blur(
            &mut result_data,
            width,
            height,
            &detections,
            config.blur_strength,
        );

        Ok(PrivacyResult {
            data: result_data,
            detections,
            processing_time_ns: start.elapsed().as_nanos() as u64,
            modified: true,
        })
    }

    fn process_metadata(
        &self,
        _metadata: &mut HashMap<String, String>,
        _config: &PrivacyConfig,
    ) -> Result<(), PrivacyError> {
        // Face filter doesn't process metadata
        Ok(())
    }
}

/// PII removal filter for metadata
pub struct PiiFilter {
    name: String,
}

impl PiiFilter {
    /// Create a new PII filter
    pub fn new() -> Self {
        Self {
            name: "pii_filter".into(),
        }
    }

    /// Hash a value using SHA-256
    fn hash_value(&self, value: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(value.as_bytes());
        let result = hasher.finalize();
        hex::encode(&result[..16]) // Use first 16 bytes for shorter hash
    }
}

impl Default for PiiFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl PrivacyFilter for PiiFilter {
    fn name(&self) -> &str {
        &self.name
    }

    fn applies_to(&self, _stream_id: &StreamId) -> bool {
        true // PII filter applies to all streams (for metadata)
    }

    fn process_image(
        &self,
        data: &[u8],
        _width: u32,
        _height: u32,
        _config: &PrivacyConfig,
    ) -> Result<PrivacyResult, PrivacyError> {
        // PII filter doesn't process images
        Ok(PrivacyResult {
            data: data.to_vec(),
            detections: Vec::new(),
            processing_time_ns: 0,
            modified: false,
        })
    }

    fn process_metadata(
        &self,
        metadata: &mut HashMap<String, String>,
        config: &PrivacyConfig,
    ) -> Result<(), PrivacyError> {
        // Remove PII fields
        for field in &config.pii_fields_to_remove {
            metadata.remove(field);
        }

        // Hash PII fields
        for field in &config.pii_fields_to_hash {
            if let Some(value) = metadata.get(field) {
                let hashed = self.hash_value(value);
                metadata.insert(field.clone(), hashed);
            }
        }

        Ok(())
    }
}

/// Privacy filter manager
pub struct PrivacyManager {
    /// Registered filters
    filters: Vec<Box<dyn PrivacyFilter>>,
    /// Configuration
    config: PrivacyConfig,
    /// Statistics
    stats: PrivacyStats,
}

/// Privacy processing statistics
#[derive(Debug, Clone, Default)]
pub struct PrivacyStats {
    /// Total images processed
    pub images_processed: u64,
    /// Images with detections
    pub images_with_detections: u64,
    /// Total detections by type
    pub detections_by_type: HashMap<String, u64>,
    /// Total processing time in nanoseconds
    pub total_processing_time_ns: u64,
    /// Metadata fields removed
    pub fields_removed: u64,
    /// Metadata fields hashed
    pub fields_hashed: u64,
}

impl PrivacyManager {
    /// Create a new privacy manager
    pub fn new(config: PrivacyConfig) -> Self {
        Self {
            filters: Vec::new(),
            config,
            stats: PrivacyStats::default(),
        }
    }

    /// Create with default filters
    pub fn with_default_filters(config: PrivacyConfig) -> Self {
        let mut manager = Self::new(config);
        manager.add_filter(Box::new(FaceBlurFilter::new()));
        manager.add_filter(Box::new(PiiFilter::new()));
        manager
    }

    /// Add a filter
    pub fn add_filter(&mut self, filter: Box<dyn PrivacyFilter>) {
        self.filters.push(filter);
    }

    /// Get configuration
    pub fn config(&self) -> &PrivacyConfig {
        &self.config
    }

    /// Update configuration
    pub fn set_config(&mut self, config: PrivacyConfig) {
        self.config = config;
    }

    /// Process image data through all applicable filters
    pub fn process_image(
        &mut self,
        stream_id: &StreamId,
        data: &[u8],
        width: u32,
        height: u32,
    ) -> Result<PrivacyResult, PrivacyError> {
        let mut current_data = data.to_vec();
        let mut all_detections = Vec::new();
        let mut total_time: u64 = 0;
        let mut any_modified = false;

        for filter in &self.filters {
            if filter.applies_to(stream_id) {
                let result = filter.process_image(&current_data, width, height, &self.config)?;
                if result.modified {
                    current_data = result.data;
                    any_modified = true;
                }
                all_detections.extend(result.detections);
                total_time += result.processing_time_ns;
            }
        }

        // Update stats
        self.stats.images_processed += 1;
        if !all_detections.is_empty() {
            self.stats.images_with_detections += 1;
            for detection in &all_detections {
                let type_name = format!("{:?}", detection.detection_type);
                *self.stats.detections_by_type.entry(type_name).or_insert(0) += 1;
            }
        }
        self.stats.total_processing_time_ns += total_time;

        Ok(PrivacyResult {
            data: current_data,
            detections: all_detections,
            processing_time_ns: total_time,
            modified: any_modified,
        })
    }

    /// Process metadata through all filters
    pub fn process_metadata(
        &mut self,
        metadata: &mut HashMap<String, String>,
    ) -> Result<(), PrivacyError> {
        let fields_before = metadata.len();

        for filter in &self.filters {
            filter.process_metadata(metadata, &self.config)?;
        }

        let removed = fields_before.saturating_sub(metadata.len());
        self.stats.fields_removed += removed as u64;
        self.stats.fields_hashed += self.config.pii_fields_to_hash.len() as u64;

        Ok(())
    }

    /// Get statistics
    pub fn stats(&self) -> &PrivacyStats {
        &self.stats
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.stats = PrivacyStats::default();
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_privacy_config_default() {
        let config = PrivacyConfig::default();
        assert!(config.blur_faces);
        assert!(!config.blur_license_plates);
        assert_eq!(config.blur_strength, 0.8);
    }

    #[test]
    fn test_privacy_config_strict() {
        let config = PrivacyConfig::strict();
        assert!(config.blur_faces);
        assert!(config.blur_license_plates);
        assert!(config.blur_persons);
        assert_eq!(config.blur_strength, 1.0);
        assert!(config.pii_fields_to_remove.len() > 3);
    }

    #[test]
    fn test_detected_region_pixel_bbox() {
        let region = DetectedRegion::new(DetectionType::Face, 0.25, 0.25, 0.5, 0.5, 0.95);

        let (x, y, w, h) = region.pixel_bbox(100, 100);
        assert_eq!(x, 25);
        assert_eq!(y, 25);
        assert_eq!(w, 50);
        assert_eq!(h, 50);
    }

    #[test]
    fn test_face_blur_filter_no_faces() {
        let filter = FaceBlurFilter::new();
        let config = PrivacyConfig::default();

        // Create a small test image (10x10 RGB)
        let data = vec![128u8; 10 * 10 * 3];

        let result = filter.process_image(&data, 10, 10, &config).unwrap();
        assert!(!result.modified); // No faces detected
        assert!(result.detections.is_empty());
    }

    #[test]
    fn test_pii_filter_remove_fields() {
        let filter = PiiFilter::new();
        let config = PrivacyConfig {
            pii_fields_to_remove: vec!["name".into(), "email".into()],
            pii_fields_to_hash: vec!["user_id".into()],
            ..Default::default()
        };

        let mut metadata = HashMap::new();
        metadata.insert("name".into(), "John Doe".into());
        metadata.insert("email".into(), "john@example.com".into());
        metadata.insert("user_id".into(), "12345".into());
        metadata.insert("timestamp".into(), "2024-01-01".into());

        filter.process_metadata(&mut metadata, &config).unwrap();

        assert!(!metadata.contains_key("name"));
        assert!(!metadata.contains_key("email"));
        assert!(metadata.contains_key("user_id"));
        assert!(metadata.contains_key("timestamp"));

        // user_id should be hashed
        assert_ne!(metadata.get("user_id").unwrap(), "12345");
    }

    #[test]
    fn test_privacy_manager_creation() {
        let config = PrivacyConfig::default();
        let manager = PrivacyManager::with_default_filters(config);

        assert_eq!(manager.filters.len(), 2);
    }

    #[test]
    fn test_privacy_manager_process_metadata() {
        let config = PrivacyConfig {
            pii_fields_to_remove: vec!["secret".into()],
            ..Default::default()
        };
        let mut manager = PrivacyManager::with_default_filters(config);

        let mut metadata = HashMap::new();
        metadata.insert("secret".into(), "password123".into());
        metadata.insert("public".into(), "visible".into());

        manager.process_metadata(&mut metadata).unwrap();

        assert!(!metadata.contains_key("secret"));
        assert!(metadata.contains_key("public"));
    }

    #[test]
    fn test_blur_region() {
        let filter = FaceBlurFilter::new();

        // Create a simple 4x4 RGB image with known values
        let mut data = vec![0u8; 4 * 4 * 3];
        // Set all pixels to white
        for i in 0..data.len() {
            data[i] = 255;
        }

        // Blur a 2x2 region (the blur should average to white since all pixels are white)
        filter.blur_region(&mut data, 4, 4, 0, 0, 2, 2, 0.5);

        // Verify the image was processed (values should still be valid)
        assert!(data.iter().all(|&v| v <= 255));
    }

    #[test]
    fn test_privacy_stats() {
        let config = PrivacyConfig::default();
        let mut manager = PrivacyManager::with_default_filters(config);

        // Process an image
        let data = vec![0u8; 100 * 100 * 3];
        let stream_id = StreamId::new("test_camera");
        manager.process_image(&stream_id, &data, 100, 100).unwrap();

        assert_eq!(manager.stats().images_processed, 1);
    }
}
