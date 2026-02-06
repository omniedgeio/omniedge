//! Core types for SSH integration
//!
//! This module contains all the shared types used across the SSH implementation.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;

/// SSH connection info (for OmniEdge peer connections)
#[derive(Debug, Clone)]
pub struct SshConnInfo {
    /// Connection unique identifier
    pub connection_id: String,
    /// Requested SSH username
    pub ssh_user: String,
    /// Source VPN IP:port
    pub src_addr: SocketAddr,
    /// Destination VPN IP:port
    pub dst_addr: SocketAddr,
    /// Source peer from OmniEdge
    pub peer_node: NodeInfo,
    /// OmniEdge user identity
    pub user_profile: UserProfile,
}

/// Node information from OmniEdge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    /// Unique node identifier
    pub id: String,
    /// Human-readable node name
    pub name: String,
    /// VPN IP address
    pub virtual_ip: String,
    /// Node tags for policy matching
    pub tags: Vec<String>,
    /// Whether node is currently online
    pub online: bool,
    /// Network ID this node belongs to
    pub network_id: String,
}

/// User profile from OmniEdge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    /// Unique user identifier
    pub id: String,
    /// User email address
    pub email: String,
    /// User display name
    pub name: Option<String>,
}

/// SSH policy from cloud or local config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshPolicy {
    /// Policy version for cache invalidation
    pub version: u64,
    /// When policy was last updated
    pub updated_at: DateTime<Utc>,
    /// Ordered list of rules (evaluated top to bottom)
    pub rules: Vec<SshRule>,
}

/// A single SSH access rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshRule {
    /// Unique rule identifier
    pub id: String,
    /// Who can connect (match any principal)
    pub principals: Vec<SshPrincipal>,
    /// SSH user → local user mapping
    pub ssh_users: HashMap<String, String>,
    /// What action to take when rule matches
    pub action: SshAction,
    /// Allowed environment variables (glob patterns)
    pub accept_env: Vec<String>,
    /// When this rule expires (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires: Option<DateTime<Utc>>,
}

/// Criteria for matching incoming connections
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SshPrincipal {
    /// Specific node ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    /// Specific VPN IP
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_ip: Option<String>,
    /// OmniEdge user email (supports wildcards)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_email: Option<String>,
    /// Any node in this network
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_id: Option<String>,
    /// Node tag (e.g., "tag:servers")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    /// Match any connection
    #[serde(default)]
    pub any: bool,
}

/// Action to take when a rule matches
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshAction {
    /// Allow the connection
    #[serde(default)]
    pub accept: bool,
    /// Deny the connection
    #[serde(default)]
    pub reject: bool,
    /// Message to show the user (for reject or info)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    // Session capabilities
    /// Allow SSH agent forwarding
    #[serde(default)]
    pub allow_agent_forwarding: bool,
    /// Allow local port forwarding (-L)
    #[serde(default)]
    pub allow_local_port_forwarding: bool,
    /// Allow remote port forwarding (-R)
    #[serde(default)]
    pub allow_remote_port_forwarding: bool,
    /// Allow SFTP subsystem
    #[serde(default = "default_true")]
    pub allow_sftp: bool,

    // Session limits
    /// Maximum session duration
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(with = "option_duration_secs")]
    pub session_duration: Option<Duration>,

    // Recording
    /// Whether to record the session
    #[serde(default)]
    pub record_session: bool,
    /// Recording server endpoints
    #[serde(default)]
    pub recorders: Vec<String>,
    /// What to do if recording fails
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_recording_failure: Option<RecordingFailureAction>,

    // Interactive authorization
    /// URL for interactive authorization (HoldAndDelegate)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hold_and_delegate: Option<String>,

    // Command Filtering (GAP-4 from cross-review)
    /// Allowed commands (regex patterns) - if set, only these commands allowed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_commands: Option<Vec<String>>,
    /// Blocked commands (regex patterns) - checked first, always denied
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_commands: Option<Vec<String>>,
    /// Allowed working directories
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_paths: Option<Vec<String>>,
    /// Read-only mode (block write operations via SFTP)
    #[serde(default)]
    pub read_only: bool,

    // Time restrictions (MGT-2 from cross-review)
    /// Time-based access restrictions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_restrictions: Option<TimeRestrictions>,
}

/// Time-based access restrictions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRestrictions {
    /// Allowed hours (24h format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_hours: Option<TimeRange>,
    /// Allowed days of week
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_days: Option<Vec<String>>,
    /// Timezone for time evaluation
    #[serde(default = "default_timezone")]
    pub timezone: String,
    /// Allow override with interactive approval
    #[serde(default)]
    pub override_with_approval: bool,
}

/// Time range for restrictions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    /// Start time (HH:MM format)
    pub start: String,
    /// End time (HH:MM format)
    pub end: String,
}

/// Action when session recording fails
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingFailureAction {
    /// Reject the session with this message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reject_session_with_message: Option<String>,
    /// Terminate an active session with this message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminate_session_with_message: Option<String>,
}

/// Result of authentication attempt
#[derive(Debug, Clone)]
pub enum AuthResult {
    /// Connection accepted
    Accept {
        /// Local system user to use
        local_user: String,
        /// Action containing session permissions
        action: SshAction,
    },
    /// Connection rejected
    Reject {
        /// Reason for rejection
        message: String,
    },
    /// Need interactive authorization
    HoldAndDelegate {
        /// URL to poll for authorization decision
        url: String,
    },
}

/// SSH session command type
#[derive(Debug, Clone)]
pub enum SessionCommand {
    /// Interactive shell session
    Shell,
    /// Execute a specific command
    Exec(String),
    /// SFTP subsystem
    Sftp,
}

/// Recording configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingConfig {
    /// Enable session recording
    #[serde(default)]
    pub enabled: bool,
    /// Local recording directory (fallback)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_dir: Option<String>,
    /// Cloud recording upload URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cloud_url: Option<String>,
    /// Chunk size in bytes for upload
    #[serde(default = "default_chunk_size")]
    pub chunk_size: usize,
}

// Helper functions for serde defaults
fn default_true() -> bool {
    true
}

fn default_timezone() -> String {
    "UTC".to_string()
}

fn default_chunk_size() -> usize {
    65536 // 64KB
}

/// Custom serialization for Option<Duration> as seconds
mod option_duration_secs {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match duration {
            Some(d) => d.as_secs().serialize(serializer),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt: Option<u64> = Option::deserialize(deserializer)?;
        Ok(opt.map(Duration::from_secs))
    }
}

impl Default for SshAction {
    fn default() -> Self {
        Self {
            accept: false,
            reject: false,
            message: None,
            allow_agent_forwarding: false,
            allow_local_port_forwarding: false,
            allow_remote_port_forwarding: false,
            allow_sftp: true,
            session_duration: None,
            record_session: false,
            recorders: vec![],
            on_recording_failure: None,
            hold_and_delegate: None,
            // Default command filtering - block dangerous commands
            allowed_commands: None,
            blocked_commands: Some(vec![
                r"^rm\s+-rf".to_string(),
                r"^rm\s+.*-rf".to_string(),
                r"^shutdown".to_string(),
                r"^reboot".to_string(),
                r"^halt".to_string(),
                r"^poweroff".to_string(),
                r"^dd\s+if=".to_string(),
                r"^mkfs".to_string(),
                r"^format".to_string(),
                r"^fdisk".to_string(),
                r"^parted".to_string(),
            ]),
            allowed_paths: None,
            read_only: false,
            time_restrictions: None,
        }
    }
}

impl Default for RecordingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            local_dir: None,
            cloud_url: None,
            chunk_size: default_chunk_size(),
        }
    }
}

impl SshPolicy {
    /// Create a new empty policy
    pub fn new() -> Self {
        Self {
            version: 0,
            updated_at: Utc::now(),
            rules: vec![],
        }
    }

    /// Create a default policy that denies all
    pub fn deny_all() -> Self {
        Self {
            version: 0,
            updated_at: Utc::now(),
            rules: vec![SshRule {
                id: "default-deny".to_string(),
                principals: vec![SshPrincipal {
                    any: true,
                    ..Default::default()
                }],
                ssh_users: HashMap::new(),
                action: SshAction {
                    reject: true,
                    message: Some("SSH access denied by default policy".to_string()),
                    ..Default::default()
                },
                accept_env: vec![],
                expires: None,
            }],
        }
    }
}

impl Default for SshPolicy {
    fn default() -> Self {
        Self::deny_all()
    }
}
