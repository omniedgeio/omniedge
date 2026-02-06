//! SSH Client for connecting to OmniEdge peers
//!
//! This module provides SSH client functionality for connecting to other
//! nodes in the OmniEdge network using OmniEdge identity authentication.

use crate::server::SshBackend;
use async_trait::async_trait;
use russh::client::{Config as ClientConfig, Handle, Msg};
use russh::{Channel, ChannelId, Disconnect, Sig};
use russh_keys::key::PublicKey;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info};

/// SSH client for connecting to peers
pub struct SshClient {
    backend: Arc<dyn SshBackend>,
}

/// Target for SSH connection
#[derive(Debug, Clone)]
pub struct SshTarget {
    /// User to connect as
    pub user: String,
    /// Peer name or IP
    pub host: String,
    /// Port (default 22)
    pub port: u16,
}

impl SshTarget {
    /// Create a new SSH target
    pub fn new(host: impl Into<String>, user: impl Into<String>) -> Self {
        Self {
            user: user.into(),
            host: host.into(),
            port: 22,
        }
    }

    /// Set custom port
    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Parse from user@host:port format
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        let (user, rest) = if let Some(pos) = s.find('@') {
            (s[..pos].to_string(), &s[pos + 1..])
        } else {
            return Err(anyhow::anyhow!("Missing user in target: {}", s));
        };

        let (host, port) = if let Some(pos) = rest.find(':') {
            let port: u16 = rest[pos + 1..].parse()?;
            (rest[..pos].to_string(), port)
        } else {
            (rest.to_string(), 22)
        };

        Ok(Self { user, host, port })
    }
}

/// Active SSH session
pub struct SshSession {
    /// Russh client handle
    handle: Handle<OmniEdgeClientHandler>,
    /// Target we connected to
    target: SshTarget,
    /// Active channels
    channels: HashMap<ChannelId, ChannelState>,
}

/// State of an SSH channel
struct ChannelState {
    /// Channel type
    channel_type: ChannelType,
}

/// Type of SSH channel
#[derive(Debug, Clone)]
enum ChannelType {
    Session,
    Shell,
    Exec,
    Sftp,
}

impl SshClient {
    /// Create a new SSH client
    pub fn new(backend: Arc<dyn SshBackend>) -> Self {
        Self { backend }
    }

    /// Connect to peer via SSH over VPN tunnel
    pub async fn connect(&self, target: SshTarget) -> anyhow::Result<SshSession> {
        // 1. Resolve target peer to VPN address
        let peer_addr = self.resolve_peer(&target).await?;

        info!(
            host = %target.host,
            user = %target.user,
            addr = %peer_addr,
            "Connecting to SSH peer"
        );

        // 2. Connect via VPN tunnel
        let stream = tokio::net::TcpStream::connect(peer_addr).await?;

        // 3. Create client handler
        let handler = OmniEdgeClientHandler::new(self.backend.clone());

        // 4. SSH handshake
        let config = Arc::new(ClientConfig::default());
        let mut handle = russh::client::connect_stream(config, stream, handler).await?;

        // 5. Authenticate with "none" method (OmniEdge identity)
        let auth_result = handle.authenticate_none(&target.user).await?;
        if !auth_result {
            return Err(anyhow::anyhow!(
                "Authentication failed for user '{}'",
                target.user
            ));
        }

        info!("SSH connection established to {}", target.host);

        Ok(SshSession {
            handle,
            target,
            channels: HashMap::new(),
        })
    }

    /// Resolve peer name/IP to socket address
    async fn resolve_peer(&self, target: &SshTarget) -> anyhow::Result<SocketAddr> {
        // Try parsing as IP first
        if let Ok(ip) = target.host.parse() {
            return Ok(SocketAddr::new(ip, target.port));
        }

        // Try to resolve via OmniEdge backend (peer name lookup)
        if let Some(ip) = self.backend.resolve_peer_name(&target.host).await? {
            debug!(
                host = %target.host,
                ip = %ip,
                "Resolved peer name via OmniEdge"
            );
            return Ok(SocketAddr::new(ip, target.port));
        }

        // Fall back to DNS resolution
        debug!(
            host = %target.host,
            "Peer not found in OmniEdge network, trying DNS"
        );
        let addrs = tokio::net::lookup_host(format!("{}:{}", target.host, target.port)).await?;

        addrs
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("Could not resolve host: {}", target.host))
    }
}

impl SshSession {
    /// Check if session is connected
    pub fn is_connected(&self) -> bool {
        // The handle is valid if it hasn't been closed
        true // TODO: Check actual connection state
    }

    /// Get target info
    pub fn target(&self) -> &SshTarget {
        &self.target
    }

    /// Execute a command and return the result
    pub async fn exec(&mut self, command: &str) -> anyhow::Result<ExecResult> {
        debug!(command = %command, "Executing remote command");

        // Open a session channel
        let mut channel = self.handle.channel_open_session().await?;

        // Execute command
        channel.exec(true, command).await?;

        // Collect output by reading channel messages
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut exit_code: i32 = -1;

        // Read from channel until it closes
        loop {
            match channel.wait().await {
                Some(msg) => match msg {
                    russh::ChannelMsg::Data { data } => {
                        stdout.extend_from_slice(&data);
                    }
                    russh::ChannelMsg::ExtendedData { data, ext } => {
                        if ext == 1 {
                            // stderr
                            stderr.extend_from_slice(&data);
                        }
                    }
                    russh::ChannelMsg::ExitStatus { exit_status } => {
                        exit_code = exit_status as i32;
                    }
                    russh::ChannelMsg::ExitSignal {
                        signal_name,
                        core_dumped,
                        error_message,
                        ..
                    } => {
                        debug!(
                            signal = ?signal_name,
                            core_dumped = core_dumped,
                            error = %error_message,
                            "Process killed by signal"
                        );
                        // Signal 128 + signal number convention
                        exit_code = 128;
                    }
                    russh::ChannelMsg::Eof => {
                        debug!("Channel EOF received");
                    }
                    russh::ChannelMsg::Close => {
                        debug!("Channel closed");
                        break;
                    }
                    _ => {}
                },
                None => {
                    // Channel closed
                    break;
                }
            }
        }

        Ok(ExecResult {
            exit_code,
            stdout,
            stderr,
        })
    }

    /// Start interactive shell
    pub async fn shell(&mut self) -> anyhow::Result<ShellChannel> {
        debug!("Starting interactive shell");

        // Open a session channel
        let channel = self.handle.channel_open_session().await?;

        // Request PTY
        channel
            .request_pty(false, "xterm-256color", 80, 24, 0, 0, &[])
            .await?;

        // Request shell
        channel.request_shell(false).await?;

        info!("Interactive shell started");

        Ok(ShellChannel { channel })
    }

    /// Open SFTP session
    #[cfg(feature = "sftp")]
    pub async fn sftp(&mut self) -> anyhow::Result<crate::sftp::SftpClient> {
        debug!("Opening SFTP session");

        // Open a session channel
        let channel = self.handle.channel_open_session().await?;

        // Request SFTP subsystem
        channel.request_subsystem(true, "sftp").await?;

        // Create SFTP client from channel
        let client = crate::sftp::SftpClient::new(channel).await?;

        info!("SFTP session opened");
        Ok(client)
    }

    /// Close the session
    pub async fn close(&mut self) -> anyhow::Result<()> {
        debug!("Closing SSH session");
        self.handle
            .disconnect(Disconnect::ByApplication, "Session closed", "en")
            .await?;
        Ok(())
    }

    /// Request local port forwarding
    pub async fn local_forward(
        &mut self,
        local_port: u16,
        remote_host: &str,
        remote_port: u16,
    ) -> anyhow::Result<()> {
        info!(
            local_port = local_port,
            remote_host = %remote_host,
            remote_port = remote_port,
            "Setting up local port forwarding"
        );

        // TODO: Implement local port forwarding
        // This would create a local TCP listener and forward connections
        // through the SSH tunnel to the remote host:port

        Ok(())
    }

    /// Request remote port forwarding
    pub async fn remote_forward(
        &mut self,
        remote_port: u16,
        local_host: &str,
        local_port: u16,
    ) -> anyhow::Result<()> {
        info!(
            remote_port = remote_port,
            local_host = %local_host,
            local_port = local_port,
            "Setting up remote port forwarding"
        );

        // Request the server to listen on remote_port
        self.handle
            .tcpip_forward("0.0.0.0", remote_port.into())
            .await?;

        // TODO: Handle incoming connections from the server

        Ok(())
    }
}

/// Interactive shell channel
pub struct ShellChannel {
    channel: Channel<Msg>,
}

impl ShellChannel {
    /// Send data to the shell
    pub async fn write(&self, data: &[u8]) -> anyhow::Result<()> {
        self.channel.data(data).await?;
        Ok(())
    }

    /// Resize the terminal
    pub async fn resize(&self, cols: u32, rows: u32) -> anyhow::Result<()> {
        self.channel.window_change(cols, rows, 0, 0).await?;
        Ok(())
    }

    /// Send a signal to the remote process
    pub async fn signal(&self, signal: &str) -> anyhow::Result<()> {
        // Convert signal name to Sig enum
        let sig = match signal.to_uppercase().as_str() {
            "INT" | "SIGINT" => Sig::INT,
            "TERM" | "SIGTERM" => Sig::TERM,
            "KILL" | "SIGKILL" => Sig::KILL,
            "HUP" | "SIGHUP" => Sig::HUP,
            "QUIT" | "SIGQUIT" => Sig::QUIT,
            _ => return Err(anyhow::anyhow!("Unknown signal: {}", signal)),
        };

        self.channel.signal(sig).await?;
        Ok(())
    }

    /// Close the shell channel
    pub async fn close(self) -> anyhow::Result<()> {
        self.channel.eof().await?;
        self.channel.close().await?;
        Ok(())
    }

    /// Get the channel ID
    pub fn id(&self) -> ChannelId {
        self.channel.id()
    }
}

/// Result of command execution
#[derive(Debug)]
pub struct ExecResult {
    /// Exit code
    pub exit_code: i32,
    /// Standard output
    pub stdout: Vec<u8>,
    /// Standard error
    pub stderr: Vec<u8>,
}

impl ExecResult {
    /// Check if command succeeded (exit code 0)
    pub fn success(&self) -> bool {
        self.exit_code == 0
    }

    /// Get stdout as string
    pub fn stdout_str(&self) -> String {
        String::from_utf8_lossy(&self.stdout).to_string()
    }

    /// Get stderr as string
    pub fn stderr_str(&self) -> String {
        String::from_utf8_lossy(&self.stderr).to_string()
    }
}

/// Client handler for OmniEdge identity-based auth
pub struct OmniEdgeClientHandler {
    backend: Arc<dyn SshBackend>,
    /// Channel data receivers for collecting output
    channel_data: Arc<Mutex<HashMap<ChannelId, ChannelDataCollector>>>,
}

/// Collects data from a channel
struct ChannelDataCollector {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    exit_status: Option<u32>,
    eof: bool,
}

impl OmniEdgeClientHandler {
    /// Create a new client handler
    pub fn new(backend: Arc<dyn SshBackend>) -> Self {
        Self {
            backend,
            channel_data: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl russh::client::Handler for OmniEdgeClientHandler {
    type Error = anyhow::Error;

    /// Verify the server's host key
    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        // In OmniEdge, we trust peers in the same network
        // The VPN tunnel already provides authentication
        // Could add TOFU (Trust On First Use) or verify against known keys

        debug!(
            "Server key received: {} ({})",
            server_public_key.name(),
            server_public_key.fingerprint()
        );

        // TODO: Optionally verify against known host keys
        // For now, trust all peers in the network
        Ok(true)
    }

    /// Called when data is received on a channel
    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        _session: &mut russh::client::Session,
    ) -> Result<(), Self::Error> {
        debug!("Received {} bytes on channel {:?}", data.len(), channel);

        let mut collectors = self.channel_data.lock().await;
        if let Some(collector) = collectors.get_mut(&channel) {
            collector.stdout.extend_from_slice(data);
        }

        Ok(())
    }

    /// Called when extended data is received (stderr)
    async fn extended_data(
        &mut self,
        channel: ChannelId,
        ext: u32,
        data: &[u8],
        _session: &mut russh::client::Session,
    ) -> Result<(), Self::Error> {
        debug!(
            "Received {} bytes extended data (type {}) on channel {:?}",
            data.len(),
            ext,
            channel
        );

        if ext == 1 {
            // stderr
            let mut collectors = self.channel_data.lock().await;
            if let Some(collector) = collectors.get_mut(&channel) {
                collector.stderr.extend_from_slice(data);
            }
        }

        Ok(())
    }

    /// Called when exit status is received
    async fn exit_status(
        &mut self,
        channel: ChannelId,
        exit_status: u32,
        _session: &mut russh::client::Session,
    ) -> Result<(), Self::Error> {
        debug!("Channel {:?} exit status: {}", channel, exit_status);

        let mut collectors = self.channel_data.lock().await;
        if let Some(collector) = collectors.get_mut(&channel) {
            collector.exit_status = Some(exit_status);
        }

        Ok(())
    }

    /// Called when channel EOF is received
    async fn channel_eof(
        &mut self,
        channel: ChannelId,
        _session: &mut russh::client::Session,
    ) -> Result<(), Self::Error> {
        debug!("Channel {:?} EOF", channel);

        let mut collectors = self.channel_data.lock().await;
        if let Some(collector) = collectors.get_mut(&channel) {
            collector.eof = true;
        }

        Ok(())
    }

    /// Called when channel is closed
    async fn channel_close(
        &mut self,
        channel: ChannelId,
        _session: &mut russh::client::Session,
    ) -> Result<(), Self::Error> {
        debug!("Channel {:?} closed", channel);

        // Clean up channel data collector
        let mut collectors = self.channel_data.lock().await;
        collectors.remove(&channel);

        Ok(())
    }

    /// Called when a server-initiated channel is opened (for remote forwarding)
    async fn server_channel_open_forwarded_tcpip(
        &mut self,
        _channel: Channel<Msg>,
        connected_address: &str,
        connected_port: u32,
        originator_address: &str,
        originator_port: u32,
        _session: &mut russh::client::Session,
    ) -> Result<(), Self::Error> {
        info!(
            "Server opened forwarded channel: {}:{} from {}:{}",
            connected_address, connected_port, originator_address, originator_port
        );

        // TODO: Handle forwarded connection
        // Connect to local target and forward data

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ssh_target_parse() {
        let target = SshTarget::parse("user@host").unwrap();
        assert_eq!(target.user, "user");
        assert_eq!(target.host, "host");
        assert_eq!(target.port, 22);

        let target = SshTarget::parse("admin@192.168.1.1:2222").unwrap();
        assert_eq!(target.user, "admin");
        assert_eq!(target.host, "192.168.1.1");
        assert_eq!(target.port, 2222);
    }

    #[test]
    fn test_ssh_target_parse_invalid() {
        assert!(SshTarget::parse("hostonly").is_err());
    }

    #[test]
    fn test_ssh_target_builder() {
        let target = SshTarget::new("myhost", "myuser").with_port(2222);
        assert_eq!(target.user, "myuser");
        assert_eq!(target.host, "myhost");
        assert_eq!(target.port, 2222);
    }

    #[test]
    fn test_exec_result() {
        let result = ExecResult {
            exit_code: 0,
            stdout: b"hello world\n".to_vec(),
            stderr: Vec::new(),
        };

        assert!(result.success());
        assert_eq!(result.stdout_str(), "hello world\n");
        assert_eq!(result.stderr_str(), "");
    }
}
