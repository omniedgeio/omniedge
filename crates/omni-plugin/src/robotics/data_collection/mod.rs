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

// Phase 0-3: Core modules
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

// Phase 4: Episode packaging
mod mcap_writer;
mod packager;
mod privacy;

// Phase 5: Storage and upload
mod storage;
mod upload;

// Phase 6: Plugin integration
mod api;
mod plugin;

// Re-export public types from Phase 0-3
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

// Re-export public types from Phase 4
pub use mcap_writer::*;
pub use packager::*;
pub use privacy::*;

// Re-export public types from Phase 5
pub use storage::*;
pub use upload::*;

// Re-export public types from Phase 6
pub use api::*;
pub use plugin::*;
