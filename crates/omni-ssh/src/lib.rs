//! # OmniEdge SSH Integration
//!
//! This crate provides SSH functionality for OmniEdge VPN, enabling secure,
//! zero-config SSH between VPN peers using OmniEdge's identity system.
//!
//! ## Features
//!
//! - **SSH Server**: Allow peers to SSH into nodes via VPN tunnel
//! - **SSH Client**: Built-in SSH client to connect to peers
//! - **SFTP Support**: Secure file transfer between peers
//! - **Port Forwarding**: Local and remote port forwarding
//! - **Session Recording**: Record sessions for audit/compliance
//! - **Emergency Access**: Break-glass mechanism for safety-critical situations
//! - **Fleet Operations**: Batch command execution across multiple devices
//! - **Command Filtering**: Allowlist/blocklist for command-level security
//! - **Connection Health**: Real-time monitoring and auto-disconnect
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use omni_ssh::{SshServer, SshServerConfig};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Create SSH server with default config
//!     let config = SshServerConfig::default();
//!     
//!     // Server will use OmniEdge identity for authentication
//!     // let server = SshServer::new(config, backend).await?;
//!     // server.start("0.0.0.0:22".parse()?).await?;
//!     
//!     Ok(())
//! }
//! ```

#![warn(missing_docs)]
#![warn(rust_2018_idioms)]
#![deny(unsafe_op_in_unsafe_fn)]

pub mod types;

#[cfg(feature = "server")]
pub mod server;

#[cfg(feature = "client")]
pub mod client;

#[cfg(feature = "sftp")]
pub mod sftp;

pub mod forwarding;
pub mod policy;

#[cfg(feature = "recording")]
pub mod recording;

#[cfg(feature = "emergency")]
pub mod emergency;

#[cfg(feature = "fleet")]
pub mod fleet;

pub mod health;

// Re-export main types
pub use types::*;

#[cfg(feature = "server")]
pub use server::{SshBackend, SshEvent, SshServer, SshServerConfig};

#[cfg(feature = "client")]
pub use client::{SshClient, SshSession, SshTarget};

pub use health::{ConnectionHealth, HealthMonitor, HealthThreshold};
pub use policy::{PolicyCache, PolicyManager, PolicyValidity};

#[cfg(feature = "fleet")]
pub use fleet::{FleetExecutor, FleetOperation, FleetResults, FleetTargets, NodeResult};

#[cfg(feature = "emergency")]
pub use emergency::{
    EmergencyAccessConfig, EmergencyAccessGrant, EmergencyAccessManager, EmergencyAccessRequest,
    EmergencyAccessStatus, EmergencySeverity,
};

// Re-export dependencies for external use
pub use async_trait::async_trait;
pub use russh_keys;

/// Crate version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
