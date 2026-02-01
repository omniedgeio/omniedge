//! URDF Integration for Camera-Joint Mapping
//!
//! Provides forward kinematics computation for determining camera poses from
//! joint states. This is critical for AI training data - every frame needs
//! the camera-to-world transform computed via FK from joint state + URDF model.
//!
//! # Coordinate Frame Conventions
//!
//! - **Robot base frame**: As defined in URDF (typically X forward, Z up)
//! - **Link frames**: As defined in URDF joint/link structure
//! - **Optical frame**: Z forward, X right, Y down (standard camera convention)
//!
//! The `link_to_optical` transform converts from URDF link frame to camera optical frame.

use super::camera_config::CameraSystemConfig;
use super::transform::{Rotation3D, Transform3D};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Robot model with forward kinematics capabilities
pub struct RobotModel {
    /// Robot name from URDF
    name: String,

    /// Links in the robot
    links: Vec<LinkInfo>,

    /// Joints in the robot
    joints: Vec<JointInfo>,

    /// Link name to index mapping
    link_indices: HashMap<String, usize>,

    /// Joint name to index mapping
    joint_indices: HashMap<String, usize>,

    /// Registered cameras with their link associations
    camera_links: HashMap<String, CameraLinkInfo>,

    /// Parent link index for each link (-1 for root)
    parent_link: Vec<i32>,

    /// Joint connecting to parent (None for root)
    parent_joint: Vec<Option<usize>>,
}

/// Information about a robot link
#[derive(Debug, Clone)]
pub struct LinkInfo {
    /// Link name
    pub name: String,
    /// Link index in the model
    pub index: usize,
}

/// Information about a robot joint
#[derive(Debug, Clone)]
pub struct JointInfo {
    /// Joint name
    pub name: String,
    /// Joint index in the model
    pub index: usize,
    /// Joint type
    pub joint_type: JointType,
    /// Parent link index
    pub parent_link_idx: usize,
    /// Child link index
    pub child_link_idx: usize,
    /// Origin transform (parent to joint frame)
    pub origin: Transform3D,
    /// Joint axis (for revolute/prismatic)
    pub axis: [f64; 3],
    /// Joint limits
    pub limits: Option<JointLimits>,
}

/// Joint types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JointType {
    /// Fixed joint (no motion)
    Fixed,
    /// Revolute joint (rotation around axis)
    Revolute,
    /// Continuous joint (revolute without limits)
    Continuous,
    /// Prismatic joint (translation along axis)
    Prismatic,
    /// Floating joint (6 DOF)
    Floating,
    /// Planar joint
    Planar,
}

/// Joint limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JointLimits {
    /// Lower position limit
    pub lower: f64,
    /// Upper position limit
    pub upper: f64,
    /// Velocity limit
    pub velocity: f64,
    /// Effort limit
    pub effort: f64,
}

/// Information about a camera mounted on a link
#[derive(Debug, Clone)]
pub struct CameraLinkInfo {
    /// Camera ID from config
    pub camera_id: String,
    /// URDF link name the camera is mounted to
    pub link_name: String,
    /// Link index in the model
    pub link_idx: usize,
    /// Transform from link frame to camera optical frame
    pub link_to_optical: Transform3D,
    /// Kinematic chain from base to this link (joint indices)
    pub chain_joints: Vec<usize>,
}

/// Camera pose with timestamp and joint state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraPoseStamped {
    /// Timestamp in nanoseconds
    pub timestamp_ns: u64,
    /// Camera ID
    pub camera_id: String,
    /// Camera optical frame pose in world coordinates
    pub pose_world_optical: Transform3D,
    /// Camera optical frame pose relative to robot base
    pub pose_base_optical: Transform3D,
    /// Joint positions used for this computation
    pub joint_positions: Vec<f64>,
    /// Joint names (for verification/debugging)
    pub joint_names: Vec<String>,
}

impl RobotModel {
    /// Create an empty robot model
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            links: Vec::new(),
            joints: Vec::new(),
            link_indices: HashMap::new(),
            joint_indices: HashMap::new(),
            camera_links: HashMap::new(),
            parent_link: Vec::new(),
            parent_joint: Vec::new(),
        }
    }

    /// Get robot name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Add a link to the model
    pub fn add_link(&mut self, name: impl Into<String>) -> usize {
        let name = name.into();
        let index = self.links.len();
        self.link_indices.insert(name.clone(), index);
        self.links.push(LinkInfo { name, index });
        self.parent_link.push(-1);
        self.parent_joint.push(None);
        index
    }

    /// Add a joint to the model
    pub fn add_joint(
        &mut self,
        name: impl Into<String>,
        joint_type: JointType,
        parent_link: &str,
        child_link: &str,
        origin: Transform3D,
        axis: [f64; 3],
        limits: Option<JointLimits>,
    ) -> Result<usize, UrdfError> {
        let parent_idx = *self
            .link_indices
            .get(parent_link)
            .ok_or_else(|| UrdfError::LinkNotFound(parent_link.to_string()))?;
        let child_idx = *self
            .link_indices
            .get(child_link)
            .ok_or_else(|| UrdfError::LinkNotFound(child_link.to_string()))?;

        let name = name.into();
        let index = self.joints.len();
        self.joint_indices.insert(name.clone(), index);
        self.joints.push(JointInfo {
            name,
            index,
            joint_type,
            parent_link_idx: parent_idx,
            child_link_idx: child_idx,
            origin,
            axis,
            limits,
        });

        // Update parent info
        self.parent_link[child_idx] = parent_idx as i32;
        self.parent_joint[child_idx] = Some(index);

        Ok(index)
    }

    /// Build a simple robot model for testing
    pub fn simple_arm(name: &str, num_joints: usize) -> Self {
        let mut model = Self::new(name);

        // Add base link
        model.add_link("base_link");

        // Add chain of links and joints
        for i in 0..num_joints {
            let link_name = format!("link_{}", i + 1);
            let joint_name = format!("joint_{}", i + 1);
            let parent = if i == 0 {
                "base_link".to_string()
            } else {
                format!("link_{}", i)
            };

            model.add_link(&link_name);
            model
                .add_joint(
                    &joint_name,
                    JointType::Revolute,
                    &parent,
                    &link_name,
                    Transform3D::from_translation(0.0, 0.0, 0.1), // 10cm link length
                    [0.0, 0.0, 1.0],                              // Z-axis rotation
                    Some(JointLimits {
                        lower: -std::f64::consts::PI,
                        upper: std::f64::consts::PI,
                        velocity: 2.0,
                        effort: 100.0,
                    }),
                )
                .unwrap();
        }

        model
    }

    /// Get list of joint names
    pub fn joint_names(&self) -> Vec<String> {
        self.joints.iter().map(|j| j.name.clone()).collect()
    }

    /// Get list of link names
    pub fn link_names(&self) -> Vec<String> {
        self.links.iter().map(|l| l.name.clone()).collect()
    }

    /// Get number of joints
    pub fn num_joints(&self) -> usize {
        self.joints.len()
    }

    /// Get number of links
    pub fn num_links(&self) -> usize {
        self.links.len()
    }

    /// Get joint info by name
    pub fn get_joint(&self, name: &str) -> Option<&JointInfo> {
        self.joint_indices.get(name).map(|&i| &self.joints[i])
    }

    /// Get link info by name
    pub fn get_link(&self, name: &str) -> Option<&LinkInfo> {
        self.link_indices.get(name).map(|&i| &self.links[i])
    }

    /// Register a camera mounted on a link
    pub fn register_camera(
        &mut self,
        camera_id: impl Into<String>,
        link_name: &str,
        link_to_optical: Transform3D,
    ) -> Result<(), UrdfError> {
        let camera_id = camera_id.into();
        let link_idx = *self
            .link_indices
            .get(link_name)
            .ok_or_else(|| UrdfError::LinkNotFound(link_name.to_string()))?;

        // Compute kinematic chain from base to this link
        let chain_joints = self.compute_chain(link_idx)?;

        self.camera_links.insert(
            camera_id.clone(),
            CameraLinkInfo {
                camera_id,
                link_name: link_name.to_string(),
                link_idx,
                link_to_optical,
                chain_joints,
            },
        );

        Ok(())
    }

    /// Register cameras from a CameraSystemConfig
    pub fn register_cameras_from_config(
        &mut self,
        config: &CameraSystemConfig,
    ) -> Result<Vec<String>, UrdfError> {
        let mut registered = Vec::new();

        for camera in &config.cameras {
            if let Some(link_name) = &camera.urdf_link {
                let link_to_optical = camera.get_link_to_optical();
                self.register_camera(&camera.camera_id, link_name, link_to_optical)?;
                registered.push(camera.camera_id.clone());
            }
        }

        Ok(registered)
    }

    /// Get list of registered camera IDs
    pub fn camera_ids(&self) -> Vec<String> {
        self.camera_links.keys().cloned().collect()
    }

    /// Compute the kinematic chain (list of joints) from base to a link
    fn compute_chain(&self, link_idx: usize) -> Result<Vec<usize>, UrdfError> {
        let mut chain = Vec::new();
        let mut current = link_idx;

        while self.parent_link[current] >= 0 {
            if let Some(joint_idx) = self.parent_joint[current] {
                chain.push(joint_idx);
            }
            current = self.parent_link[current] as usize;
        }

        chain.reverse();
        Ok(chain)
    }

    /// Compute transform from base to a link given joint positions
    pub fn compute_link_pose(
        &self,
        link_name: &str,
        joint_positions: &HashMap<String, f64>,
    ) -> Result<Transform3D, UrdfError> {
        let link_idx = *self
            .link_indices
            .get(link_name)
            .ok_or_else(|| UrdfError::LinkNotFound(link_name.to_string()))?;

        let chain = self.compute_chain(link_idx)?;
        self.compute_chain_transform(&chain, joint_positions)
    }

    /// Compute transform for a kinematic chain
    fn compute_chain_transform(
        &self,
        chain: &[usize],
        joint_positions: &HashMap<String, f64>,
    ) -> Result<Transform3D, UrdfError> {
        let mut transform = Transform3D::identity();

        for &joint_idx in chain {
            let joint = &self.joints[joint_idx];
            let joint_transform = self.compute_joint_transform(joint, joint_positions)?;
            transform = transform.compose(&joint_transform);
        }

        Ok(transform)
    }

    /// Compute the transform contributed by a single joint
    fn compute_joint_transform(
        &self,
        joint: &JointInfo,
        joint_positions: &HashMap<String, f64>,
    ) -> Result<Transform3D, UrdfError> {
        // Start with the joint origin transform
        let mut transform = joint.origin.clone();

        // Apply joint motion if not fixed
        match joint.joint_type {
            JointType::Fixed => {
                // No additional transform
            }
            JointType::Revolute | JointType::Continuous => {
                let position = joint_positions.get(&joint.name).copied().unwrap_or(0.0);

                // Check limits for revolute joints
                if joint.joint_type == JointType::Revolute {
                    if let Some(limits) = &joint.limits {
                        if position < limits.lower || position > limits.upper {
                            log::warn!(
                                "Joint '{}' position {} outside limits [{}, {}]",
                                joint.name,
                                position,
                                limits.lower,
                                limits.upper
                            );
                        }
                    }
                }

                // Rotation around joint axis
                let rotation = rotation_from_axis_angle(joint.axis, position);
                let joint_motion = Transform3D {
                    translation: [0.0, 0.0, 0.0],
                    rotation,
                };
                transform = transform.compose(&joint_motion);
            }
            JointType::Prismatic => {
                let position = joint_positions.get(&joint.name).copied().unwrap_or(0.0);

                // Check limits
                if let Some(limits) = &joint.limits {
                    if position < limits.lower || position > limits.upper {
                        log::warn!(
                            "Joint '{}' position {} outside limits [{}, {}]",
                            joint.name,
                            position,
                            limits.lower,
                            limits.upper
                        );
                    }
                }

                // Translation along joint axis
                let joint_motion = Transform3D {
                    translation: [
                        joint.axis[0] * position,
                        joint.axis[1] * position,
                        joint.axis[2] * position,
                    ],
                    rotation: Rotation3D::identity(),
                };
                transform = transform.compose(&joint_motion);
            }
            JointType::Floating | JointType::Planar => {
                // Not yet implemented - these require special handling
                log::warn!(
                    "Joint type {:?} not fully implemented for joint '{}'",
                    joint.joint_type,
                    joint.name
                );
            }
        }

        Ok(transform)
    }

    /// Compute camera pose given joint state
    pub fn compute_camera_pose(
        &self,
        camera_id: &str,
        joint_positions: &HashMap<String, f64>,
        base_pose_world: Option<&Transform3D>,
        timestamp_ns: u64,
    ) -> Result<CameraPoseStamped, UrdfError> {
        let camera_info = self
            .camera_links
            .get(camera_id)
            .ok_or_else(|| UrdfError::CameraNotRegistered(camera_id.to_string()))?;

        // Compute transform from base to camera link
        let base_to_link =
            self.compute_chain_transform(&camera_info.chain_joints, joint_positions)?;

        // Apply link-to-optical transform
        let pose_base_optical = base_to_link.compose(&camera_info.link_to_optical);

        // Apply world-to-base transform if provided
        let pose_world_optical = if let Some(base_pose) = base_pose_world {
            base_pose.compose(&pose_base_optical)
        } else {
            pose_base_optical.clone()
        };

        // Collect joint positions and names for the chain
        let chain_joint_names: Vec<String> = camera_info
            .chain_joints
            .iter()
            .map(|&i| self.joints[i].name.clone())
            .collect();

        let chain_joint_positions: Vec<f64> = chain_joint_names
            .iter()
            .map(|name| joint_positions.get(name).copied().unwrap_or(0.0))
            .collect();

        Ok(CameraPoseStamped {
            timestamp_ns,
            camera_id: camera_id.to_string(),
            pose_world_optical,
            pose_base_optical,
            joint_positions: chain_joint_positions,
            joint_names: chain_joint_names,
        })
    }

    /// Compute all registered camera poses at once (more efficient)
    pub fn compute_all_camera_poses(
        &self,
        joint_positions: &HashMap<String, f64>,
        base_pose_world: Option<&Transform3D>,
        timestamp_ns: u64,
    ) -> Vec<Result<CameraPoseStamped, UrdfError>> {
        self.camera_links
            .keys()
            .map(|camera_id| {
                self.compute_camera_pose(camera_id, joint_positions, base_pose_world, timestamp_ns)
            })
            .collect()
    }

    /// Compute all camera poses, returning only successful ones
    pub fn compute_all_camera_poses_ok(
        &self,
        joint_positions: &HashMap<String, f64>,
        base_pose_world: Option<&Transform3D>,
        timestamp_ns: u64,
    ) -> Vec<CameraPoseStamped> {
        self.compute_all_camera_poses(joint_positions, base_pose_world, timestamp_ns)
            .into_iter()
            .filter_map(|r| r.ok())
            .collect()
    }
}

/// Create rotation from axis-angle representation
fn rotation_from_axis_angle(axis: [f64; 3], angle: f64) -> Rotation3D {
    let half = angle / 2.0;
    let s = half.sin();
    let norm = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();

    if norm < 1e-10 {
        return Rotation3D::identity();
    }

    Rotation3D::Quaternion {
        x: axis[0] / norm * s,
        y: axis[1] / norm * s,
        z: axis[2] / norm * s,
        w: half.cos(),
    }
}

/// URDF-related errors
#[derive(Debug, Clone, thiserror::Error)]
pub enum UrdfError {
    #[error("Failed to parse URDF: {0}")]
    ParseError(String),

    #[error("Link not found: {0}")]
    LinkNotFound(String),

    #[error("Joint not found: {0}")]
    JointNotFound(String),

    #[error("Camera not registered: {0}")]
    CameraNotRegistered(String),

    #[error("IO error: {0}")]
    IoError(String),

    #[error("Invalid kinematic chain")]
    InvalidChain,
}

/// Parse URDF from string (stub - requires urdf_rs crate)
pub fn parse_urdf_string(_urdf_xml: &str) -> Result<RobotModel, UrdfError> {
    // This is a stub. Full implementation requires the urdf_rs crate.
    // For now, return an error indicating the feature is not available.
    Err(UrdfError::ParseError(
        "URDF parsing requires the 'urdf_rs' feature. Use RobotModel::simple_arm() for testing."
            .to_string(),
    ))
}

/// Parse URDF from file (stub - requires urdf_rs crate)
pub fn parse_urdf_file(path: &std::path::Path) -> Result<RobotModel, UrdfError> {
    let urdf_xml = std::fs::read_to_string(path).map_err(|e| UrdfError::IoError(e.to_string()))?;
    parse_urdf_string(&urdf_xml)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    const EPSILON: f64 = 1e-6;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < EPSILON
    }

    #[test]
    fn test_simple_arm_creation() {
        let model = RobotModel::simple_arm("test_arm", 3);
        assert_eq!(model.name(), "test_arm");
        assert_eq!(model.num_joints(), 3);
        assert_eq!(model.num_links(), 4); // base + 3 links
    }

    #[test]
    fn test_joint_names() {
        let model = RobotModel::simple_arm("test_arm", 3);
        let names = model.joint_names();
        assert_eq!(names, vec!["joint_1", "joint_2", "joint_3"]);
    }

    #[test]
    fn test_link_names() {
        let model = RobotModel::simple_arm("test_arm", 3);
        let names = model.link_names();
        assert_eq!(names, vec!["base_link", "link_1", "link_2", "link_3"]);
    }

    #[test]
    fn test_register_camera() {
        let mut model = RobotModel::simple_arm("test_arm", 3);
        let result = model.register_camera("end_effector_cam", "link_3", Transform3D::identity());
        assert!(result.is_ok());
        assert!(model.camera_ids().contains(&"end_effector_cam".to_string()));
    }

    #[test]
    fn test_register_camera_invalid_link() {
        let mut model = RobotModel::simple_arm("test_arm", 3);
        let result = model.register_camera("cam", "nonexistent_link", Transform3D::identity());
        assert!(matches!(result, Err(UrdfError::LinkNotFound(_))));
    }

    #[test]
    fn test_compute_link_pose_zero() {
        let model = RobotModel::simple_arm("test_arm", 2);
        let joint_positions: HashMap<String, f64> = HashMap::new();

        // At zero position, link_2 should be at z = 0.2 (two 0.1m links)
        let pose = model.compute_link_pose("link_2", &joint_positions).unwrap();
        assert!(approx_eq(pose.translation[2], 0.2));
        assert!(approx_eq(pose.translation[0], 0.0));
        assert!(approx_eq(pose.translation[1], 0.0));
    }

    #[test]
    fn test_compute_link_pose_rotated() {
        let model = RobotModel::simple_arm("test_arm", 1);
        let mut joint_positions = HashMap::new();
        joint_positions.insert("joint_1".to_string(), PI / 2.0); // 90 degrees

        // After 90 degree rotation around Z, the link should still be at z=0.1
        // but the frame should be rotated
        let pose = model.compute_link_pose("link_1", &joint_positions).unwrap();
        assert!(approx_eq(pose.translation[2], 0.1));
    }

    #[test]
    fn test_compute_camera_pose() {
        let mut model = RobotModel::simple_arm("test_arm", 2);
        model
            .register_camera("cam", "link_2", Transform3D::identity())
            .unwrap();

        let joint_positions: HashMap<String, f64> = HashMap::new();
        let pose = model
            .compute_camera_pose("cam", &joint_positions, None, 1000)
            .unwrap();

        assert_eq!(pose.camera_id, "cam");
        assert_eq!(pose.timestamp_ns, 1000);
        assert!(approx_eq(pose.pose_base_optical.translation[2], 0.2));
    }

    #[test]
    fn test_compute_all_camera_poses() {
        let mut model = RobotModel::simple_arm("test_arm", 3);
        model
            .register_camera("cam1", "link_1", Transform3D::identity())
            .unwrap();
        model
            .register_camera("cam2", "link_3", Transform3D::identity())
            .unwrap();

        let joint_positions: HashMap<String, f64> = HashMap::new();
        let poses = model.compute_all_camera_poses_ok(&joint_positions, None, 2000);

        assert_eq!(poses.len(), 2);
    }

    #[test]
    fn test_with_base_pose() {
        let mut model = RobotModel::simple_arm("test_arm", 1);
        model
            .register_camera("cam", "link_1", Transform3D::identity())
            .unwrap();

        let joint_positions: HashMap<String, f64> = HashMap::new();
        let base_pose = Transform3D::from_translation(1.0, 2.0, 3.0);

        let pose = model
            .compute_camera_pose("cam", &joint_positions, Some(&base_pose), 1000)
            .unwrap();

        // World pose should include base offset
        assert!(approx_eq(pose.pose_world_optical.translation[0], 1.0));
        assert!(approx_eq(pose.pose_world_optical.translation[1], 2.0));
        assert!(approx_eq(pose.pose_world_optical.translation[2], 3.1)); // 3.0 + 0.1

        // Base-relative pose should not include base offset
        assert!(approx_eq(pose.pose_base_optical.translation[0], 0.0));
        assert!(approx_eq(pose.pose_base_optical.translation[1], 0.0));
        assert!(approx_eq(pose.pose_base_optical.translation[2], 0.1));
    }
}
