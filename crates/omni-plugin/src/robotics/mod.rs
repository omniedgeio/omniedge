//! Robotics-specific plugin implementations
//!
//! This module contains specialized plugins for humanoid robot applications:
//!
//! - **data_collection**: AI training data pipeline with URDF-aware camera pose tracking
//!
//! # Feature Flags
//!
//! - `robotics`: Enables all robotics plugins (requires Linux for camera access)

pub mod data_collection;

pub use data_collection::*;
