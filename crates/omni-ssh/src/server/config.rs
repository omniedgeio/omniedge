//! SSH Server configuration

use crate::types::RecordingConfig;
use std::time::Duration;

/// Configuration for the SSH server
#[derive(Debug, Clone)]
pub struct SshServerConfig {
    /// Port to listen on (default 22)
    pub port: u16,
    /// Enable SFTP subsystem
    pub enable_sftp: bool,
    /// Enable port forwarding
    pub enable_forwarding: bool,
    /// Enable PTY allocation
    pub enable_pty: bool,
    /// Session recording config
    pub recording: RecordingConfig,
    /// Max connections per IP per minute
    pub rate_limit_per_ip: u32,
    /// Max failed auth attempts before ban
    pub max_failed_auth: u32,
    /// Ban duration after max failed auth
    pub ban_duration: Duration,
    /// Max concurrent connections
    pub max_concurrent: u32,
    /// Connection idle timeout
    pub idle_timeout: Duration,
    /// TCP keepalive interval
    pub keepalive_interval: Duration,
}

impl Default for SshServerConfig {
    fn default() -> Self {
        Self {
            port: 22,
            enable_sftp: true,
            enable_forwarding: false,
            enable_pty: true,
            recording: RecordingConfig::default(),
            rate_limit_per_ip: 10,
            max_failed_auth: 5,
            ban_duration: Duration::from_secs(15 * 60), // 15 minutes
            max_concurrent: 100,
            idle_timeout: Duration::from_secs(30 * 60), // 30 minutes
            keepalive_interval: Duration::from_secs(60),
        }
    }
}

impl SshServerConfig {
    /// Create a new config with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable recording with local directory
    pub fn with_local_recording(mut self, dir: String) -> Self {
        self.recording.enabled = true;
        self.recording.local_dir = Some(dir);
        self
    }

    /// Enable recording with cloud upload
    pub fn with_cloud_recording(mut self, url: String) -> Self {
        self.recording.enabled = true;
        self.recording.cloud_url = Some(url);
        self
    }

    /// Enable port forwarding
    pub fn with_forwarding(mut self, enable: bool) -> Self {
        self.enable_forwarding = enable;
        self
    }

    /// Set rate limiting
    pub fn with_rate_limit(mut self, per_ip: u32, max_failed: u32, ban_duration: Duration) -> Self {
        self.rate_limit_per_ip = per_ip;
        self.max_failed_auth = max_failed;
        self.ban_duration = ban_duration;
        self
    }
}
