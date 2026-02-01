//! Frame Enrichment with Camera Poses
//!
//! Enriches camera frames with computed camera poses from joint state + URDF.
//! This is critical for imitation learning - every frame needs:
//!
//! - Camera-to-world transform
//! - Camera-to-base transform
//! - Joint positions at capture time
//!
//! The enricher maintains a buffer of recent joint states and interpolates
//! to match frame timestamps.

use super::transform::Transform3D;
use super::types::{JointState, QualityMetrics, TimestampNs};
use super::urdf::{CameraPoseStamped, RobotModel, UrdfError};
use std::collections::HashMap;

/// Enriched frame with camera pose and quality metrics
#[derive(Debug, Clone)]
pub struct EnrichedFrame {
    /// Original camera frame data
    pub frame: CameraFrame,
    /// Computed camera pose from FK
    pub camera_pose: CameraPoseStamped,
    /// Quality assessment
    pub quality: QualityMetrics,
}

impl EnrichedFrame {
    /// Check if this frame is usable for training
    pub fn is_usable(&self) -> bool {
        self.quality.usable
    }
}

/// Raw camera frame data
#[derive(Debug, Clone)]
pub struct CameraFrame {
    /// Camera ID
    pub camera_id: String,
    /// Capture timestamp in nanoseconds
    pub timestamp_ns: TimestampNs,
    /// Frame sequence number
    pub sequence: u64,
    /// Image width
    pub width: u32,
    /// Image height
    pub height: u32,
    /// Pixel encoding (e.g., "rgb8", "bgr8", "mono8")
    pub encoding: String,
    /// Raw image data
    pub data: Vec<u8>,
}

impl CameraFrame {
    /// Create a new camera frame
    pub fn new(
        camera_id: impl Into<String>,
        timestamp_ns: TimestampNs,
        width: u32,
        height: u32,
        encoding: impl Into<String>,
        data: Vec<u8>,
    ) -> Self {
        Self {
            camera_id: camera_id.into(),
            timestamp_ns,
            sequence: 0,
            width,
            height,
            encoding: encoding.into(),
            data,
        }
    }

    /// Set sequence number
    pub fn with_sequence(mut self, seq: u64) -> Self {
        self.sequence = seq;
        self
    }

    /// Get expected data size based on dimensions and encoding
    pub fn expected_size(&self) -> usize {
        let bytes_per_pixel = match self.encoding.as_str() {
            "rgb8" | "bgr8" => 3,
            "rgba8" | "bgra8" => 4,
            "mono8" => 1,
            "mono16" | "depth16" => 2,
            "depth32f" => 4,
            _ => 3, // Default assumption
        };
        self.width as usize * self.height as usize * bytes_per_pixel
    }

    /// Check if data size matches expected
    pub fn validate_size(&self) -> bool {
        self.data.len() >= self.expected_size()
    }
}

/// Joint state observation with timestamp
#[derive(Debug, Clone)]
struct TimestampedJointState {
    timestamp_ns: TimestampNs,
    positions: HashMap<String, f64>,
    velocities: Option<HashMap<String, f64>>,
}

/// Frame enricher that adds camera poses to frames
pub struct FrameEnricher {
    /// Robot model for FK computation
    robot_model: RobotModel,
    /// Ring buffer of recent joint states
    joint_state_buffer: Vec<TimestampedJointState>,
    /// Maximum buffer size
    max_buffer_size: usize,
    /// Maximum time window in nanoseconds
    max_duration_ns: u64,
    /// Base pose in world frame (if known)
    base_pose_world: Option<Transform3D>,
    /// Quality threshold settings
    quality_config: QualityConfig,
    /// Statistics
    stats: EnricherStats,
}

/// Configuration for quality assessment
#[derive(Debug, Clone)]
pub struct QualityConfig {
    /// Maximum acceptable timestamp difference for interpolation (ms)
    pub max_interpolation_gap_ms: f64,
    /// Minimum sharpness score to consider usable
    pub min_sharpness: f32,
    /// Enable motion blur detection
    pub detect_motion_blur: bool,
    /// Maximum joint velocity for acceptable blur (rad/s)
    pub max_joint_velocity: f64,
}

impl Default for QualityConfig {
    fn default() -> Self {
        Self {
            max_interpolation_gap_ms: 50.0,
            min_sharpness: 0.3,
            detect_motion_blur: true,
            max_joint_velocity: 2.0, // 2 rad/s is reasonable for most joints
        }
    }
}

/// Statistics about enrichment process
#[derive(Debug, Clone, Default)]
pub struct EnricherStats {
    /// Total frames processed
    pub frames_processed: u64,
    /// Frames successfully enriched
    pub frames_enriched: u64,
    /// Frames with missing joint state
    pub frames_missing_joint_state: u64,
    /// Frames rejected for quality
    pub frames_rejected_quality: u64,
    /// Average interpolation gap in ms
    pub avg_interpolation_gap_ms: f64,
    /// Joint state buffer utilization
    pub buffer_utilization: f32,
}

impl FrameEnricher {
    /// Create a new frame enricher
    ///
    /// # Arguments
    /// * `robot_model` - Robot model for FK computation
    /// * `buffer_duration_ms` - Maximum joint state history to keep (milliseconds)
    pub fn new(robot_model: RobotModel, buffer_duration_ms: u32) -> Self {
        let max_duration_ns = buffer_duration_ms as u64 * 1_000_000;
        // Assume ~100Hz joint state, size buffer accordingly
        let max_buffer_size = (buffer_duration_ms as usize / 10).max(100);

        Self {
            robot_model,
            joint_state_buffer: Vec::with_capacity(max_buffer_size),
            max_buffer_size,
            max_duration_ns,
            base_pose_world: None,
            quality_config: QualityConfig::default(),
            stats: EnricherStats::default(),
        }
    }

    /// Set quality configuration
    pub fn with_quality_config(mut self, config: QualityConfig) -> Self {
        self.quality_config = config;
        self
    }

    /// Set base pose in world frame
    pub fn set_base_pose(&mut self, pose: Transform3D) {
        self.base_pose_world = Some(pose);
    }

    /// Clear base pose (use identity)
    pub fn clear_base_pose(&mut self) {
        self.base_pose_world = None;
    }

    /// Add a joint state observation
    pub fn add_joint_state(&mut self, timestamp_ns: TimestampNs, joint_state: &JointState) {
        let positions = joint_state.as_map();
        let velocities = joint_state.velocities.as_ref().map(|vels| {
            joint_state
                .names
                .iter()
                .cloned()
                .zip(vels.iter().copied())
                .collect()
        });

        self.add_joint_state_map(timestamp_ns, positions, velocities);
    }

    /// Add joint state from HashMap
    pub fn add_joint_state_map(
        &mut self,
        timestamp_ns: TimestampNs,
        positions: HashMap<String, f64>,
        velocities: Option<HashMap<String, f64>>,
    ) {
        let state = TimestampedJointState {
            timestamp_ns,
            positions,
            velocities,
        };

        // Insert in sorted order (by timestamp)
        let insert_idx = self
            .joint_state_buffer
            .binary_search_by_key(&timestamp_ns, |s| s.timestamp_ns)
            .unwrap_or_else(|i| i);

        self.joint_state_buffer.insert(insert_idx, state);

        // Trim buffer if too large
        while self.joint_state_buffer.len() > self.max_buffer_size {
            self.joint_state_buffer.remove(0);
        }

        // Trim old entries
        self.trim_old_entries(timestamp_ns);
    }

    /// Remove entries older than max duration
    fn trim_old_entries(&mut self, current_time_ns: TimestampNs) {
        let cutoff = current_time_ns.saturating_sub(self.max_duration_ns);
        self.joint_state_buffer.retain(|s| s.timestamp_ns >= cutoff);
    }

    /// Get current buffer size
    pub fn buffer_size(&self) -> usize {
        self.joint_state_buffer.len()
    }

    /// Enrich a camera frame with pose information
    pub fn enrich_frame(&mut self, frame: CameraFrame) -> Result<EnrichedFrame, EnrichmentError> {
        self.stats.frames_processed += 1;

        // Interpolate joint state at frame timestamp
        let (joint_positions, interpolation_gap_ns) =
            self.interpolate_joint_state(frame.timestamp_ns)?;

        // Check interpolation gap
        let gap_ms = interpolation_gap_ns as f64 / 1_000_000.0;
        if gap_ms > self.quality_config.max_interpolation_gap_ms {
            return Err(EnrichmentError::InterpolationGapTooLarge {
                gap_ms,
                max_ms: self.quality_config.max_interpolation_gap_ms,
            });
        }

        // Update running average of interpolation gap
        self.stats.avg_interpolation_gap_ms =
            0.9 * self.stats.avg_interpolation_gap_ms + 0.1 * gap_ms;

        // Compute camera pose
        let camera_pose = self.robot_model.compute_camera_pose(
            &frame.camera_id,
            &joint_positions,
            self.base_pose_world.as_ref(),
            frame.timestamp_ns,
        )?;

        // Assess quality
        let quality = self.assess_quality(&frame, &joint_positions);

        self.stats.frames_enriched += 1;
        if !quality.usable {
            self.stats.frames_rejected_quality += 1;
        }

        // Update buffer utilization
        self.stats.buffer_utilization =
            self.joint_state_buffer.len() as f32 / self.max_buffer_size as f32;

        Ok(EnrichedFrame {
            frame,
            camera_pose,
            quality,
        })
    }

    /// Try to enrich, returning None if not possible
    pub fn try_enrich_frame(&mut self, frame: CameraFrame) -> Option<EnrichedFrame> {
        self.enrich_frame(frame).ok()
    }

    /// Interpolate joint state at a specific timestamp
    fn interpolate_joint_state(
        &self,
        timestamp_ns: TimestampNs,
    ) -> Result<(HashMap<String, f64>, u64), EnrichmentError> {
        if self.joint_state_buffer.is_empty() {
            return Err(EnrichmentError::NoJointState);
        }

        // Find the two states surrounding the timestamp
        let idx = self
            .joint_state_buffer
            .binary_search_by_key(&timestamp_ns, |s| s.timestamp_ns);

        match idx {
            Ok(i) => {
                // Exact match
                Ok((self.joint_state_buffer[i].positions.clone(), 0))
            }
            Err(0) => {
                // Before all samples - use first
                let first = &self.joint_state_buffer[0];
                let gap = first.timestamp_ns.saturating_sub(timestamp_ns);
                Ok((first.positions.clone(), gap))
            }
            Err(i) if i >= self.joint_state_buffer.len() => {
                // After all samples - use last
                let last = self.joint_state_buffer.last().unwrap();
                let gap = timestamp_ns.saturating_sub(last.timestamp_ns);
                Ok((last.positions.clone(), gap))
            }
            Err(i) => {
                // Between two samples - interpolate
                let before = &self.joint_state_buffer[i - 1];
                let after = &self.joint_state_buffer[i];

                let gap = (timestamp_ns.saturating_sub(before.timestamp_ns))
                    .min(after.timestamp_ns.saturating_sub(timestamp_ns));

                // Linear interpolation
                let dt = (after.timestamp_ns - before.timestamp_ns) as f64;
                let t = (timestamp_ns - before.timestamp_ns) as f64 / dt;

                let mut interpolated = HashMap::new();
                for (name, &pos_before) in &before.positions {
                    if let Some(&pos_after) = after.positions.get(name) {
                        let interpolated_pos = pos_before + t * (pos_after - pos_before);
                        interpolated.insert(name.clone(), interpolated_pos);
                    } else {
                        interpolated.insert(name.clone(), pos_before);
                    }
                }

                Ok((interpolated, gap))
            }
        }
    }

    /// Assess frame quality
    fn assess_quality(
        &self,
        frame: &CameraFrame,
        _joint_positions: &HashMap<String, f64>,
    ) -> QualityMetrics {
        let mut quality = QualityMetrics {
            overall_score: 1.0,
            sharpness: None,
            exposure: None,
            motion_blur: false,
            usable: true,
            rejection_reason: None,
        };

        // Check for motion blur using joint velocities
        if self.quality_config.detect_motion_blur {
            if let Some(motion_blur) = self.detect_motion_blur(frame.timestamp_ns) {
                quality.motion_blur = motion_blur;
                if motion_blur {
                    quality.overall_score *= 0.5;
                }
            }
        }

        // Validate frame data
        if !frame.validate_size() {
            quality.usable = false;
            quality.overall_score = 0.0;
            quality.rejection_reason = Some("Invalid frame data size".to_string());
        }

        // TODO: Implement actual sharpness detection using image analysis
        // For now, assume frames are sharp if no motion blur
        quality.sharpness = Some(if quality.motion_blur { 0.3 } else { 0.8 });

        quality
    }

    /// Detect motion blur using joint velocities
    fn detect_motion_blur(&self, timestamp_ns: TimestampNs) -> Option<bool> {
        // Find the joint state closest to this timestamp
        let idx = self
            .joint_state_buffer
            .binary_search_by_key(&timestamp_ns, |s| s.timestamp_ns)
            .unwrap_or_else(|i| i.min(self.joint_state_buffer.len().saturating_sub(1)));

        if idx >= self.joint_state_buffer.len() {
            return None;
        }

        let state = &self.joint_state_buffer[idx];
        if let Some(velocities) = &state.velocities {
            // Check if any joint is moving too fast
            let max_velocity = velocities.values().map(|v| v.abs()).fold(0.0, f64::max);
            Some(max_velocity > self.quality_config.max_joint_velocity)
        } else {
            None
        }
    }

    /// Get current statistics
    pub fn stats(&self) -> &EnricherStats {
        &self.stats
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.stats = EnricherStats::default();
    }

    /// Clear the joint state buffer
    pub fn clear_buffer(&mut self) {
        self.joint_state_buffer.clear();
    }

    /// Get the time range covered by the buffer
    pub fn buffer_time_range(&self) -> Option<(TimestampNs, TimestampNs)> {
        if self.joint_state_buffer.is_empty() {
            None
        } else {
            Some((
                self.joint_state_buffer.first().unwrap().timestamp_ns,
                self.joint_state_buffer.last().unwrap().timestamp_ns,
            ))
        }
    }
}

/// Errors during frame enrichment
#[derive(Debug, Clone, thiserror::Error)]
pub enum EnrichmentError {
    #[error("No joint state available")]
    NoJointState,

    #[error("Interpolation gap too large: {gap_ms:.1}ms > {max_ms:.1}ms max")]
    InterpolationGapTooLarge { gap_ms: f64, max_ms: f64 },

    #[error("Camera not configured: {0}")]
    CameraNotConfigured(String),

    #[error("URDF error: {0}")]
    UrdfError(#[from] UrdfError),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_enricher() -> FrameEnricher {
        let mut model = RobotModel::simple_arm("test", 2);
        model
            .register_camera("cam", "link_2", Transform3D::identity())
            .unwrap();
        FrameEnricher::new(model, 1000) // 1 second buffer
    }

    #[test]
    fn test_add_joint_state() {
        let mut enricher = create_test_enricher();
        assert_eq!(enricher.buffer_size(), 0);

        let js = JointState::new(
            vec!["joint_1".to_string(), "joint_2".to_string()],
            vec![0.1, 0.2],
        );
        enricher.add_joint_state(1_000_000_000, &js);

        assert_eq!(enricher.buffer_size(), 1);
    }

    #[test]
    fn test_interpolation_exact() {
        let mut enricher = create_test_enricher();

        let js = JointState::new(
            vec!["joint_1".to_string(), "joint_2".to_string()],
            vec![0.5, 1.0],
        );
        enricher.add_joint_state(1_000_000_000, &js);

        let (positions, gap) = enricher.interpolate_joint_state(1_000_000_000).unwrap();
        assert_eq!(gap, 0);
        assert_eq!(positions.get("joint_1"), Some(&0.5));
        assert_eq!(positions.get("joint_2"), Some(&1.0));
    }

    #[test]
    fn test_interpolation_between() {
        let mut enricher = create_test_enricher();

        enricher.add_joint_state_map(
            1_000_000_000,
            [("joint_1".to_string(), 0.0)].into_iter().collect(),
            None,
        );
        enricher.add_joint_state_map(
            2_000_000_000,
            [("joint_1".to_string(), 1.0)].into_iter().collect(),
            None,
        );

        // Query at midpoint
        let (positions, _gap) = enricher.interpolate_joint_state(1_500_000_000).unwrap();
        let pos = positions.get("joint_1").unwrap();
        assert!((pos - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_enrich_frame() {
        let mut enricher = create_test_enricher();

        // Add joint states
        enricher.add_joint_state_map(
            1_000_000_000,
            [("joint_1".to_string(), 0.0), ("joint_2".to_string(), 0.0)]
                .into_iter()
                .collect(),
            None,
        );

        // Create a frame
        let frame = CameraFrame::new(
            "cam",
            1_000_000_000,
            640,
            480,
            "rgb8",
            vec![0u8; 640 * 480 * 3],
        );

        let enriched = enricher.enrich_frame(frame).unwrap();
        assert_eq!(enriched.camera_pose.camera_id, "cam");
        assert!(enriched.quality.usable);
    }

    #[test]
    fn test_enrich_frame_no_joint_state() {
        let mut enricher = create_test_enricher();
        let frame = CameraFrame::new(
            "cam",
            1_000_000_000,
            640,
            480,
            "rgb8",
            vec![0u8; 640 * 480 * 3],
        );

        let result = enricher.enrich_frame(frame);
        assert!(matches!(result, Err(EnrichmentError::NoJointState)));
    }

    #[test]
    fn test_buffer_time_range() {
        let mut enricher = create_test_enricher();
        assert!(enricher.buffer_time_range().is_none());

        // Use timestamps within the 1 second buffer window
        enricher.add_joint_state_map(1_000_000_000, HashMap::new(), None);
        enricher.add_joint_state_map(1_300_000_000, HashMap::new(), None);
        enricher.add_joint_state_map(1_600_000_000, HashMap::new(), None);

        let (start, end) = enricher.buffer_time_range().unwrap();
        assert_eq!(start, 1_000_000_000);
        assert_eq!(end, 1_600_000_000);
    }

    #[test]
    fn test_motion_blur_detection() {
        let mut enricher = create_test_enricher().with_quality_config(QualityConfig {
            max_joint_velocity: 1.0,
            ..Default::default()
        });

        // Add state with high velocity
        enricher.add_joint_state_map(
            1_000_000_000,
            [("joint_1".to_string(), 0.0)].into_iter().collect(),
            Some([("joint_1".to_string(), 5.0)].into_iter().collect()), // 5 rad/s
        );

        let blur = enricher.detect_motion_blur(1_000_000_000);
        assert_eq!(blur, Some(true));

        // Add state with low velocity
        enricher.add_joint_state_map(
            2_000_000_000,
            [("joint_1".to_string(), 0.0)].into_iter().collect(),
            Some([("joint_1".to_string(), 0.5)].into_iter().collect()), // 0.5 rad/s
        );

        let blur = enricher.detect_motion_blur(2_000_000_000);
        assert_eq!(blur, Some(false));
    }

    #[test]
    fn test_stats() {
        let mut enricher = create_test_enricher();

        enricher.add_joint_state_map(
            1_000_000_000,
            [("joint_1".to_string(), 0.0), ("joint_2".to_string(), 0.0)]
                .into_iter()
                .collect(),
            None,
        );

        let frame = CameraFrame::new(
            "cam",
            1_000_000_000,
            640,
            480,
            "rgb8",
            vec![0u8; 640 * 480 * 3],
        );
        let _ = enricher.enrich_frame(frame);

        let stats = enricher.stats();
        assert_eq!(stats.frames_processed, 1);
        assert_eq!(stats.frames_enriched, 1);
    }
}
