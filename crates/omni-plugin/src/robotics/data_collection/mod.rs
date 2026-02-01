//! Robot Data Collection Plugin
//!
//! Enables AI training data pipeline for humanoid robots with:
//!
//! - High-bandwidth sensor data buffering with ring buffers
//! - Event-triggered episode capture (teleoperation, failures, novel situations)
//! - MCAP format packaging compatible with Foxglove Studio
//! - URDF-aware camera pose tracking for imitation learning
//! - Privacy-aware data processing (face blurring, PII removal)
//! - Cloud upload with resumable transfers
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────┐  ┌─────────────┐  ┌─────────────┐
//! │ RGB Cameras │  │ Depth Cams  │  │ Joint State │
//! └──────┬──────┘  └──────┬──────┘  └──────┬──────┘
//!        └─────────────────┴─────────────────┘
//!                          │
//!               ┌──────────┴──────────┐
//!               │   Ring Buffer Pool  │
//!               │   (per-stream)      │
//!               └──────────┬──────────┘
//!                          │
//!               ┌──────────┴──────────┐
//!               │   Frame Enricher    │
//!               │   (adds FK poses)   │
//!               └──────────┬──────────┘
//!                          │
//!         ┌────────────────┼────────────────┐
//!         │                │                │
//!    ┌────┴────┐     ┌─────┴─────┐    ┌─────┴─────┐
//!    │ Teleop  │     │  Failure  │    │  Manual   │
//!    │ Trigger │     │  Trigger  │    │  Trigger  │
//!    └────┬────┘     └─────┬─────┘    └─────┬─────┘
//!         └────────────────┼────────────────┘
//!                          │
//!               ┌──────────┴──────────┐
//!               │  Episode Packager   │
//!               │  (MCAP + privacy)   │
//!               └──────────┬──────────┘
//!                          │
//!               ┌──────────┴──────────┐
//!               │   Upload Manager    │
//!               │   (S3/GCS)          │
//!               └─────────────────────┘
//! ```

mod buffer;
mod camera_config;
mod compression;
mod enrichment;
mod metadata;
mod streams;
mod transform;
mod trigger_impl;
mod triggers;
mod types;
mod urdf;

// Re-export public types
pub use buffer::*;
pub use camera_config::*;
pub use compression::*;
pub use enrichment::*;
pub use metadata::*;
pub use streams::*;
pub use transform::*;
pub use trigger_impl::*;
pub use triggers::*;
pub use types::*;
pub use urdf::*;

// Conditional modules (require additional dependencies)
// These will be implemented in future phases
// #[cfg(feature = "robotics-full")]
// mod compression;
// #[cfg(feature = "robotics-full")]
// mod mcap_writer;
// #[cfg(feature = "robotics-full")]
// mod packager;
// #[cfg(feature = "robotics-full")]
// mod plugin;
// #[cfg(feature = "robotics-full")]
// mod privacy;
// #[cfg(feature = "robotics-full")]
// mod storage;
// #[cfg(feature = "robotics-full")]
// mod triggers;
// #[cfg(feature = "robotics-full")]
// mod upload;
