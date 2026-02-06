//! SSH Server implementation
//!
//! This module provides the SSH server that accepts connections from OmniEdge peers.

mod auth;
mod command_filter;
mod config;
mod handler;
mod incubator;
mod pty;
mod rate_limit;
mod session;

pub use auth::OmniEdgeAuthenticator;
pub use command_filter::{CommandFilter, CommandFilterResult};
pub use config::SshServerConfig;
pub use handler::{OmniEdgeSshServer, SshConnectionHandler};
pub use pty::{AsyncPtySession, PtyConfig, PtySession};
pub use rate_limit::ConnectionRateLimiter;

use crate::types::*;
use async_trait::async_trait;
use dashmap::DashMap;
use russh::server::Config as RusshConfig;
use russh_keys::key::KeyPair;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tracing::{debug, error, info};

/// Main SSH server
pub struct SshServer {
    config: SshServerConfig,
    backend: Arc<dyn SshBackend>,
    active_conns: DashMap<String, Arc<SshConnection>>,
    host_keys: Vec<KeyPair>,
    rate_limiter: ConnectionRateLimiter,
}

/// Interface to OmniEdge core
#[async_trait]
pub trait SshBackend: Send + Sync {
    /// Get host keys for this node
    async fn get_host_keys(&self) -> anyhow::Result<Vec<KeyPair>>;

    /// Check if SSH server should run
    fn ssh_enabled(&self) -> bool;

    /// Look up peer identity by VPN address
    async fn who_is(&self, addr: IpAddr) -> anyhow::Result<Option<PeerIdentity>>;

    /// Get current SSH policy
    async fn get_ssh_policy(&self) -> anyhow::Result<SshPolicy>;

    /// Notify of SSH events (for recording, logging)
    async fn on_ssh_event(&self, event: SshEvent);

    /// Check if address is a valid OmniEdge VPN address
    fn is_omniedge_ip(&self, addr: IpAddr) -> bool;

    /// Get device ID for this node
    fn device_id(&self) -> &str;

    /// Get network ID for this node
    fn network_id(&self) -> &str;
}

/// Peer identity from OmniEdge
#[derive(Debug, Clone)]
pub struct PeerIdentity {
    /// Node information
    pub node: NodeInfo,
    /// User information
    pub user: UserProfile,
}

/// SSH events for logging/metrics
#[derive(Debug, Clone)]
pub enum SshEvent {
    /// Connection attempt started
    ConnectionAttempt {
        /// Source address
        src: SocketAddr,
        /// Destination address
        dst: SocketAddr,
    },
    /// Connection authenticated successfully
    ConnectionAuthenticated {
        /// Connection ID
        conn_id: String,
        /// SSH username
        ssh_user: String,
        /// Local username
        local_user: String,
    },
    /// Connection rejected
    ConnectionRejected {
        /// Source address
        src: SocketAddr,
        /// Rejection reason
        reason: String,
    },
    /// Connection closed
    ConnectionClosed {
        /// Connection ID
        conn_id: String,
    },
    /// Session started
    SessionStarted {
        /// Connection ID
        conn_id: String,
        /// Session type (shell, exec, sftp)
        session_type: String,
    },
    /// Session ended
    SessionEnded {
        /// Connection ID
        conn_id: String,
    },
    /// Recording started
    RecordingStarted {
        /// Connection ID
        conn_id: String,
    },
    /// Recording failed
    RecordingFailed {
        /// Connection ID
        conn_id: String,
        /// Error message
        error: String,
    },
    /// Command blocked by filter
    CommandBlocked {
        /// Connection ID
        conn_id: String,
        /// Attempted command
        command: String,
        /// Reason for blocking
        reason: String,
    },
    /// Rate limit exceeded
    RateLimitExceeded {
        /// Source IP
        src_ip: IpAddr,
        /// Current count
        count: u32,
    },
}

/// Active SSH connection
pub struct SshConnection {
    /// Connection ID
    pub id: String,
    /// Connection info
    pub info: SshConnInfo,
    /// Applied action/permissions
    pub action: SshAction,
    /// Whether connection is still valid
    valid: std::sync::atomic::AtomicBool,
}

impl SshConnection {
    /// Create a new connection
    pub fn new(id: String, info: SshConnInfo, action: SshAction) -> Self {
        Self {
            id,
            info,
            action,
            valid: std::sync::atomic::AtomicBool::new(true),
        }
    }

    /// Check if connection is still valid
    pub fn is_valid(&self) -> bool {
        self.valid.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Check if connection should be terminated (called on policy change)
    pub async fn check_still_valid(&self) {
        // Re-evaluate policy for this connection
        // Implementation would check current policy against connection info
    }

    /// Terminate the connection
    pub async fn terminate(&self, reason: &str) {
        info!(
            conn_id = %self.id,
            reason = %reason,
            "Terminating SSH connection"
        );
        self.valid
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }
}

impl SshServer {
    /// Create a new SSH server
    pub async fn new(
        config: SshServerConfig,
        backend: Arc<dyn SshBackend>,
    ) -> anyhow::Result<Self> {
        let host_keys = backend.get_host_keys().await?;

        if host_keys.is_empty() {
            return Err(anyhow::anyhow!("No host keys configured"));
        }

        let rate_limiter = ConnectionRateLimiter::new(
            config.rate_limit_per_ip,
            config.max_failed_auth,
            config.ban_duration,
            config.max_concurrent,
        );

        Ok(Self {
            config,
            backend,
            active_conns: DashMap::new(),
            host_keys,
            rate_limiter,
        })
    }

    /// Start the SSH server
    pub async fn start(&self, addr: SocketAddr) -> anyhow::Result<()> {
        if !self.backend.ssh_enabled() {
            return Err(anyhow::anyhow!("SSH is disabled"));
        }

        let listener = TcpListener::bind(addr).await?;
        info!("SSH server listening on {}", addr);

        loop {
            match listener.accept().await {
                Ok((stream, peer_addr)) => {
                    let server = self.clone_for_spawn();

                    tokio::spawn(async move {
                        if let Err(e) = server.handle_connection(stream, peer_addr).await {
                            error!("SSH connection error from {}: {}", peer_addr, e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                }
            }
        }
    }

    /// Handle a single SSH connection
    async fn handle_connection(
        &self,
        stream: tokio::net::TcpStream,
        peer_addr: SocketAddr,
    ) -> anyhow::Result<()> {
        let local_addr = stream.local_addr()?;

        // Emit connection attempt event
        self.backend
            .on_ssh_event(SshEvent::ConnectionAttempt {
                src: peer_addr,
                dst: local_addr,
            })
            .await;

        // Check rate limit
        match self.rate_limiter.check_allowed(peer_addr.ip()) {
            rate_limit::RateLimitResult::Allowed => {}
            rate_limit::RateLimitResult::RateLimited { retry_after } => {
                self.backend
                    .on_ssh_event(SshEvent::ConnectionRejected {
                        src: peer_addr,
                        reason: format!("Rate limited, retry after {:?}", retry_after),
                    })
                    .await;
                return Err(anyhow::anyhow!(
                    "Rate limited for {}, retry after {:?}",
                    peer_addr.ip(),
                    retry_after
                ));
            }
            rate_limit::RateLimitResult::Banned { remaining } => {
                self.backend
                    .on_ssh_event(SshEvent::ConnectionRejected {
                        src: peer_addr,
                        reason: format!("IP banned, {} seconds remaining", remaining.as_secs()),
                    })
                    .await;
                return Err(anyhow::anyhow!(
                    "IP {} banned for {} more seconds",
                    peer_addr.ip(),
                    remaining.as_secs()
                ));
            }
            rate_limit::RateLimitResult::TooManyConnections => {
                self.backend
                    .on_ssh_event(SshEvent::ConnectionRejected {
                        src: peer_addr,
                        reason: "Too many concurrent connections".to_string(),
                    })
                    .await;
                return Err(anyhow::anyhow!("Too many concurrent connections"));
            }
        }

        // Verify connection is from VPN
        if !self.backend.is_omniedge_ip(peer_addr.ip()) {
            self.backend
                .on_ssh_event(SshEvent::ConnectionRejected {
                    src: peer_addr,
                    reason: "Connection not from OmniEdge network".to_string(),
                })
                .await;
            return Err(anyhow::anyhow!("Connection not from OmniEdge network"));
        }

        // Check max concurrent connections
        if self.active_conns.len() >= self.config.max_concurrent as usize {
            self.backend
                .on_ssh_event(SshEvent::ConnectionRejected {
                    src: peer_addr,
                    reason: "Max concurrent connections exceeded".to_string(),
                })
                .await;
            return Err(anyhow::anyhow!("Max concurrent connections exceeded"));
        }

        debug!("Accepted SSH connection from {}", peer_addr);

        // Create russh server config
        let mut russh_config = RusshConfig::default();
        russh_config.auth_rejection_time = Duration::from_secs(3);
        russh_config.auth_rejection_time_initial = Some(Duration::from_secs(0));

        // Add host keys
        for key in &self.host_keys {
            russh_config.keys.push(key.clone());
        }

        let russh_config = Arc::new(russh_config);

        // Create handler for this connection
        let ssh_server = OmniEdgeSshServer::new(self.backend.clone(), self.config.clone());
        let handler = ssh_server.make_handler(peer_addr);

        // Run the SSH protocol
        let _session = russh::server::run_stream(russh_config, stream, handler).await?;

        // Session is now running - it will handle its own lifecycle
        // The handler will emit events and manage the connection

        Ok(())
    }

    /// Called when SSH policy changes - re-evaluate active sessions
    pub async fn on_policy_change(&self) {
        info!(
            "SSH policy changed, re-evaluating {} active connections",
            self.active_conns.len()
        );

        for entry in self.active_conns.iter() {
            let conn = entry.value().clone();
            tokio::spawn(async move {
                conn.check_still_valid().await;
            });
        }
    }

    /// Shutdown all active connections
    pub async fn shutdown(&self) {
        info!(
            "Shutting down SSH server, terminating {} connections",
            self.active_conns.len()
        );

        for entry in self.active_conns.iter() {
            entry.value().terminate("Server shutting down").await;
        }
        self.active_conns.clear();
    }

    /// Number of active connections
    pub fn num_active_conns(&self) -> usize {
        self.active_conns.len()
    }

    /// Clone for spawning into new task
    fn clone_for_spawn(&self) -> SshServerRef {
        SshServerRef {
            config: self.config.clone(),
            backend: self.backend.clone(),
            active_conns: self.active_conns.clone(),
            host_keys: self.host_keys.clone(),
            rate_limiter: self.rate_limiter.clone(),
        }
    }
}

/// Reference to SSH server for spawned tasks
struct SshServerRef {
    config: SshServerConfig,
    backend: Arc<dyn SshBackend>,
    active_conns: DashMap<String, Arc<SshConnection>>,
    host_keys: Vec<KeyPair>,
    rate_limiter: ConnectionRateLimiter,
}

impl SshServerRef {
    async fn handle_connection(
        &self,
        stream: tokio::net::TcpStream,
        peer_addr: SocketAddr,
    ) -> anyhow::Result<()> {
        let local_addr = stream.local_addr()?;

        // Emit connection attempt event
        self.backend
            .on_ssh_event(SshEvent::ConnectionAttempt {
                src: peer_addr,
                dst: local_addr,
            })
            .await;

        // Check rate limit
        match self.rate_limiter.check_allowed(peer_addr.ip()) {
            rate_limit::RateLimitResult::Allowed => {}
            rate_limit::RateLimitResult::RateLimited { retry_after } => {
                self.backend
                    .on_ssh_event(SshEvent::ConnectionRejected {
                        src: peer_addr,
                        reason: format!("Rate limited, retry after {:?}", retry_after),
                    })
                    .await;
                return Err(anyhow::anyhow!(
                    "Rate limited for {}, retry after {:?}",
                    peer_addr.ip(),
                    retry_after
                ));
            }
            rate_limit::RateLimitResult::Banned { remaining } => {
                self.backend
                    .on_ssh_event(SshEvent::ConnectionRejected {
                        src: peer_addr,
                        reason: format!("IP banned, {} seconds remaining", remaining.as_secs()),
                    })
                    .await;
                return Err(anyhow::anyhow!(
                    "IP {} banned for {} more seconds",
                    peer_addr.ip(),
                    remaining.as_secs()
                ));
            }
            rate_limit::RateLimitResult::TooManyConnections => {
                self.backend
                    .on_ssh_event(SshEvent::ConnectionRejected {
                        src: peer_addr,
                        reason: "Too many concurrent connections".to_string(),
                    })
                    .await;
                return Err(anyhow::anyhow!("Too many concurrent connections"));
            }
        }

        // Verify connection is from VPN
        if !self.backend.is_omniedge_ip(peer_addr.ip()) {
            self.backend
                .on_ssh_event(SshEvent::ConnectionRejected {
                    src: peer_addr,
                    reason: "Connection not from OmniEdge network".to_string(),
                })
                .await;
            return Err(anyhow::anyhow!("Connection not from OmniEdge network"));
        }

        debug!("Accepted SSH connection from {}", peer_addr);

        // Create russh server config
        let mut russh_config = RusshConfig::default();
        russh_config.auth_rejection_time = Duration::from_secs(3);
        russh_config.auth_rejection_time_initial = Some(Duration::from_secs(0));

        // Add host keys
        for key in &self.host_keys {
            russh_config.keys.push(key.clone());
        }

        let russh_config = Arc::new(russh_config);

        // Create handler for this connection
        let ssh_server = OmniEdgeSshServer::new(self.backend.clone(), self.config.clone());
        let handler = ssh_server.make_handler(peer_addr);

        // Run the SSH protocol
        let _session = russh::server::run_stream(russh_config, stream, handler).await?;

        // Session is now running - it will handle its own lifecycle
        // The handler will emit events and manage the connection

        Ok(())
    }
}
