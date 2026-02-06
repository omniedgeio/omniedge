//! SSH Session management

use crate::types::*;
use std::collections::HashMap;
use std::path::PathBuf;

/// Arguments for spawning a user session
#[derive(Debug, Clone)]
pub struct SessionArgs {
    /// Target user ID
    pub uid: u32,
    /// Target group ID
    pub gid: u32,
    /// Supplementary groups
    pub groups: Vec<u32>,
    /// Home directory
    pub home_dir: PathBuf,
    /// Login shell
    pub shell: PathBuf,
    /// Command to execute
    pub command: SessionCommand,
    /// Environment variables
    pub env: HashMap<String, String>,
    /// PTY request (if any)
    pub pty: Option<PtyRequest>,
}

/// PTY request info
#[derive(Debug, Clone)]
pub struct PtyRequest {
    /// Terminal type (e.g., "xterm-256color")
    pub term: String,
    /// Terminal width in characters
    pub width: u32,
    /// Terminal height in characters
    pub height: u32,
    /// Terminal width in pixels
    pub width_px: u32,
    /// Terminal height in pixels
    pub height_px: u32,
}

/// Active session state
pub struct Session {
    /// Session unique identifier
    pub id: String,
    /// Connection ID this session belongs to
    pub connection_id: String,
    /// Session type
    pub session_type: SessionCommand,
    /// Applied permissions
    pub action: SshAction,
    /// Start time
    pub started_at: chrono::DateTime<chrono::Utc>,
    /// PTY info (if allocated)
    pub pty: Option<PtyRequest>,
}

impl Session {
    /// Create a new session
    pub fn new(
        id: String,
        connection_id: String,
        session_type: SessionCommand,
        action: SshAction,
    ) -> Self {
        Self {
            id,
            connection_id,
            session_type,
            action,
            started_at: chrono::Utc::now(),
            pty: None,
        }
    }

    /// Set PTY info
    pub fn with_pty(mut self, pty: PtyRequest) -> Self {
        self.pty = Some(pty);
        self
    }

    /// Get session duration
    pub fn duration(&self) -> chrono::Duration {
        chrono::Utc::now() - self.started_at
    }

    /// Check if session has exceeded max duration
    pub fn is_expired(&self) -> bool {
        if let Some(max_duration) = self.action.session_duration {
            let elapsed = self.duration();
            return elapsed.to_std().unwrap_or_default() > max_duration;
        }
        false
    }
}

impl Default for PtyRequest {
    fn default() -> Self {
        Self {
            term: "xterm-256color".to_string(),
            width: 80,
            height: 24,
            width_px: 0,
            height_px: 0,
        }
    }
}
