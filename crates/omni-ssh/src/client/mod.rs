//! SSH Client for connecting to OmniEdge peers
//!
//! This module provides SSH client functionality for connecting to other
//! nodes in the OmniEdge network using OmniEdge identity authentication.
//!
//! ## Host Key Verification
//!
//! The client supports multiple host key verification modes:
//! - **TOFU (Trust On First Use)**: Accept and remember keys on first connection
//! - **Strict**: Only accept keys in the known hosts store
//! - **AcceptNew**: Accept new hosts but reject changed keys
//! - **Insecure**: Accept all keys (not recommended for production)

use crate::server::SshBackend;
use async_trait::async_trait;
use russh::client::{Config as ClientConfig, Handle, Msg};
use russh::{Channel, ChannelId, Disconnect, Sig};
use russh_keys::key::PublicKey;
use russh_keys::PublicKeyBase64;
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

/// SSH client for connecting to peers
pub struct SshClient {
    backend: Arc<dyn SshBackend>,
    /// Host key verification settings
    host_key_config: HostKeyConfig,
}

/// Host key verification mode
#[derive(Debug, Clone, Default)]
pub enum HostKeyVerification {
    /// Trust On First Use - accept and remember new keys, reject changed keys
    #[default]
    Tofu,
    /// Only accept keys already in the known hosts store
    Strict,
    /// Accept new hosts but reject if key has changed
    AcceptNew,
    /// Accept all keys without verification (INSECURE - not recommended)
    Insecure,
}

/// Configuration for host key verification
#[derive(Debug, Clone)]
pub struct HostKeyConfig {
    /// Verification mode
    pub mode: HostKeyVerification,
    /// Path to known hosts file (defaults to ~/.omniedge/known_hosts)
    pub known_hosts_path: Option<PathBuf>,
}

impl Default for HostKeyConfig {
    fn default() -> Self {
        Self {
            mode: HostKeyVerification::Tofu,
            known_hosts_path: None,
        }
    }
}

impl HostKeyConfig {
    /// Create insecure config that accepts all keys (for testing only)
    pub fn insecure() -> Self {
        Self {
            mode: HostKeyVerification::Insecure,
            known_hosts_path: None,
        }
    }

    /// Create strict config that only accepts known keys
    pub fn strict() -> Self {
        Self {
            mode: HostKeyVerification::Strict,
            known_hosts_path: None,
        }
    }

    /// Set custom known hosts path
    pub fn with_known_hosts_path(mut self, path: PathBuf) -> Self {
        self.known_hosts_path = Some(path);
        self
    }
}

/// Known hosts store for host key verification
#[derive(Debug)]
pub struct KnownHostsStore {
    path: PathBuf,
    /// In-memory cache: host -> (key_type, fingerprint)
    hosts: std::sync::RwLock<HashMap<String, KnownHostEntry>>,
}

/// Entry in known hosts store
#[derive(Debug, Clone)]
struct KnownHostEntry {
    key_type: String,
    fingerprint: String,
    /// Base64-encoded public key
    public_key_base64: String,
}

impl KnownHostsStore {
    /// Create or load known hosts store
    pub fn new(path: PathBuf) -> anyhow::Result<Self> {
        let store = Self {
            path: path.clone(),
            hosts: std::sync::RwLock::new(HashMap::new()),
        };

        // Load existing entries if file exists
        if path.exists() {
            store.load()?;
        }

        Ok(store)
    }

    /// Get default path for known hosts file
    pub fn default_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".omniedge")
            .join("known_hosts")
    }

    /// Load entries from file
    fn load(&self) -> anyhow::Result<()> {
        let file = std::fs::File::open(&self.path)?;
        let reader = std::io::BufReader::new(file);
        let mut hosts = self.hosts.write().unwrap();

        for line in reader.lines() {
            let line = line?;
            let line = line.trim();

            // Skip comments and empty lines
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Format: host key_type base64_key
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                let host = parts[0].to_string();
                let key_type = parts[1].to_string();
                let public_key_base64 = parts[2].to_string();

                // Calculate fingerprint from base64 key
                let fingerprint = if let Ok(key_bytes) = base64::Engine::decode(
                    &base64::engine::general_purpose::STANDARD,
                    &public_key_base64,
                ) {
                    use sha2::{Digest, Sha256};
                    let hash = Sha256::digest(&key_bytes);
                    format!(
                        "SHA256:{}",
                        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &hash)
                    )
                } else {
                    continue;
                };

                hosts.insert(
                    host,
                    KnownHostEntry {
                        key_type,
                        fingerprint,
                        public_key_base64,
                    },
                );
            }
        }

        debug!("Loaded {} known hosts from {:?}", hosts.len(), self.path);
        Ok(())
    }

    /// Check if a host key is known and matches
    pub fn check(&self, host: &str, port: u16, key: &PublicKey) -> HostKeyCheckResult {
        let host_key = if port == 22 {
            host.to_string()
        } else {
            format!("[{}]:{}", host, port)
        };

        let hosts = self.hosts.read().unwrap();
        let fingerprint = key.fingerprint();

        if let Some(entry) = hosts.get(&host_key) {
            if entry.fingerprint == fingerprint {
                HostKeyCheckResult::Match
            } else {
                HostKeyCheckResult::Mismatch {
                    expected: entry.fingerprint.clone(),
                    actual: fingerprint,
                }
            }
        } else {
            HostKeyCheckResult::NotFound
        }
    }

    /// Add a host key to the store
    pub fn add(&self, host: &str, port: u16, key: &PublicKey) -> anyhow::Result<()> {
        let host_key = if port == 22 {
            host.to_string()
        } else {
            format!("[{}]:{}", host, port)
        };

        let key_type = key.name().to_string();
        let fingerprint = key.fingerprint();

        // Encode public key to base64
        let public_key_base64 = key.public_key_base64();

        // Add to memory
        {
            let mut hosts = self.hosts.write().unwrap();
            hosts.insert(
                host_key.clone(),
                KnownHostEntry {
                    key_type: key_type.clone(),
                    fingerprint: fingerprint.clone(),
                    public_key_base64: public_key_base64.clone(),
                },
            );
        }

        // Persist to file
        self.save_entry(&host_key, &key_type, &public_key_base64)?;

        info!(
            host = %host_key,
            key_type = %key_type,
            fingerprint = %fingerprint,
            "Added host key to known hosts"
        );

        Ok(())
    }

    /// Save a single entry to the known hosts file
    fn save_entry(
        &self,
        host: &str,
        key_type: &str,
        public_key_base64: &str,
    ) -> anyhow::Result<()> {
        // Ensure directory exists
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Append to file
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;

        writeln!(file, "{} {} {}", host, key_type, public_key_base64)?;

        Ok(())
    }

    /// Remove a host from the store
    pub fn remove(&self, host: &str, port: u16) -> anyhow::Result<bool> {
        let host_key = if port == 22 {
            host.to_string()
        } else {
            format!("[{}]:{}", host, port)
        };

        let removed = {
            let mut hosts = self.hosts.write().unwrap();
            hosts.remove(&host_key).is_some()
        };

        if removed {
            // Rewrite file without the removed entry
            self.rewrite_file()?;
        }

        Ok(removed)
    }

    /// Rewrite the entire known hosts file
    fn rewrite_file(&self) -> anyhow::Result<()> {
        let hosts = self.hosts.read().unwrap();

        // Ensure directory exists
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut file = std::fs::File::create(&self.path)?;

        writeln!(file, "# OmniEdge SSH known hosts")?;
        writeln!(file, "# Format: host key_type base64_public_key")?;

        for (host, entry) in hosts.iter() {
            writeln!(
                file,
                "{} {} {}",
                host, entry.key_type, entry.public_key_base64
            )?;
        }

        Ok(())
    }
}

/// Result of host key verification
#[derive(Debug, Clone, PartialEq)]
pub enum HostKeyCheckResult {
    /// Key matches stored key
    Match,
    /// Key doesn't match stored key (possible MITM attack)
    Mismatch {
        /// The expected fingerprint from known hosts
        expected: String,
        /// The actual fingerprint received from server
        actual: String,
    },
    /// Host not in known hosts
    NotFound,
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
    /// Create a new SSH client with default TOFU host key verification
    pub fn new(backend: Arc<dyn SshBackend>) -> Self {
        Self::with_config(backend, HostKeyConfig::default())
    }

    /// Create a new SSH client with custom host key verification config
    pub fn with_config(backend: Arc<dyn SshBackend>, host_key_config: HostKeyConfig) -> Self {
        Self {
            backend,
            host_key_config,
        }
    }

    /// Create a new SSH client with insecure mode (accepts all keys)
    /// WARNING: Only use for testing or when other security layers exist
    pub fn insecure(backend: Arc<dyn SshBackend>) -> Self {
        Self::with_config(backend, HostKeyConfig::insecure())
    }

    /// Get the known hosts store path
    fn known_hosts_path(&self) -> PathBuf {
        self.host_key_config
            .known_hosts_path
            .clone()
            .unwrap_or_else(KnownHostsStore::default_path)
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

        // 3. Initialize known hosts store for host key verification
        let known_hosts = Arc::new(KnownHostsStore::new(self.known_hosts_path())?);

        // 4. Create client handler with host key verification
        let handler = OmniEdgeClientHandler::new(
            self.backend.clone(),
            known_hosts,
            self.host_key_config.mode.clone(),
            target.host.clone(),
            target.port,
        );

        // 5. SSH handshake
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
    /// Known hosts store for host key verification
    known_hosts: Arc<KnownHostsStore>,
    /// Host key verification mode
    host_key_mode: HostKeyVerification,
    /// Target host being connected to
    target_host: String,
    /// Target port
    target_port: u16,
}

/// Collects data from a channel
struct ChannelDataCollector {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    exit_status: Option<u32>,
    eof: bool,
}

impl OmniEdgeClientHandler {
    /// Create a new client handler with host key verification
    pub fn new(
        backend: Arc<dyn SshBackend>,
        known_hosts: Arc<KnownHostsStore>,
        host_key_mode: HostKeyVerification,
        target_host: String,
        target_port: u16,
    ) -> Self {
        Self {
            backend,
            channel_data: Arc::new(Mutex::new(HashMap::new())),
            known_hosts,
            host_key_mode,
            target_host,
            target_port,
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
        let fingerprint = server_public_key.fingerprint();
        let key_type = server_public_key.name();

        debug!(
            host = %self.target_host,
            port = %self.target_port,
            key_type = %key_type,
            fingerprint = %fingerprint,
            "Verifying server host key"
        );

        // Check verification mode
        match self.host_key_mode {
            HostKeyVerification::Insecure => {
                warn!(
                    host = %self.target_host,
                    "INSECURE: Accepting host key without verification"
                );
                return Ok(true);
            }
            HostKeyVerification::Tofu
            | HostKeyVerification::Strict
            | HostKeyVerification::AcceptNew => {
                // Continue with verification below
            }
        }

        // Check known hosts
        let check_result =
            self.known_hosts
                .check(&self.target_host, self.target_port, server_public_key);

        match check_result {
            HostKeyCheckResult::Match => {
                debug!(
                    host = %self.target_host,
                    "Host key verified - matches known host"
                );
                Ok(true)
            }
            HostKeyCheckResult::Mismatch { expected, actual } => {
                // Key has changed - this is a potential MITM attack
                warn!(
                    host = %self.target_host,
                    expected = %expected,
                    actual = %actual,
                    "HOST KEY MISMATCH - possible MITM attack!"
                );

                // All modes reject changed keys
                Err(anyhow::anyhow!(
                    "Host key verification failed for '{}': key has changed!\n\
                     Expected: {}\n\
                     Received: {}\n\
                     This could indicate a man-in-the-middle attack.\n\
                     If the server key was legitimately changed, remove the old key with:\n\
                     omniedge ssh-keygen -R {}",
                    self.target_host,
                    expected,
                    actual,
                    self.target_host
                ))
            }
            HostKeyCheckResult::NotFound => {
                // New host - behavior depends on mode
                match self.host_key_mode {
                    HostKeyVerification::Strict => {
                        // Strict mode rejects unknown hosts
                        warn!(
                            host = %self.target_host,
                            "Host not in known hosts (strict mode)"
                        );
                        Err(anyhow::anyhow!(
                            "Host key verification failed: '{}' not in known hosts.\n\
                             To add this host, connect with TOFU mode first or manually add the key.",
                            self.target_host
                        ))
                    }
                    HostKeyVerification::Tofu | HostKeyVerification::AcceptNew => {
                        // TOFU and AcceptNew modes accept and remember new keys
                        info!(
                            host = %self.target_host,
                            key_type = %key_type,
                            fingerprint = %fingerprint,
                            "New host - adding to known hosts (TOFU)"
                        );

                        // Add to known hosts
                        if let Err(e) = self.known_hosts.add(
                            &self.target_host,
                            self.target_port,
                            server_public_key,
                        ) {
                            warn!(
                                host = %self.target_host,
                                error = %e,
                                "Failed to save host key to known hosts"
                            );
                            // Continue anyway - verification passed
                        }

                        Ok(true)
                    }
                    HostKeyVerification::Insecure => unreachable!(),
                }
            }
        }
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
