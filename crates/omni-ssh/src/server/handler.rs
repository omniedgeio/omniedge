//! SSH protocol handler implementing russh::server::Handler
//!
//! This module implements the SSH protocol handling for OmniEdge SSH server,
//! integrating with OmniEdge identity-based authentication.

#[cfg(feature = "recording")]
use crate::recording::SessionRecorder;
use crate::server::{
    auth::OmniEdgeAuthenticator,
    command_filter::CommandFilter,
    config::SshServerConfig,
    pty::{AsyncPtySession, PtyConfig},
    PeerIdentity, SshBackend, SshConnection, SshEvent,
};
use crate::types::*;
use async_trait::async_trait;
use dashmap::DashMap;
use russh::server::{Auth, Handle, Msg, Session};
use russh::{Channel, ChannelId, CryptoVec};
use russh_keys::key::PublicKey;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

/// Type of channel subsystem
#[derive(Debug, Clone)]
pub enum ChannelSubsystem {
    /// No subsystem (shell/exec)
    None,
    /// SFTP subsystem
    #[cfg(feature = "sftp")]
    Sftp,
}

/// Represents an active SSH channel with its state
pub struct ChannelState {
    /// The raw channel (kept for SFTP subsystem use)
    pub channel: Option<Channel<Msg>>,
    /// PTY session if allocated
    pub pty: Option<Arc<Mutex<AsyncPtySession>>>,
    /// PTY configuration
    pub pty_config: Option<PtyConfig>,
    /// Whether shell has been requested
    pub shell_requested: bool,
    /// Environment variables set for this channel
    pub env: HashMap<String, String>,
    /// Subsystem type
    pub subsystem: ChannelSubsystem,
    /// Session recorder for audit logging
    #[cfg(feature = "recording")]
    pub recorder: Option<Arc<Mutex<SessionRecorder>>>,
}

/// SSH connection handler - one instance per connection
pub struct SshConnectionHandler {
    /// OmniEdge backend for identity/policy lookups
    backend: Arc<dyn SshBackend>,
    /// Server configuration
    config: SshServerConfig,
    /// Peer socket address
    peer_addr: SocketAddr,
    /// Connection unique ID
    conn_id: String,
    /// Authenticated peer identity (set after auth)
    peer_identity: Option<PeerIdentity>,
    /// Resolved local user (set after auth)
    local_user: Option<String>,
    /// SSH username (set during auth)
    ssh_user: Option<String>,
    /// Applied SSH action/permissions (set after auth)
    action: Option<SshAction>,
    /// Authenticator
    authenticator: OmniEdgeAuthenticator,
    /// Active channels
    channels: DashMap<ChannelId, ChannelState>,
    /// Shared connections registry
    active_conns: DashMap<String, Arc<SshConnection>>,
    /// Whether auth has been completed
    auth_completed: bool,
}

impl SshConnectionHandler {
    /// Create a new connection handler
    pub fn new(
        backend: Arc<dyn SshBackend>,
        config: SshServerConfig,
        peer_addr: SocketAddr,
        active_conns: DashMap<String, Arc<SshConnection>>,
    ) -> Self {
        let conn_id = uuid::Uuid::new_v4().to_string();
        let authenticator = OmniEdgeAuthenticator::new(backend.clone());

        Self {
            backend,
            config,
            peer_addr,
            conn_id,
            peer_identity: None,
            local_user: None,
            ssh_user: None,
            action: None,
            authenticator,
            channels: DashMap::new(),
            active_conns,
            auth_completed: false,
        }
    }

    /// Get connection ID
    pub fn conn_id(&self) -> &str {
        &self.conn_id
    }

    /// Get the command filter for this session based on action
    fn create_command_filter(&self) -> Option<CommandFilter> {
        self.action
            .as_ref()
            .and_then(|action| CommandFilter::new(action).ok())
    }

    /// Spawn PTY I/O forwarding task with optional recording
    #[cfg(feature = "recording")]
    fn spawn_pty_io_task(
        &self,
        channel_id: ChannelId,
        pty: Arc<Mutex<AsyncPtySession>>,
        handle: Handle,
        recorder: Option<Arc<Mutex<SessionRecorder>>>,
    ) {
        let conn_id = self.conn_id.clone();
        let backend = self.backend.clone();

        tokio::spawn(async move {
            loop {
                // Get output from PTY
                let output = {
                    let mut pty_guard = pty.lock().await;
                    pty_guard.recv_output().await
                };

                match output {
                    Some(data) => {
                        // Record output if recorder is present
                        if let Some(ref rec) = recorder {
                            let mut rec_guard = rec.lock().await;
                            if let Err(e) = rec_guard.record_output(&data).await {
                                warn!("Failed to record output: {}", e);
                            }
                        }

                        if handle
                            .data(channel_id, CryptoVec::from_slice(&data))
                            .await
                            .is_err()
                        {
                            error!("Failed to send PTY output to channel");
                            break;
                        }
                    }
                    None => {
                        // PTY closed - check for exit status
                        debug!("PTY output closed for connection {}", conn_id);
                        let exit_code = {
                            let mut pty_guard = pty.lock().await;
                            pty_guard.try_recv_exit()
                        };
                        if let Some(code) = exit_code {
                            info!(
                                "PTY process exited with code {} for connection {}",
                                code, conn_id
                            );
                            let _ = handle.exit_status_request(channel_id, code as u32).await;
                        }
                        let _ = handle.eof(channel_id).await;
                        let _ = handle.close(channel_id).await;
                        break;
                    }
                }

                // Check for exit status periodically
                let exit_code = {
                    let mut pty_guard = pty.lock().await;
                    pty_guard.try_recv_exit()
                };
                if let Some(code) = exit_code {
                    info!(
                        "PTY process exited with code {} for connection {}",
                        code, conn_id
                    );
                    let _ = handle.exit_status_request(channel_id, code as u32).await;
                    let _ = handle.eof(channel_id).await;
                    let _ = handle.close(channel_id).await;
                    break;
                }
            }

            // Finalize recording if present
            if let Some(ref rec) = recorder {
                let mut rec_guard = rec.lock().await;
                if let Err(e) = rec_guard.finalize().await {
                    warn!("Failed to finalize recording: {}", e);
                }
            }

            // Emit session ended event
            backend
                .on_ssh_event(SshEvent::SessionEnded {
                    conn_id: conn_id.clone(),
                })
                .await;
        });
    }

    /// Spawn PTY I/O forwarding task (no recording)
    #[cfg(not(feature = "recording"))]
    fn spawn_pty_io_task(
        &self,
        channel_id: ChannelId,
        pty: Arc<Mutex<AsyncPtySession>>,
        handle: Handle,
    ) {
        let conn_id = self.conn_id.clone();
        let backend = self.backend.clone();

        tokio::spawn(async move {
            loop {
                // Get output from PTY
                let output = {
                    let mut pty_guard = pty.lock().await;
                    pty_guard.recv_output().await
                };

                match output {
                    Some(data) => {
                        if handle
                            .data(channel_id, CryptoVec::from_slice(&data))
                            .await
                            .is_err()
                        {
                            error!("Failed to send PTY output to channel");
                            break;
                        }
                    }
                    None => {
                        // PTY closed - check for exit status
                        debug!("PTY output closed for connection {}", conn_id);
                        let exit_code = {
                            let mut pty_guard = pty.lock().await;
                            pty_guard.try_recv_exit()
                        };
                        if let Some(code) = exit_code {
                            info!(
                                "PTY process exited with code {} for connection {}",
                                code, conn_id
                            );
                            let _ = handle.exit_status_request(channel_id, code as u32).await;
                        }
                        let _ = handle.eof(channel_id).await;
                        let _ = handle.close(channel_id).await;
                        break;
                    }
                }

                // Check for exit status periodically
                let exit_code = {
                    let mut pty_guard = pty.lock().await;
                    pty_guard.try_recv_exit()
                };
                if let Some(code) = exit_code {
                    info!(
                        "PTY process exited with code {} for connection {}",
                        code, conn_id
                    );
                    let _ = handle.exit_status_request(channel_id, code as u32).await;
                    let _ = handle.eof(channel_id).await;
                    let _ = handle.close(channel_id).await;
                    break;
                }
            }

            // Emit session ended event
            backend
                .on_ssh_event(SshEvent::SessionEnded {
                    conn_id: conn_id.clone(),
                })
                .await;
        });
    }

    /// Create a session recorder if recording is enabled
    #[cfg(feature = "recording")]
    async fn create_recorder(
        &self,
        channel_id: ChannelId,
        session_type: &str,
    ) -> Option<Arc<Mutex<SessionRecorder>>> {
        use crate::recording::{CloudRecordingWriter, FileRecordingWriter};

        // Check if recording is enabled in action
        let should_record = self
            .action
            .as_ref()
            .map(|a| a.record_session)
            .unwrap_or(false);

        if !should_record && !self.config.recording.enabled {
            return None;
        }

        // Get terminal dimensions from PTY config
        let (width, height) = if let Some(ch) = self.channels.get(&channel_id) {
            ch.pty_config
                .as_ref()
                .map(|c| (c.cols as u32, c.rows as u32))
                .unwrap_or((80, 24))
        } else {
            (80, 24)
        };

        // Create session ID
        let session_id = format!("{}_{}", self.conn_id, session_type);

        // Create recorder
        let mut recorder = SessionRecorder::new(session_id.clone(), width, height);

        // Add file writer if configured
        if let Some(ref dir) = self.config.recording.local_dir {
            let path = std::path::PathBuf::from(dir);
            let filename = format!("{}.cast", session_id);
            let file_path = path.join(filename);

            match FileRecordingWriter::new(file_path).await {
                Ok(writer) => {
                    recorder.add_writer(Box::new(writer));
                }
                Err(e) => {
                    warn!("Failed to create file recording writer: {}", e);
                }
            }
        }

        // Add cloud writer if configured
        if let Some(ref url) = self.config.recording.cloud_url {
            let writer = CloudRecordingWriter::new(url.clone(), session_id.clone());
            recorder.add_writer(Box::new(writer));
        }

        // Add recorders from action
        if let Some(ref action) = self.action {
            for url in &action.recorders {
                let writer = CloudRecordingWriter::new(url.clone(), session_id.clone());
                recorder.add_writer(Box::new(writer));
            }
        }

        // Write header
        let mut header = recorder.create_header();
        header.ssh_user = self.ssh_user.clone();
        header.local_user = self.local_user.clone();
        header.connection_id = Some(self.conn_id.clone());
        header.command = Some(session_type.to_string());

        let recorder = Arc::new(Mutex::new(recorder));

        // Write header in separate task to avoid blocking
        let rec_clone = recorder.clone();
        let header_clone = header;
        tokio::spawn(async move {
            let mut rec = rec_clone.lock().await;
            if let Err(e) = rec.write_header(header_clone).await {
                warn!("Failed to write recording header: {}", e);
            }
        });

        Some(recorder)
    }

    /// Start a shell session (with recording support)
    #[cfg(feature = "recording")]
    async fn start_shell(&self, channel_id: ChannelId, handle: Handle) -> anyhow::Result<()> {
        let local_user = self
            .local_user
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not authenticated"))?;

        // Get shell for user
        let shell = self.get_user_shell(local_user)?;

        // Get PTY config from channel state
        let pty_config = self
            .channels
            .get(&channel_id)
            .and_then(|ch| ch.pty_config.clone())
            .unwrap_or_default();

        // Get environment variables
        let env: Vec<(String, String)> = self
            .channels
            .get(&channel_id)
            .map(|ch| ch.env.clone().into_iter().collect())
            .unwrap_or_default();

        // Get home directory
        let home_dir = self.get_user_home(local_user);

        // Create async PTY session
        let pty = AsyncPtySession::new(&shell, pty_config, env, home_dir)?;
        let pty = Arc::new(Mutex::new(pty));

        // Create recorder if enabled
        let recorder = self.create_recorder(channel_id, "shell").await;

        // Store PTY and recorder in channel state
        if let Some(mut channel) = self.channels.get_mut(&channel_id) {
            channel.pty = Some(pty.clone());
            channel.recorder = recorder.clone();
        }

        // Spawn I/O forwarding task with recording
        self.spawn_pty_io_task(channel_id, pty, handle, recorder);

        // Emit session started event
        self.backend
            .on_ssh_event(SshEvent::SessionStarted {
                conn_id: self.conn_id.clone(),
                session_type: "shell".to_string(),
            })
            .await;

        Ok(())
    }

    /// Start a shell session (no recording)
    #[cfg(not(feature = "recording"))]
    async fn start_shell(&self, channel_id: ChannelId, handle: Handle) -> anyhow::Result<()> {
        let local_user = self
            .local_user
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not authenticated"))?;

        // Get shell for user
        let shell = self.get_user_shell(local_user)?;

        // Get PTY config from channel state
        let pty_config = self
            .channels
            .get(&channel_id)
            .and_then(|ch| ch.pty_config.clone())
            .unwrap_or_default();

        // Get environment variables
        let env: Vec<(String, String)> = self
            .channels
            .get(&channel_id)
            .map(|ch| ch.env.clone().into_iter().collect())
            .unwrap_or_default();

        // Get home directory
        let home_dir = self.get_user_home(local_user);

        // Create async PTY session
        let pty = AsyncPtySession::new(&shell, pty_config, env, home_dir)?;
        let pty = Arc::new(Mutex::new(pty));

        // Store PTY in channel state
        if let Some(mut channel) = self.channels.get_mut(&channel_id) {
            channel.pty = Some(pty.clone());
        }

        // Spawn I/O forwarding task
        self.spawn_pty_io_task(channel_id, pty, handle);

        // Emit session started event
        self.backend
            .on_ssh_event(SshEvent::SessionStarted {
                conn_id: self.conn_id.clone(),
                session_type: "shell".to_string(),
            })
            .await;

        Ok(())
    }

    /// Execute a command (with recording support)
    #[cfg(feature = "recording")]
    async fn exec_command(
        &self,
        channel_id: ChannelId,
        command: &str,
        handle: Handle,
    ) -> anyhow::Result<()> {
        // Check command filter
        if let Some(filter) = self.create_command_filter() {
            use crate::server::command_filter::CommandFilterResult;
            match filter.check_command(command) {
                CommandFilterResult::Allowed => {}
                CommandFilterResult::Blocked { reason } => {
                    warn!("Command blocked: {} - {}", command, reason);
                    self.backend
                        .on_ssh_event(SshEvent::CommandBlocked {
                            conn_id: self.conn_id.clone(),
                            command: command.to_string(),
                            reason: reason.clone(),
                        })
                        .await;

                    // Send error message to client
                    let msg = format!("Command blocked: {}\r\n", reason);
                    let _ = handle
                        .extended_data(channel_id, 1, CryptoVec::from_slice(msg.as_bytes()))
                        .await;
                    let _ = handle.exit_status_request(channel_id, 1).await;
                    let _ = handle.eof(channel_id).await;
                    let _ = handle.close(channel_id).await;
                    return Ok(());
                }
                CommandFilterResult::PathNotAllowed { path } => {
                    warn!("Path not allowed: {}", path);
                    let msg = format!("Path not allowed: {}\r\n", path);
                    let _ = handle
                        .extended_data(channel_id, 1, CryptoVec::from_slice(msg.as_bytes()))
                        .await;
                    let _ = handle.exit_status_request(channel_id, 1).await;
                    let _ = handle.eof(channel_id).await;
                    let _ = handle.close(channel_id).await;
                    return Ok(());
                }
                CommandFilterResult::ReadOnlyViolation => {
                    warn!("Read-only violation for command: {}", command);
                    let msg = "Read-only mode: write operations not allowed\r\n";
                    let _ = handle
                        .extended_data(channel_id, 1, CryptoVec::from_slice(msg.as_bytes()))
                        .await;
                    let _ = handle.exit_status_request(channel_id, 1).await;
                    let _ = handle.eof(channel_id).await;
                    let _ = handle.close(channel_id).await;
                    return Ok(());
                }
            }
        }

        let local_user = self
            .local_user
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not authenticated"))?;

        let shell = self.get_user_shell(local_user)?;

        // Get PTY config from channel state (if PTY was requested)
        let pty_config = self
            .channels
            .get(&channel_id)
            .and_then(|ch| ch.pty_config.clone())
            .unwrap_or_default();

        let env: Vec<(String, String)> = self
            .channels
            .get(&channel_id)
            .map(|ch| ch.env.clone().into_iter().collect())
            .unwrap_or_default();

        let home_dir = self.get_user_home(local_user);

        // Create PTY session for exec (using shell -c command)
        let pty = AsyncPtySession::new(&shell, pty_config, env, home_dir)?;
        let pty = Arc::new(Mutex::new(pty));

        // Send command to PTY
        {
            let pty_guard = pty.lock().await;
            let cmd_with_newline = format!("{}\n", command);
            pty_guard.send_input(cmd_with_newline.into_bytes()).await?;
        }

        // Create recorder if enabled (include command in session type)
        let session_type = format!("exec:{}", command.chars().take(50).collect::<String>());
        let recorder = self.create_recorder(channel_id, &session_type).await;

        // Store PTY and recorder in channel state
        if let Some(mut channel) = self.channels.get_mut(&channel_id) {
            channel.pty = Some(pty.clone());
            channel.recorder = recorder.clone();
        }

        // Spawn I/O forwarding task with recording
        self.spawn_pty_io_task(channel_id, pty, handle, recorder);

        // Emit session started event
        self.backend
            .on_ssh_event(SshEvent::SessionStarted {
                conn_id: self.conn_id.clone(),
                session_type: format!("exec: {}", command),
            })
            .await;

        Ok(())
    }

    /// Execute a command (no recording)
    #[cfg(not(feature = "recording"))]
    async fn exec_command(
        &self,
        channel_id: ChannelId,
        command: &str,
        handle: Handle,
    ) -> anyhow::Result<()> {
        // Check command filter
        if let Some(filter) = self.create_command_filter() {
            use crate::server::command_filter::CommandFilterResult;
            match filter.check_command(command) {
                CommandFilterResult::Allowed => {}
                CommandFilterResult::Blocked { reason } => {
                    warn!("Command blocked: {} - {}", command, reason);
                    self.backend
                        .on_ssh_event(SshEvent::CommandBlocked {
                            conn_id: self.conn_id.clone(),
                            command: command.to_string(),
                            reason: reason.clone(),
                        })
                        .await;

                    // Send error message to client
                    let msg = format!("Command blocked: {}\r\n", reason);
                    let _ = handle
                        .extended_data(channel_id, 1, CryptoVec::from_slice(msg.as_bytes()))
                        .await;
                    let _ = handle.exit_status_request(channel_id, 1).await;
                    let _ = handle.eof(channel_id).await;
                    let _ = handle.close(channel_id).await;
                    return Ok(());
                }
                CommandFilterResult::PathNotAllowed { path } => {
                    warn!("Path not allowed: {}", path);
                    let msg = format!("Path not allowed: {}\r\n", path);
                    let _ = handle
                        .extended_data(channel_id, 1, CryptoVec::from_slice(msg.as_bytes()))
                        .await;
                    let _ = handle.exit_status_request(channel_id, 1).await;
                    let _ = handle.eof(channel_id).await;
                    let _ = handle.close(channel_id).await;
                    return Ok(());
                }
                CommandFilterResult::ReadOnlyViolation => {
                    warn!("Read-only violation for command: {}", command);
                    let msg = "Read-only mode: write operations not allowed\r\n";
                    let _ = handle
                        .extended_data(channel_id, 1, CryptoVec::from_slice(msg.as_bytes()))
                        .await;
                    let _ = handle.exit_status_request(channel_id, 1).await;
                    let _ = handle.eof(channel_id).await;
                    let _ = handle.close(channel_id).await;
                    return Ok(());
                }
            }
        }

        let local_user = self
            .local_user
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not authenticated"))?;

        let shell = self.get_user_shell(local_user)?;

        // Get PTY config from channel state (if PTY was requested)
        let pty_config = self
            .channels
            .get(&channel_id)
            .and_then(|ch| ch.pty_config.clone())
            .unwrap_or_default();

        let env: Vec<(String, String)> = self
            .channels
            .get(&channel_id)
            .map(|ch| ch.env.clone().into_iter().collect())
            .unwrap_or_default();

        let home_dir = self.get_user_home(local_user);

        // Create PTY session for exec (using shell -c command)
        let pty = AsyncPtySession::new(&shell, pty_config, env, home_dir)?;
        let pty = Arc::new(Mutex::new(pty));

        // Send command to PTY
        {
            let pty_guard = pty.lock().await;
            let cmd_with_newline = format!("{}\n", command);
            pty_guard.send_input(cmd_with_newline.into_bytes()).await?;
        }

        // Store PTY in channel state
        if let Some(mut channel) = self.channels.get_mut(&channel_id) {
            channel.pty = Some(pty.clone());
        }

        // Spawn I/O forwarding task
        self.spawn_pty_io_task(channel_id, pty, handle);

        // Emit session started event
        self.backend
            .on_ssh_event(SshEvent::SessionStarted {
                conn_id: self.conn_id.clone(),
                session_type: format!("exec: {}", command),
            })
            .await;

        Ok(())
    }

    /// Get user's shell
    #[cfg(unix)]
    fn get_user_shell(&self, username: &str) -> anyhow::Result<String> {
        use users::{get_user_by_name, os::unix::UserExt};

        if let Some(user) = get_user_by_name(username) {
            Ok(user.shell().to_string_lossy().to_string())
        } else {
            // Default shell
            Ok("/bin/sh".to_string())
        }
    }

    #[cfg(windows)]
    fn get_user_shell(&self, _username: &str) -> anyhow::Result<String> {
        // On Windows, use PowerShell or cmd
        if std::path::Path::new("C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe")
            .exists()
        {
            Ok("C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe".to_string())
        } else {
            Ok("C:\\Windows\\System32\\cmd.exe".to_string())
        }
    }

    /// Get user's home directory
    #[cfg(unix)]
    fn get_user_home(&self, username: &str) -> Option<String> {
        use users::{get_user_by_name, os::unix::UserExt};

        get_user_by_name(username).map(|u| u.home_dir().to_string_lossy().to_string())
    }

    #[cfg(windows)]
    fn get_user_home(&self, username: &str) -> Option<String> {
        // On Windows, use USERPROFILE or construct from username
        std::env::var("USERPROFILE")
            .ok()
            .or_else(|| Some(format!("C:\\Users\\{}", username)))
    }
}

#[async_trait]
impl russh::server::Handler for SshConnectionHandler {
    type Error = anyhow::Error;

    /// Called when the client sends its identification banner
    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        if !self.auth_completed {
            warn!("Channel open before auth completed");
            return Ok(false);
        }

        let channel_id = channel.id();
        debug!("Channel open session request: {:?}", channel_id);

        // Create channel state - store the channel for later use (e.g., SFTP)
        #[cfg(feature = "recording")]
        let channel_state = ChannelState {
            channel: Some(channel),
            pty: None,
            pty_config: None,
            shell_requested: false,
            env: HashMap::new(),
            subsystem: ChannelSubsystem::None,
            recorder: None,
        };

        #[cfg(not(feature = "recording"))]
        let channel_state = ChannelState {
            channel: Some(channel),
            pty: None,
            pty_config: None,
            shell_requested: false,
            env: HashMap::new(),
            subsystem: ChannelSubsystem::None,
        };

        self.channels.insert(channel_id, channel_state);

        Ok(true)
    }

    /// Called when authentication with "none" method is attempted
    /// OmniEdge uses VPN identity, so we accept "none" and authenticate via IP
    async fn auth_none(&mut self, user: &str) -> Result<Auth, Self::Error> {
        debug!(
            "Auth none attempt for user '{}' from {}",
            user, self.peer_addr
        );

        self.ssh_user = Some(user.to_string());

        // Authenticate using OmniEdge identity
        let result = self
            .authenticator
            .authenticate(self.peer_addr.ip(), user)
            .await?;

        match result {
            AuthResult::Accept { local_user, action } => {
                info!(
                    "SSH auth accepted: {} -> {} (local: {}) from {}",
                    user, self.conn_id, local_user, self.peer_addr
                );

                // Store peer identity
                if let Ok(Some(peer)) = self.backend.who_is(self.peer_addr.ip()).await {
                    self.peer_identity = Some(peer);
                }

                self.local_user = Some(local_user.clone());
                self.action = Some(action.clone());
                self.auth_completed = true;

                // Register connection
                let conn_info = SshConnInfo {
                    connection_id: self.conn_id.clone(),
                    ssh_user: user.to_string(),
                    src_addr: self.peer_addr,
                    dst_addr: self.peer_addr, // Will be updated
                    peer_node: self
                        .peer_identity
                        .as_ref()
                        .map(|p| p.node.clone())
                        .unwrap_or_else(|| NodeInfo {
                            id: "unknown".to_string(),
                            name: "unknown".to_string(),
                            virtual_ip: self.peer_addr.ip().to_string(),
                            tags: vec![],
                            online: true,
                            network_id: "unknown".to_string(),
                        }),
                    user_profile: self
                        .peer_identity
                        .as_ref()
                        .map(|p| p.user.clone())
                        .unwrap_or_else(|| UserProfile {
                            id: "unknown".to_string(),
                            email: "unknown".to_string(),
                            name: None,
                        }),
                };

                let conn = Arc::new(SshConnection::new(self.conn_id.clone(), conn_info, action));
                self.active_conns.insert(self.conn_id.clone(), conn);

                // Emit authenticated event
                self.backend
                    .on_ssh_event(SshEvent::ConnectionAuthenticated {
                        conn_id: self.conn_id.clone(),
                        ssh_user: user.to_string(),
                        local_user,
                    })
                    .await;

                Ok(Auth::Accept)
            }
            AuthResult::Reject { message } => {
                warn!(
                    "SSH auth rejected for '{}' from {}: {}",
                    user, self.peer_addr, message
                );

                self.backend
                    .on_ssh_event(SshEvent::ConnectionRejected {
                        src: self.peer_addr,
                        reason: message.clone(),
                    })
                    .await;

                Ok(Auth::Reject {
                    proceed_with_methods: None,
                })
            }
            AuthResult::HoldAndDelegate { url } => {
                // For now, treat hold-and-delegate as partial success
                // Client needs to complete interactive approval
                info!(
                    "SSH auth requires approval for '{}' from {}: {}",
                    user, self.peer_addr, url
                );

                // In a full implementation, we would wait for approval
                // For now, reject with a message
                Ok(Auth::Reject {
                    proceed_with_methods: None,
                })
            }
        }
    }

    /// Called when authentication with public key is attempted
    async fn auth_publickey_offered(
        &mut self,
        user: &str,
        public_key: &PublicKey,
    ) -> Result<Auth, Self::Error> {
        debug!(
            "Public key offered for user '{}' from {}: {:?}",
            user,
            self.peer_addr,
            public_key.name()
        );

        // OmniEdge doesn't use SSH keys for auth - identity comes from VPN
        // We can optionally verify the key matches a registered key for audit purposes
        // For now, fall through to "none" auth
        Ok(Auth::Reject {
            proceed_with_methods: Some(russh::MethodSet::NONE),
        })
    }

    /// Called when authentication with public key signature is attempted
    async fn auth_publickey(
        &mut self,
        user: &str,
        public_key: &PublicKey,
    ) -> Result<Auth, Self::Error> {
        debug!(
            "Public key auth for user '{}' from {}: {:?}",
            user,
            self.peer_addr,
            public_key.name()
        );

        // Same as above - we use VPN identity instead
        Ok(Auth::Reject {
            proceed_with_methods: Some(russh::MethodSet::NONE),
        })
    }

    /// Called when a PTY is requested
    async fn pty_request(
        &mut self,
        channel: ChannelId,
        term: &str,
        col_width: u32,
        row_height: u32,
        pix_width: u32,
        pix_height: u32,
        _modes: &[(russh::Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if !self.config.enable_pty {
            warn!("PTY request denied - PTY disabled in config");
            return Ok(());
        }

        debug!(
            "PTY request on channel {:?}: term={}, {}x{} ({}x{} px)",
            channel, term, col_width, row_height, pix_width, pix_height
        );

        let pty_config = PtyConfig {
            term: term.to_string(),
            cols: col_width as u16,
            rows: row_height as u16,
            pixel_width: pix_width as u16,
            pixel_height: pix_height as u16,
        };

        // Store PTY config for later use when shell/exec is requested
        if let Some(mut channel_state) = self.channels.get_mut(&channel) {
            channel_state.pty_config = Some(pty_config);
        }

        // Accept PTY request
        session.request_success();
        Ok(())
    }

    /// Called when terminal size changes
    async fn window_change_request(
        &mut self,
        channel: ChannelId,
        col_width: u32,
        row_height: u32,
        pix_width: u32,
        pix_height: u32,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        debug!(
            "Window change on channel {:?}: {}x{} ({}x{} px)",
            channel, col_width, row_height, pix_width, pix_height
        );

        // Update PTY config and record resize
        if let Some(mut channel_state) = self.channels.get_mut(&channel) {
            if let Some(ref mut config) = channel_state.pty_config {
                config.cols = col_width as u16;
                config.rows = row_height as u16;
                config.pixel_width = pix_width as u16;
                config.pixel_height = pix_height as u16;
            }

            // Record resize event if recording is enabled
            #[cfg(feature = "recording")]
            if let Some(ref recorder) = channel_state.recorder {
                let mut rec_guard = recorder.lock().await;
                if let Err(e) = rec_guard.record_resize(col_width, row_height).await {
                    warn!("Failed to record resize: {}", e);
                }
            }

            // If PTY is active, resize it
            // Note: The AsyncPtySession needs a resize method - we'll handle this via message
            // For now, just update the config
        }

        Ok(())
    }

    /// Called when shell is requested
    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        debug!("Shell request on channel {:?}", channel);

        if let Some(mut channel_state) = self.channels.get_mut(&channel) {
            channel_state.shell_requested = true;
        }

        let handle = session.handle();

        match self.start_shell(channel, handle).await {
            Ok(()) => {
                session.request_success();
            }
            Err(e) => {
                error!("Failed to start shell: {}", e);
                session.request_failure();
            }
        }

        Ok(())
    }

    /// Called when command execution is requested
    async fn exec_request(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let command = String::from_utf8_lossy(data);
        debug!("Exec request on channel {:?}: {}", channel, command);

        let handle = session.handle();

        match self.exec_command(channel, &command, handle).await {
            Ok(()) => {
                session.request_success();
            }
            Err(e) => {
                error!("Failed to execute command: {}", e);
                session.request_failure();
            }
        }

        Ok(())
    }

    /// Called when subsystem is requested (e.g., SFTP)
    async fn subsystem_request(
        &mut self,
        channel_id: ChannelId,
        name: &str,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        debug!("Subsystem request on channel {:?}: {}", channel_id, name);

        match name {
            #[cfg(feature = "sftp")]
            "sftp" => {
                if !self.config.enable_sftp {
                    warn!("SFTP subsystem disabled");
                    session.request_failure();
                    return Ok(());
                }

                // Check if SFTP is allowed by policy
                if let Some(action) = &self.action {
                    if !action.allow_sftp {
                        warn!("SFTP not allowed by policy");
                        session.request_failure();
                        return Ok(());
                    }
                }

                // Get the channel from channel state
                let channel = {
                    let mut channel_state = self.channels.get_mut(&channel_id);
                    if let Some(ref mut state) = channel_state {
                        state.subsystem = ChannelSubsystem::Sftp;
                        state.channel.take()
                    } else {
                        None
                    }
                };

                let channel = match channel {
                    Some(ch) => ch,
                    None => {
                        error!("Channel not found or already taken for SFTP");
                        session.request_failure();
                        return Ok(());
                    }
                };

                // Get local user's home directory as SFTP root
                let root_dir = self
                    .local_user
                    .as_ref()
                    .and_then(|u| self.get_user_home(u))
                    .map(std::path::PathBuf::from);

                // Create SFTP config from action
                let sftp_config = if let Some(action) = &self.action {
                    crate::sftp::SftpServerConfig {
                        root_dir,
                        allow_read: true,
                        allow_write: !action.read_only,
                        max_file_size: None,
                        allowed_paths: action.allowed_paths.clone(),
                    }
                } else {
                    crate::sftp::SftpServerConfig {
                        root_dir,
                        ..Default::default()
                    }
                };

                // Create SFTP handler
                let sftp_handler = crate::sftp::SftpHandler::new(
                    sftp_config,
                    self.conn_id.clone(),
                    self.ssh_user.clone().unwrap_or_default(),
                );

                let conn_id = self.conn_id.clone();
                let backend = self.backend.clone();

                info!(
                    "[{}] SFTP subsystem starting on channel {:?}",
                    conn_id, channel_id
                );

                // Accept the subsystem request before spawning the handler
                session.channel_success(channel_id);

                // Emit session started event
                self.backend
                    .on_ssh_event(SshEvent::SessionStarted {
                        conn_id: conn_id.clone(),
                        session_type: "sftp".to_string(),
                    })
                    .await;

                // Convert channel to stream and run SFTP handler
                let stream = channel.into_stream();

                // Spawn SFTP handler task
                tokio::spawn(async move {
                    info!("[{}] SFTP handler task started", conn_id);

                    // Run the SFTP server on this channel's stream
                    russh_sftp::server::run(stream, sftp_handler).await;

                    info!("[{}] SFTP handler task completed", conn_id);

                    // Emit session ended event
                    backend
                        .on_ssh_event(SshEvent::SessionEnded {
                            conn_id: conn_id.clone(),
                        })
                        .await;
                });
            }
            #[cfg(not(feature = "sftp"))]
            "sftp" => {
                warn!("SFTP feature not enabled");
                session.request_failure();
            }
            _ => {
                warn!("Unknown subsystem requested: {}", name);
                session.request_failure();
            }
        }

        Ok(())
    }

    /// Called when environment variable is set
    async fn env_request(
        &mut self,
        channel: ChannelId,
        variable_name: &str,
        variable_value: &str,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        debug!(
            "Env request on channel {:?}: {}={}",
            channel, variable_name, variable_value
        );

        // Check if this env var is in the accept list
        let should_accept = if self.action.is_some() {
            // Get accept_env patterns from the matched rule
            // For now, accept common safe variables
            matches!(
                variable_name,
                "TERM" | "LANG" | "LC_ALL" | "LC_CTYPE" | "TZ" | "COLORTERM"
            )
        } else {
            false
        };

        if should_accept {
            if let Some(mut channel_state) = self.channels.get_mut(&channel) {
                channel_state
                    .env
                    .insert(variable_name.to_string(), variable_value.to_string());
            }
            session.request_success();
        } else {
            debug!("Env variable {} not accepted", variable_name);
            session.request_failure();
        }

        Ok(())
    }

    /// Called when data is received on a channel
    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        // Forward data to PTY and record input if enabled
        if let Some(channel_state) = self.channels.get(&channel) {
            // Record input if recorder is present
            #[cfg(feature = "recording")]
            if let Some(ref recorder) = channel_state.recorder {
                let mut rec_guard = recorder.lock().await;
                if let Err(e) = rec_guard.record_input(data).await {
                    warn!("Failed to record input: {}", e);
                }
            }

            // Forward to PTY
            if let Some(ref pty) = channel_state.pty {
                let pty_guard = pty.lock().await;
                if let Err(e) = pty_guard.send_input(data.to_vec()).await {
                    error!("Failed to send data to PTY: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Called when channel is closed
    async fn channel_close(
        &mut self,
        channel: ChannelId,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        debug!("Channel close: {:?}", channel);

        // Clean up channel state
        self.channels.remove(&channel);

        Ok(())
    }

    /// Called when EOF is received
    async fn channel_eof(
        &mut self,
        channel: ChannelId,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        debug!("Channel EOF: {:?}", channel);

        // Signal EOF to PTY if present
        // The PTY task will handle cleanup

        Ok(())
    }
}

impl Drop for SshConnectionHandler {
    fn drop(&mut self) {
        // Clean up when handler is dropped (connection closed)
        let conn_id = self.conn_id.clone();
        let active_conns = self.active_conns.clone();

        // Remove from active connections
        active_conns.remove(&conn_id);

        // Note: We can't easily emit async events in Drop
        // The session ended events are emitted by the PTY task
        info!("SSH connection handler dropped: {}", conn_id);
    }
}

/// Server implementation for russh
pub struct OmniEdgeSshServer {
    backend: Arc<dyn SshBackend>,
    config: SshServerConfig,
    active_conns: DashMap<String, Arc<SshConnection>>,
}

impl OmniEdgeSshServer {
    /// Create a new SSH server instance
    pub fn new(backend: Arc<dyn SshBackend>, config: SshServerConfig) -> Self {
        Self {
            backend,
            config,
            active_conns: DashMap::new(),
        }
    }

    /// Create a handler for a new connection
    pub fn make_handler(&self, peer_addr: SocketAddr) -> SshConnectionHandler {
        SshConnectionHandler::new(
            self.backend.clone(),
            self.config.clone(),
            peer_addr,
            self.active_conns.clone(),
        )
    }

    /// Get number of active connections
    pub fn num_active_conns(&self) -> usize {
        self.active_conns.len()
    }

    /// Get active connections
    pub fn active_conns(&self) -> &DashMap<String, Arc<SshConnection>> {
        &self.active_conns
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tests will be added once the basic structure is verified
}
