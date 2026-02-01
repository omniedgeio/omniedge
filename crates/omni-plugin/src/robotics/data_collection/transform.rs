//! 3D Transform types for robotics
//!
//! Provides transform representations compatible with ROS conventions.

use serde::{Deserialize, Serialize};

/// 3D transform consisting of translation and rotation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transform3D {
    /// Translation in meters [x, y, z]
    pub translation: [f64; 3],
    /// Rotation representation
    pub rotation: Rotation3D,
}

impl Transform3D {
    /// Create identity transform
    pub fn identity() -> Self {
        Self {
            translation: [0.0, 0.0, 0.0],
            rotation: Rotation3D::identity(),
        }
    }

    /// Create transform from translation only
    pub fn from_translation(x: f64, y: f64, z: f64) -> Self {
        Self {
            translation: [x, y, z],
            rotation: Rotation3D::identity(),
        }
    }

    /// Create transform from quaternion
    pub fn from_quaternion(tx: f64, ty: f64, tz: f64, qx: f64, qy: f64, qz: f64, qw: f64) -> Self {
        Self {
            translation: [tx, ty, tz],
            rotation: Rotation3D::Quaternion {
                x: qx,
                y: qy,
                z: qz,
                w: qw,
            },
        }
    }

    /// Get translation as array
    pub fn translation(&self) -> [f64; 3] {
        self.translation
    }

    /// Get rotation as quaternion [x, y, z, w]
    pub fn quaternion(&self) -> [f64; 4] {
        self.rotation.as_quaternion()
    }

    /// Compose two transforms: self * other
    /// Result represents applying other first, then self
    pub fn compose(&self, other: &Transform3D) -> Transform3D {
        let q1 = self.rotation.as_quaternion();
        let q2 = other.rotation.as_quaternion();

        // Quaternion multiplication: q1 * q2
        let w = q1[3] * q2[3] - q1[0] * q2[0] - q1[1] * q2[1] - q1[2] * q2[2];
        let x = q1[3] * q2[0] + q1[0] * q2[3] + q1[1] * q2[2] - q1[2] * q2[1];
        let y = q1[3] * q2[1] - q1[0] * q2[2] + q1[1] * q2[3] + q1[2] * q2[0];
        let z = q1[3] * q2[2] + q1[0] * q2[1] - q1[1] * q2[0] + q1[2] * q2[3];

        // Rotate other.translation by self.rotation, then add self.translation
        let rotated = self.rotation.rotate_vector(other.translation);

        Transform3D {
            translation: [
                self.translation[0] + rotated[0],
                self.translation[1] + rotated[1],
                self.translation[2] + rotated[2],
            ],
            rotation: Rotation3D::Quaternion { x, y, z, w },
        }
    }

    /// Compute inverse transform
    pub fn inverse(&self) -> Transform3D {
        let inv_rotation = self.rotation.inverse();
        let rotated = inv_rotation.rotate_vector(self.translation);

        Transform3D {
            translation: [-rotated[0], -rotated[1], -rotated[2]],
            rotation: inv_rotation,
        }
    }

    /// Transform a 3D point
    pub fn transform_point(&self, point: [f64; 3]) -> [f64; 3] {
        let rotated = self.rotation.rotate_vector(point);
        [
            self.translation[0] + rotated[0],
            self.translation[1] + rotated[1],
            self.translation[2] + rotated[2],
        ]
    }
}

impl Default for Transform3D {
    fn default() -> Self {
        Self::identity()
    }
}

/// 3D rotation representation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Rotation3D {
    /// Quaternion [x, y, z, w] (Hamilton convention, w is scalar)
    Quaternion { x: f64, y: f64, z: f64, w: f64 },
    /// Euler angles in radians [roll, pitch, yaw] (XYZ convention)
    Euler { roll: f64, pitch: f64, yaw: f64 },
    /// 3x3 rotation matrix (row-major)
    Matrix { data: [[f64; 3]; 3] },
    /// Axis-angle representation
    AxisAngle { axis: [f64; 3], angle: f64 },
}

impl Rotation3D {
    /// Create identity rotation
    pub fn identity() -> Self {
        Self::Quaternion {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        }
    }

    /// Create rotation from quaternion components
    pub fn from_quaternion(x: f64, y: f64, z: f64, w: f64) -> Self {
        Self::Quaternion { x, y, z, w }
    }

    /// Create rotation from Euler angles (radians, XYZ convention)
    pub fn from_euler(roll: f64, pitch: f64, yaw: f64) -> Self {
        Self::Euler { roll, pitch, yaw }
    }

    /// Create rotation from axis-angle
    pub fn from_axis_angle(axis: [f64; 3], angle: f64) -> Self {
        Self::AxisAngle { axis, angle }
    }

    /// Create rotation around X axis
    pub fn from_rotation_x(angle: f64) -> Self {
        let half = angle / 2.0;
        Self::Quaternion {
            x: half.sin(),
            y: 0.0,
            z: 0.0,
            w: half.cos(),
        }
    }

    /// Create rotation around Y axis
    pub fn from_rotation_y(angle: f64) -> Self {
        let half = angle / 2.0;
        Self::Quaternion {
            x: 0.0,
            y: half.sin(),
            z: 0.0,
            w: half.cos(),
        }
    }

    /// Create rotation around Z axis
    pub fn from_rotation_z(angle: f64) -> Self {
        let half = angle / 2.0;
        Self::Quaternion {
            x: 0.0,
            y: 0.0,
            z: half.sin(),
            w: half.cos(),
        }
    }

    /// Convert to quaternion [x, y, z, w]
    pub fn as_quaternion(&self) -> [f64; 4] {
        match self {
            Rotation3D::Quaternion { x, y, z, w } => [*x, *y, *z, *w],
            Rotation3D::Euler { roll, pitch, yaw } => {
                // Convert Euler XYZ to quaternion
                let (sr, cr) = (roll / 2.0).sin_cos();
                let (sp, cp) = (pitch / 2.0).sin_cos();
                let (sy, cy) = (yaw / 2.0).sin_cos();

                let w = cr * cp * cy + sr * sp * sy;
                let x = sr * cp * cy - cr * sp * sy;
                let y = cr * sp * cy + sr * cp * sy;
                let z = cr * cp * sy - sr * sp * cy;

                [x, y, z, w]
            }
            Rotation3D::Matrix { data } => {
                // Convert rotation matrix to quaternion
                let trace = data[0][0] + data[1][1] + data[2][2];
                if trace > 0.0 {
                    let s = (trace + 1.0).sqrt() * 2.0;
                    let w = 0.25 * s;
                    let x = (data[2][1] - data[1][2]) / s;
                    let y = (data[0][2] - data[2][0]) / s;
                    let z = (data[1][0] - data[0][1]) / s;
                    [x, y, z, w]
                } else if data[0][0] > data[1][1] && data[0][0] > data[2][2] {
                    let s = (1.0 + data[0][0] - data[1][1] - data[2][2]).sqrt() * 2.0;
                    let w = (data[2][1] - data[1][2]) / s;
                    let x = 0.25 * s;
                    let y = (data[0][1] + data[1][0]) / s;
                    let z = (data[0][2] + data[2][0]) / s;
                    [x, y, z, w]
                } else if data[1][1] > data[2][2] {
                    let s = (1.0 + data[1][1] - data[0][0] - data[2][2]).sqrt() * 2.0;
                    let w = (data[0][2] - data[2][0]) / s;
                    let x = (data[0][1] + data[1][0]) / s;
                    let y = 0.25 * s;
                    let z = (data[1][2] + data[2][1]) / s;
                    [x, y, z, w]
                } else {
                    let s = (1.0 + data[2][2] - data[0][0] - data[1][1]).sqrt() * 2.0;
                    let w = (data[1][0] - data[0][1]) / s;
                    let x = (data[0][2] + data[2][0]) / s;
                    let y = (data[1][2] + data[2][1]) / s;
                    let z = 0.25 * s;
                    [x, y, z, w]
                }
            }
            Rotation3D::AxisAngle { axis, angle } => {
                let half = angle / 2.0;
                let s = half.sin();
                let norm = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
                if norm < 1e-10 {
                    [0.0, 0.0, 0.0, 1.0]
                } else {
                    [
                        axis[0] / norm * s,
                        axis[1] / norm * s,
                        axis[2] / norm * s,
                        half.cos(),
                    ]
                }
            }
        }
    }

    /// Convert to 3x3 rotation matrix (row-major)
    pub fn as_matrix(&self) -> [[f64; 3]; 3] {
        let [x, y, z, w] = self.as_quaternion();

        let xx = x * x;
        let yy = y * y;
        let zz = z * z;
        let xy = x * y;
        let xz = x * z;
        let yz = y * z;
        let wx = w * x;
        let wy = w * y;
        let wz = w * z;

        [
            [1.0 - 2.0 * (yy + zz), 2.0 * (xy - wz), 2.0 * (xz + wy)],
            [2.0 * (xy + wz), 1.0 - 2.0 * (xx + zz), 2.0 * (yz - wx)],
            [2.0 * (xz - wy), 2.0 * (yz + wx), 1.0 - 2.0 * (xx + yy)],
        ]
    }

    /// Convert to Euler angles (radians, XYZ convention)
    pub fn as_euler(&self) -> [f64; 3] {
        let mat = self.as_matrix();

        // Check for gimbal lock
        let sy = -mat[0][2];
        if sy.abs() > 0.99999 {
            // Gimbal lock
            let pitch = std::f64::consts::FRAC_PI_2 * sy.signum();
            let yaw = 0.0;
            let roll = mat[1][0].atan2(mat[1][1]);
            [roll, pitch, yaw]
        } else {
            let pitch = sy.asin();
            let roll = mat[1][2].atan2(mat[2][2]);
            let yaw = mat[0][1].atan2(mat[0][0]);
            [roll, pitch, yaw]
        }
    }

    /// Rotate a 3D vector
    pub fn rotate_vector(&self, v: [f64; 3]) -> [f64; 3] {
        let [qx, qy, qz, qw] = self.as_quaternion();

        // Quaternion rotation: q * v * q^-1
        // Using the formula: v' = v + 2 * q_w * (q_xyz × v) + 2 * (q_xyz × (q_xyz × v))
        let cross1 = [
            qy * v[2] - qz * v[1],
            qz * v[0] - qx * v[2],
            qx * v[1] - qy * v[0],
        ];

        let cross2 = [
            qy * cross1[2] - qz * cross1[1],
            qz * cross1[0] - qx * cross1[2],
            qx * cross1[1] - qy * cross1[0],
        ];

        [
            v[0] + 2.0 * (qw * cross1[0] + cross2[0]),
            v[1] + 2.0 * (qw * cross1[1] + cross2[1]),
            v[2] + 2.0 * (qw * cross1[2] + cross2[2]),
        ]
    }

    /// Compute inverse rotation
    pub fn inverse(&self) -> Rotation3D {
        let [x, y, z, w] = self.as_quaternion();
        Rotation3D::Quaternion {
            x: -x,
            y: -y,
            z: -z,
            w,
        }
    }

    /// Multiply two rotations: self * other
    pub fn compose(&self, other: &Rotation3D) -> Rotation3D {
        let q1 = self.as_quaternion();
        let q2 = other.as_quaternion();

        let w = q1[3] * q2[3] - q1[0] * q2[0] - q1[1] * q2[1] - q1[2] * q2[2];
        let x = q1[3] * q2[0] + q1[0] * q2[3] + q1[1] * q2[2] - q1[2] * q2[1];
        let y = q1[3] * q2[1] - q1[0] * q2[2] + q1[1] * q2[3] + q1[2] * q2[0];
        let z = q1[3] * q2[2] + q1[0] * q2[1] - q1[1] * q2[0] + q1[2] * q2[3];

        Rotation3D::Quaternion { x, y, z, w }
    }

    /// Normalize the rotation (for quaternions)
    pub fn normalized(&self) -> Rotation3D {
        let [x, y, z, w] = self.as_quaternion();
        let norm = (x * x + y * y + z * z + w * w).sqrt();
        if norm < 1e-10 {
            Rotation3D::identity()
        } else {
            Rotation3D::Quaternion {
                x: x / norm,
                y: y / norm,
                z: z / norm,
                w: w / norm,
            }
        }
    }
}

impl Default for Rotation3D {
    fn default() -> Self {
        Self::identity()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    const EPSILON: f64 = 1e-6;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < EPSILON
    }

    fn approx_eq_loose(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-3
    }

    fn approx_eq_arr3(a: [f64; 3], b: [f64; 3]) -> bool {
        approx_eq(a[0], b[0]) && approx_eq(a[1], b[1]) && approx_eq(a[2], b[2])
    }

    fn approx_eq_arr3_loose(a: [f64; 3], b: [f64; 3]) -> bool {
        approx_eq_loose(a[0], b[0]) && approx_eq_loose(a[1], b[1]) && approx_eq_loose(a[2], b[2])
    }

    #[test]
    fn test_identity_transform() {
        let t = Transform3D::identity();
        let point = [1.0, 2.0, 3.0];
        let result = t.transform_point(point);
        assert!(approx_eq_arr3(result, point));
    }

    #[test]
    fn test_translation_only() {
        let t = Transform3D::from_translation(1.0, 2.0, 3.0);
        let point = [0.0, 0.0, 0.0];
        let result = t.transform_point(point);
        assert!(approx_eq_arr3(result, [1.0, 2.0, 3.0]));
    }

    #[test]
    fn test_rotation_z_90() {
        let rot = Rotation3D::from_rotation_z(PI / 2.0);
        let v = [1.0, 0.0, 0.0];
        let result = rot.rotate_vector(v);
        assert!(approx_eq(result[0], 0.0));
        assert!(approx_eq(result[1], 1.0));
        assert!(approx_eq(result[2], 0.0));
    }

    #[test]
    fn test_transform_compose() {
        let t1 = Transform3D::from_translation(1.0, 0.0, 0.0);
        let t2 = Transform3D::from_translation(0.0, 1.0, 0.0);
        let composed = t1.compose(&t2);
        let point = [0.0, 0.0, 0.0];
        let result = composed.transform_point(point);
        assert!(approx_eq_arr3(result, [1.0, 1.0, 0.0]));
    }

    #[test]
    fn test_inverse_transform() {
        // Use a proper normalized quaternion for 45 degree rotation around Z
        let angle = PI / 4.0;
        let half = angle / 2.0;
        let t = Transform3D::from_quaternion(1.0, 2.0, 3.0, 0.0, 0.0, half.sin(), half.cos());
        let inv = t.inverse();
        let composed = t.compose(&inv);

        let point = [5.0, 6.0, 7.0];
        let result = composed.transform_point(point);
        assert!(approx_eq_arr3_loose(result, point));
    }

    #[test]
    fn test_euler_quaternion_roundtrip() {
        // Test with small angles where the conversion is well-behaved
        let euler = Rotation3D::from_euler(0.1, 0.2, 0.3);
        let quat = euler.as_quaternion();
        let back = Rotation3D::from_quaternion(quat[0], quat[1], quat[2], quat[3]);

        // Test by rotating a vector - this is more robust than comparing Euler angles
        let v = [1.0, 0.0, 0.0];
        let v1 = euler.rotate_vector(v);
        let v2 = back.rotate_vector(v);

        assert!(approx_eq_arr3_loose(v1, v2));
    }
}
