//! Port forwarding for SSH sessions
//!
//! This module provides local (-L), remote (-R), and dynamic (-D) port forwarding
//! capabilities for SSH connections.
//!
//! ## Local Port Forwarding (-L)
//!
//! Forward a local port through the SSH tunnel to a remote destination:
//! ```text
//! Client -> [local:8080] -> SSH Tunnel -> [remote:80]
//! ```
//!
//! ## Remote Port Forwarding (-R)
//!
//! Forward connections from a remote port back through the SSH tunnel:
//! ```text
//! Remote -> [remote:9000] -> SSH Tunnel -> [local:3000]
//! ```
//!
//! ## Dynamic Port Forwarding (-D)
//!
//! Create a SOCKS proxy that forwards connections through the SSH tunnel:
//! ```text
//! Client -> [local:1080/SOCKS] -> SSH Tunnel -> [target]
//! ```

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{oneshot, Mutex};
use tracing::{debug, error, info, warn};

/// Local port forwarding configuration (-L)
#[derive(Debug, Clone)]
pub struct LocalForward {
    /// Local address to bind
    pub bind_addr: SocketAddr,
    /// Remote host to connect to
    pub remote_host: String,
    /// Remote port to connect to
    pub remote_port: u16,
}

impl LocalForward {
    /// Create a new local forward
    pub fn new(bind_addr: SocketAddr, remote_host: String, remote_port: u16) -> Self {
        Self {
            bind_addr,
            remote_host,
            remote_port,
        }
    }

    /// Parse from -L format: [bind_address:]port:host:hostport
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        let parts: Vec<&str> = s.split(':').collect();

        match parts.len() {
            3 => {
                // port:host:hostport
                let local_port: u16 = parts[0].parse()?;
                let remote_host = parts[1].to_string();
                let remote_port: u16 = parts[2].parse()?;

                Ok(Self {
                    bind_addr: SocketAddr::from(([127, 0, 0, 1], local_port)),
                    remote_host,
                    remote_port,
                })
            }
            4 => {
                // bind_address:port:host:hostport
                let bind_addr: std::net::IpAddr = parts[0].parse()?;
                let local_port: u16 = parts[1].parse()?;
                let remote_host = parts[2].to_string();
                let remote_port: u16 = parts[3].parse()?;

                Ok(Self {
                    bind_addr: SocketAddr::new(bind_addr, local_port),
                    remote_host,
                    remote_port,
                })
            }
            _ => Err(anyhow::anyhow!("Invalid local forward format: {}", s)),
        }
    }
}

/// Remote port forwarding configuration (-R)
#[derive(Debug, Clone)]
pub struct RemoteForward {
    /// Remote address to bind on server
    pub bind_addr: SocketAddr,
    /// Local host to forward to
    pub local_host: String,
    /// Local port to forward to
    pub local_port: u16,
}

impl RemoteForward {
    /// Create a new remote forward
    pub fn new(bind_addr: SocketAddr, local_host: String, local_port: u16) -> Self {
        Self {
            bind_addr,
            local_host,
            local_port,
        }
    }

    /// Parse from -R format: [bind_address:]port:host:hostport
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        let parts: Vec<&str> = s.split(':').collect();

        match parts.len() {
            3 => {
                // port:host:hostport
                let remote_port: u16 = parts[0].parse()?;
                let local_host = parts[1].to_string();
                let local_port: u16 = parts[2].parse()?;

                Ok(Self {
                    bind_addr: SocketAddr::from(([0, 0, 0, 0], remote_port)),
                    local_host,
                    local_port,
                })
            }
            4 => {
                // bind_address:port:host:hostport
                let bind_addr: std::net::IpAddr = parts[0].parse()?;
                let remote_port: u16 = parts[1].parse()?;
                let local_host = parts[2].to_string();
                let local_port: u16 = parts[3].parse()?;

                Ok(Self {
                    bind_addr: SocketAddr::new(bind_addr, remote_port),
                    local_host,
                    local_port,
                })
            }
            _ => Err(anyhow::anyhow!("Invalid remote forward format: {}", s)),
        }
    }
}

/// Dynamic port forwarding (SOCKS proxy) configuration (-D)
#[derive(Debug, Clone)]
pub struct DynamicForward {
    /// Local address to bind SOCKS proxy
    pub bind_addr: SocketAddr,
}

impl DynamicForward {
    /// Create a new dynamic forward
    pub fn new(bind_addr: SocketAddr) -> Self {
        Self { bind_addr }
    }

    /// Parse from -D format: [bind_address:]port
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        if let Some(pos) = s.rfind(':') {
            let bind_addr: std::net::IpAddr = s[..pos].parse()?;
            let port: u16 = s[pos + 1..].parse()?;
            Ok(Self {
                bind_addr: SocketAddr::new(bind_addr, port),
            })
        } else {
            let port: u16 = s.parse()?;
            Ok(Self {
                bind_addr: SocketAddr::from(([127, 0, 0, 1], port)),
            })
        }
    }
}

/// Handle for an active local port forward
pub struct LocalForwardHandle {
    id: u64,
    config: LocalForward,
    cancel_tx: Option<oneshot::Sender<()>>,
}

impl LocalForwardHandle {
    /// Get the forward ID
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Get the forward configuration
    pub fn config(&self) -> &LocalForward {
        &self.config
    }

    /// Stop the port forward
    pub fn cancel(mut self) {
        if let Some(tx) = self.cancel_tx.take() {
            let _ = tx.send(());
        }
    }
}

impl Drop for LocalForwardHandle {
    fn drop(&mut self) {
        if let Some(tx) = self.cancel_tx.take() {
            let _ = tx.send(());
        }
    }
}

/// Handle for an active remote port forward
pub struct RemoteForwardHandle {
    id: u64,
    config: RemoteForward,
    cancel_tx: Option<oneshot::Sender<()>>,
}

impl RemoteForwardHandle {
    /// Get the forward ID
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Get the forward configuration
    pub fn config(&self) -> &RemoteForward {
        &self.config
    }

    /// Stop the port forward
    pub fn cancel(mut self) {
        if let Some(tx) = self.cancel_tx.take() {
            let _ = tx.send(());
        }
    }
}

impl Drop for RemoteForwardHandle {
    fn drop(&mut self) {
        if let Some(tx) = self.cancel_tx.take() {
            let _ = tx.send(());
        }
    }
}

/// Handle for an active dynamic (SOCKS) port forward
pub struct DynamicForwardHandle {
    id: u64,
    config: DynamicForward,
    cancel_tx: Option<oneshot::Sender<()>>,
}

impl DynamicForwardHandle {
    /// Get the forward ID
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Get the forward configuration
    pub fn config(&self) -> &DynamicForward {
        &self.config
    }

    /// Stop the port forward
    pub fn cancel(mut self) {
        if let Some(tx) = self.cancel_tx.take() {
            let _ = tx.send(());
        }
    }
}

impl Drop for DynamicForwardHandle {
    fn drop(&mut self) {
        if let Some(tx) = self.cancel_tx.take() {
            let _ = tx.send(());
        }
    }
}

/// Statistics for a port forward
#[derive(Debug, Clone, Default)]
pub struct ForwardStats {
    /// Total bytes sent
    pub bytes_sent: u64,
    /// Total bytes received
    pub bytes_received: u64,
    /// Total connections handled
    pub connections: u64,
    /// Active connections
    pub active_connections: u64,
}

/// Trait for creating SSH tunnels (to be implemented by SSH client)
#[async_trait::async_trait]
pub trait TunnelProvider: Send + Sync {
    /// Open a direct-tcpip channel to the remote host:port
    /// Returns a bidirectional stream
    async fn open_direct_tcpip(
        &self,
        remote_host: &str,
        remote_port: u16,
        originator_addr: &str,
        originator_port: u16,
    ) -> anyhow::Result<Box<dyn TunnelStream>>;

    /// Request remote port forwarding
    async fn tcpip_forward(&self, bind_addr: &str, bind_port: u16) -> anyhow::Result<u16>;

    /// Cancel remote port forwarding
    async fn cancel_tcpip_forward(&self, bind_addr: &str, bind_port: u16) -> anyhow::Result<()>;
}

/// Bidirectional stream for tunnel data
#[async_trait::async_trait]
pub trait TunnelStream: Send + Sync {
    /// Read data from the tunnel
    async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize>;

    /// Write data to the tunnel
    async fn write(&mut self, buf: &[u8]) -> std::io::Result<usize>;

    /// Flush the tunnel
    async fn flush(&mut self) -> std::io::Result<()>;

    /// Shutdown the tunnel
    async fn shutdown(&mut self) -> std::io::Result<()>;
}

/// Port forwarder manager
pub struct Forwarder {
    /// Tunnel provider for SSH connections
    tunnel_provider: Option<Arc<dyn TunnelProvider>>,
    /// Next forward ID
    next_id: AtomicU64,
    /// Active local forward handles
    local_handles: Arc<Mutex<HashMap<u64, LocalForward>>>,
    /// Active remote forward handles
    remote_handles: Arc<Mutex<HashMap<u64, RemoteForward>>>,
    /// Active dynamic forward handles
    dynamic_handles: Arc<Mutex<HashMap<u64, DynamicForward>>>,
    /// Statistics per forward
    stats: Arc<Mutex<HashMap<u64, ForwardStats>>>,
}

impl Forwarder {
    /// Create a new forwarder without a tunnel provider
    pub fn new() -> Self {
        Self {
            tunnel_provider: None,
            next_id: AtomicU64::new(1),
            local_handles: Arc::new(Mutex::new(HashMap::new())),
            remote_handles: Arc::new(Mutex::new(HashMap::new())),
            dynamic_handles: Arc::new(Mutex::new(HashMap::new())),
            stats: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a forwarder with a tunnel provider
    pub fn with_tunnel_provider(provider: Arc<dyn TunnelProvider>) -> Self {
        Self {
            tunnel_provider: Some(provider),
            next_id: AtomicU64::new(1),
            local_handles: Arc::new(Mutex::new(HashMap::new())),
            remote_handles: Arc::new(Mutex::new(HashMap::new())),
            dynamic_handles: Arc::new(Mutex::new(HashMap::new())),
            stats: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Set the tunnel provider
    pub fn set_tunnel_provider(&mut self, provider: Arc<dyn TunnelProvider>) {
        self.tunnel_provider = Some(provider);
    }

    /// Start a local port forward (-L)
    pub async fn start_local(&self, config: LocalForward) -> anyhow::Result<LocalForwardHandle> {
        let tunnel_provider = self
            .tunnel_provider
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No tunnel provider configured"))?;

        let listener = TcpListener::bind(config.bind_addr).await?;
        let actual_addr = listener.local_addr()?;

        info!(
            "Local forward started: {} -> {}:{}",
            actual_addr, config.remote_host, config.remote_port
        );

        let (cancel_tx, mut cancel_rx) = oneshot::channel();
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);

        let remote_host = config.remote_host.clone();
        let remote_port = config.remote_port;
        let stats = self.stats.clone();
        let local_handles = self.local_handles.clone();

        // Store the config
        {
            let mut handles = local_handles.lock().await;
            handles.insert(id, config.clone());
        }
        {
            let mut s = stats.lock().await;
            s.insert(id, ForwardStats::default());
        }

        // Spawn the listener task
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        match result {
                            Ok((stream, peer_addr)) => {
                                debug!("Local forward connection from {}", peer_addr);

                                // Update stats
                                {
                                    let mut s = stats.lock().await;
                                    if let Some(st) = s.get_mut(&id) {
                                        st.connections += 1;
                                        st.active_connections += 1;
                                    }
                                }

                                let tunnel_provider = tunnel_provider.clone();
                                let remote_host = remote_host.clone();
                                let stats = stats.clone();

                                // Handle the connection in a separate task
                                tokio::spawn(async move {
                                    if let Err(e) = handle_local_forward_connection(
                                        stream,
                                        peer_addr,
                                        &tunnel_provider,
                                        &remote_host,
                                        remote_port,
                                        id,
                                        stats.clone(),
                                    )
                                    .await
                                    {
                                        warn!("Local forward connection error: {}", e);
                                    }

                                    // Update active connections
                                    let mut s = stats.lock().await;
                                    if let Some(st) = s.get_mut(&id) {
                                        st.active_connections = st.active_connections.saturating_sub(1);
                                    }
                                });
                            }
                            Err(e) => {
                                error!("Local forward accept error: {}", e);
                                // Brief pause before retrying
                                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                            }
                        }
                    }
                    _ = &mut cancel_rx => {
                        info!("Local forward {} cancelled", id);
                        break;
                    }
                }
            }

            // Clean up
            let mut handles = local_handles.lock().await;
            handles.remove(&id);
        });

        Ok(LocalForwardHandle {
            id,
            config,
            cancel_tx: Some(cancel_tx),
        })
    }

    /// Start a remote port forward (-R)
    pub async fn start_remote(&self, config: RemoteForward) -> anyhow::Result<RemoteForwardHandle> {
        let tunnel_provider = self
            .tunnel_provider
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No tunnel provider configured"))?;

        // Request the remote side to bind the port
        let bind_addr_str = config.bind_addr.ip().to_string();
        let actual_port = tunnel_provider
            .tcpip_forward(&bind_addr_str, config.bind_addr.port())
            .await?;

        info!(
            "Remote forward started: {}:{} -> {}:{}",
            bind_addr_str, actual_port, config.local_host, config.local_port
        );

        let (cancel_tx, cancel_rx) = oneshot::channel();
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);

        let _local_host = config.local_host.clone();
        let _local_port = config.local_port;
        let stats = self.stats.clone();
        let remote_handles = self.remote_handles.clone();

        // Store the config
        {
            let mut handles = remote_handles.lock().await;
            handles.insert(id, config.clone());
        }
        {
            let mut s = stats.lock().await;
            s.insert(id, ForwardStats::default());
        }

        // Spawn a task to wait for cancellation
        // Note: Actual incoming connections are handled by the SSH server's
        // forwarded-tcpip channel callbacks
        let bind_addr_cancel = bind_addr_str.clone();
        let bind_port_cancel = config.bind_addr.port();
        let tunnel_provider_cancel = tunnel_provider.clone();

        tokio::spawn(async move {
            // Wait for cancellation
            let _ = cancel_rx.await;

            // Cancel the remote forward
            info!("Remote forward {} cancelling", id);
            if let Err(e) = tunnel_provider_cancel
                .cancel_tcpip_forward(&bind_addr_cancel, bind_port_cancel)
                .await
            {
                warn!("Failed to cancel remote forward: {}", e);
            }

            // Clean up
            let mut handles = remote_handles.lock().await;
            handles.remove(&id);
        });

        Ok(RemoteForwardHandle {
            id,
            config,
            cancel_tx: Some(cancel_tx),
        })
    }

    /// Start a dynamic (SOCKS) port forward (-D)
    pub async fn start_dynamic(
        &self,
        config: DynamicForward,
    ) -> anyhow::Result<DynamicForwardHandle> {
        let tunnel_provider = self
            .tunnel_provider
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No tunnel provider configured"))?;

        let listener = TcpListener::bind(config.bind_addr).await?;
        let actual_addr = listener.local_addr()?;

        info!("SOCKS proxy started on {}", actual_addr);

        let (cancel_tx, mut cancel_rx) = oneshot::channel();
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);

        let stats = self.stats.clone();
        let dynamic_handles = self.dynamic_handles.clone();

        // Store the config
        {
            let mut handles = dynamic_handles.lock().await;
            handles.insert(id, config.clone());
        }
        {
            let mut s = stats.lock().await;
            s.insert(id, ForwardStats::default());
        }

        // Spawn the SOCKS proxy listener task
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        match result {
                            Ok((stream, peer_addr)) => {
                                debug!("SOCKS connection from {}", peer_addr);

                                // Update stats
                                {
                                    let mut s = stats.lock().await;
                                    if let Some(st) = s.get_mut(&id) {
                                        st.connections += 1;
                                        st.active_connections += 1;
                                    }
                                }

                                let tunnel_provider = tunnel_provider.clone();
                                let stats = stats.clone();

                                // Handle the SOCKS connection
                                tokio::spawn(async move {
                                    if let Err(e) = handle_socks_connection(
                                        stream,
                                        peer_addr,
                                        &tunnel_provider,
                                        id,
                                        stats.clone(),
                                    )
                                    .await
                                    {
                                        debug!("SOCKS connection error: {}", e);
                                    }

                                    // Update active connections
                                    let mut s = stats.lock().await;
                                    if let Some(st) = s.get_mut(&id) {
                                        st.active_connections = st.active_connections.saturating_sub(1);
                                    }
                                });
                            }
                            Err(e) => {
                                error!("SOCKS accept error: {}", e);
                                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                            }
                        }
                    }
                    _ = &mut cancel_rx => {
                        info!("SOCKS proxy {} cancelled", id);
                        break;
                    }
                }
            }

            // Clean up
            let mut handles = dynamic_handles.lock().await;
            handles.remove(&id);
        });

        Ok(DynamicForwardHandle {
            id,
            config,
            cancel_tx: Some(cancel_tx),
        })
    }

    /// Get statistics for a forward
    pub async fn get_stats(&self, id: u64) -> Option<ForwardStats> {
        let stats = self.stats.lock().await;
        stats.get(&id).cloned()
    }

    /// List all active local forwards
    pub async fn list_local(&self) -> Vec<(u64, LocalForward)> {
        let handles = self.local_handles.lock().await;
        handles.iter().map(|(k, v)| (*k, v.clone())).collect()
    }

    /// List all active remote forwards
    pub async fn list_remote(&self) -> Vec<(u64, RemoteForward)> {
        let handles = self.remote_handles.lock().await;
        handles.iter().map(|(k, v)| (*k, v.clone())).collect()
    }

    /// List all active dynamic forwards
    pub async fn list_dynamic(&self) -> Vec<(u64, DynamicForward)> {
        let handles = self.dynamic_handles.lock().await;
        handles.iter().map(|(k, v)| (*k, v.clone())).collect()
    }
}

impl Default for Forwarder {
    fn default() -> Self {
        Self::new()
    }
}

/// Handle a local forward connection
async fn handle_local_forward_connection(
    mut local_stream: TcpStream,
    peer_addr: SocketAddr,
    tunnel_provider: &Arc<dyn TunnelProvider>,
    remote_host: &str,
    remote_port: u16,
    forward_id: u64,
    stats: Arc<Mutex<HashMap<u64, ForwardStats>>>,
) -> anyhow::Result<()> {
    // Open tunnel to remote
    let mut tunnel = tunnel_provider
        .open_direct_tcpip(
            remote_host,
            remote_port,
            &peer_addr.ip().to_string(),
            peer_addr.port(),
        )
        .await?;

    // Bidirectional copy
    let mut local_buf = vec![0u8; 32768];
    let mut tunnel_buf = vec![0u8; 32768];

    loop {
        tokio::select! {
            // Local -> Tunnel
            result = local_stream.read(&mut local_buf) => {
                match result {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        tunnel.write(&local_buf[..n]).await?;
                        tunnel.flush().await?;

                        // Update stats
                        let mut s = stats.lock().await;
                        if let Some(st) = s.get_mut(&forward_id) {
                            st.bytes_sent += n as u64;
                        }
                    }
                    Err(e) => {
                        return Err(e.into());
                    }
                }
            }
            // Tunnel -> Local
            result = tunnel.read(&mut tunnel_buf) => {
                match result {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        local_stream.write_all(&tunnel_buf[..n]).await?;

                        // Update stats
                        let mut s = stats.lock().await;
                        if let Some(st) = s.get_mut(&forward_id) {
                            st.bytes_received += n as u64;
                        }
                    }
                    Err(e) => {
                        return Err(e.into());
                    }
                }
            }
        }
    }

    // Clean shutdown
    let _ = tunnel.shutdown().await;
    let _ = local_stream.shutdown().await;

    Ok(())
}

/// Handle a SOCKS5 connection
async fn handle_socks_connection(
    mut stream: TcpStream,
    peer_addr: SocketAddr,
    tunnel_provider: &Arc<dyn TunnelProvider>,
    forward_id: u64,
    stats: Arc<Mutex<HashMap<u64, ForwardStats>>>,
) -> anyhow::Result<()> {
    // SOCKS5 handshake
    // Read version and auth methods
    let mut buf = [0u8; 258];
    let n = stream.read(&mut buf[..2]).await?;
    if n < 2 {
        return Err(anyhow::anyhow!("SOCKS handshake too short"));
    }

    let version = buf[0];
    if version != 0x05 {
        return Err(anyhow::anyhow!("Unsupported SOCKS version: {}", version));
    }

    let nmethods = buf[1] as usize;
    if nmethods > 0 {
        stream.read_exact(&mut buf[..nmethods]).await?;
    }

    // We only support no-auth (0x00)
    // Send response: version 5, no auth required
    stream.write_all(&[0x05, 0x00]).await?;

    // Read connection request
    let mut request = [0u8; 4];
    stream.read_exact(&mut request).await?;

    if request[0] != 0x05 {
        return Err(anyhow::anyhow!("Invalid SOCKS version in request"));
    }

    let cmd = request[1];
    if cmd != 0x01 {
        // Only support CONNECT
        // Send failure response
        stream
            .write_all(&[0x05, 0x07, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
            .await?;
        return Err(anyhow::anyhow!("Unsupported SOCKS command: {}", cmd));
    }

    // Parse address
    let atyp = request[3];
    let (dest_host, dest_port) = match atyp {
        0x01 => {
            // IPv4
            let mut addr = [0u8; 4];
            stream.read_exact(&mut addr).await?;
            let mut port_buf = [0u8; 2];
            stream.read_exact(&mut port_buf).await?;
            let port = u16::from_be_bytes(port_buf);
            let ip = std::net::Ipv4Addr::from(addr);
            (ip.to_string(), port)
        }
        0x03 => {
            // Domain name
            let mut len_buf = [0u8; 1];
            stream.read_exact(&mut len_buf).await?;
            let len = len_buf[0] as usize;
            let mut domain = vec![0u8; len];
            stream.read_exact(&mut domain).await?;
            let mut port_buf = [0u8; 2];
            stream.read_exact(&mut port_buf).await?;
            let port = u16::from_be_bytes(port_buf);
            (String::from_utf8(domain)?, port)
        }
        0x04 => {
            // IPv6
            let mut addr = [0u8; 16];
            stream.read_exact(&mut addr).await?;
            let mut port_buf = [0u8; 2];
            stream.read_exact(&mut port_buf).await?;
            let port = u16::from_be_bytes(port_buf);
            let ip = std::net::Ipv6Addr::from(addr);
            (ip.to_string(), port)
        }
        _ => {
            stream
                .write_all(&[0x05, 0x08, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await?;
            return Err(anyhow::anyhow!("Unsupported SOCKS address type: {}", atyp));
        }
    };

    debug!("SOCKS connect to {}:{}", dest_host, dest_port);

    // Open tunnel to destination
    let tunnel_result = tunnel_provider
        .open_direct_tcpip(
            &dest_host,
            dest_port,
            &peer_addr.ip().to_string(),
            peer_addr.port(),
        )
        .await;

    match tunnel_result {
        Ok(mut tunnel) => {
            // Send success response
            // Reply: version, success, reserved, address type, bound address, bound port
            stream
                .write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await?;

            // Bidirectional copy
            let mut local_buf = vec![0u8; 32768];
            let mut tunnel_buf = vec![0u8; 32768];

            loop {
                tokio::select! {
                    // Local -> Tunnel
                    result = stream.read(&mut local_buf) => {
                        match result {
                            Ok(0) => break,
                            Ok(n) => {
                                tunnel.write(&local_buf[..n]).await?;
                                tunnel.flush().await?;

                                let mut s = stats.lock().await;
                                if let Some(st) = s.get_mut(&forward_id) {
                                    st.bytes_sent += n as u64;
                                }
                            }
                            Err(e) => return Err(e.into()),
                        }
                    }
                    // Tunnel -> Local
                    result = tunnel.read(&mut tunnel_buf) => {
                        match result {
                            Ok(0) => break,
                            Ok(n) => {
                                stream.write_all(&tunnel_buf[..n]).await?;

                                let mut s = stats.lock().await;
                                if let Some(st) = s.get_mut(&forward_id) {
                                    st.bytes_received += n as u64;
                                }
                            }
                            Err(e) => return Err(e.into()),
                        }
                    }
                }
            }

            let _ = tunnel.shutdown().await;
        }
        Err(e) => {
            // Send connection refused
            stream
                .write_all(&[0x05, 0x05, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await?;
            return Err(e);
        }
    }

    let _ = stream.shutdown().await;
    Ok(())
}

/// Handle a remote forward incoming connection (called by SSH server)
pub async fn handle_remote_forward_incoming(
    mut tunnel: Box<dyn TunnelStream>,
    local_host: &str,
    local_port: u16,
    forward_id: u64,
    stats: Arc<Mutex<HashMap<u64, ForwardStats>>>,
) -> anyhow::Result<()> {
    // Connect to local destination
    let addr = format!("{}:{}", local_host, local_port);
    let mut local_stream = TcpStream::connect(&addr).await?;

    // Bidirectional copy
    let mut local_buf = vec![0u8; 32768];
    let mut tunnel_buf = vec![0u8; 32768];

    loop {
        tokio::select! {
            // Local -> Tunnel (send back to remote)
            result = local_stream.read(&mut local_buf) => {
                match result {
                    Ok(0) => break,
                    Ok(n) => {
                        tunnel.write(&local_buf[..n]).await?;
                        tunnel.flush().await?;

                        let mut s = stats.lock().await;
                        if let Some(st) = s.get_mut(&forward_id) {
                            st.bytes_sent += n as u64;
                        }
                    }
                    Err(e) => return Err(e.into()),
                }
            }
            // Tunnel -> Local (from remote)
            result = tunnel.read(&mut tunnel_buf) => {
                match result {
                    Ok(0) => break,
                    Ok(n) => {
                        local_stream.write_all(&tunnel_buf[..n]).await?;

                        let mut s = stats.lock().await;
                        if let Some(st) = s.get_mut(&forward_id) {
                            st.bytes_received += n as u64;
                        }
                    }
                    Err(e) => return Err(e.into()),
                }
            }
        }
    }

    let _ = tunnel.shutdown().await;
    let _ = local_stream.shutdown().await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_forward_parse() {
        let fwd = LocalForward::parse("8080:localhost:80").unwrap();
        assert_eq!(fwd.bind_addr.port(), 8080);
        assert_eq!(fwd.remote_host, "localhost");
        assert_eq!(fwd.remote_port, 80);

        let fwd = LocalForward::parse("0.0.0.0:8080:db.internal:5432").unwrap();
        assert_eq!(fwd.bind_addr.to_string(), "0.0.0.0:8080");
        assert_eq!(fwd.remote_host, "db.internal");
        assert_eq!(fwd.remote_port, 5432);
    }

    #[test]
    fn test_remote_forward_parse() {
        let fwd = RemoteForward::parse("9000:localhost:3000").unwrap();
        assert_eq!(fwd.bind_addr.port(), 9000);
        assert_eq!(fwd.local_host, "localhost");
        assert_eq!(fwd.local_port, 3000);
    }

    #[test]
    fn test_dynamic_forward_parse() {
        let fwd = DynamicForward::parse("1080").unwrap();
        assert_eq!(fwd.bind_addr.to_string(), "127.0.0.1:1080");

        let fwd = DynamicForward::parse("0.0.0.0:1080").unwrap();
        assert_eq!(fwd.bind_addr.to_string(), "0.0.0.0:1080");
    }
}
