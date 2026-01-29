use crate::config::CliConfig;
use crate::state::ConnectionState;
use anyhow::{Context, Result};
use log::{debug, error, info, warn};
use omni_api::{types::*, ApiClient, AuthService, DeviceService, NetworkService};
use omni_proto::{handle_nucleus_message, NucleusState, OmniProto};
use omni_tun::OmniTun;
use omninervous::Identity;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};

pub struct ConnectionManager {
    state: Arc<RwLock<ConnectionState>>,
    api_client: Option<ApiClient>,
    proto: Option<Arc<OmniProto>>,
    tun: Option<OmniTun>,
    identity: Identity,
    base_url: String,
    is_nucleus: bool,
    nucleus_state: Option<Arc<Mutex<NucleusState>>>,
    nucleus_port: u16,
    as_exit_node: Arc<AtomicBool>,
    exit_node_ip: Option<String>,
    cluster_secret: Option<String>,
    device_id: Option<String>,
    current_network_id: Arc<RwLock<Option<String>>>,
    virtual_ip: Arc<RwLock<Option<String>>>,
    heartbeat_tx: Option<mpsc::Sender<()>>,
    shutdown_tx: Option<broadcast::Sender<()>>,
}

impl ConnectionManager {
    pub fn new(base_url: String, private_key: Option<[u8; 32]>) -> Self {
        let identity = if let Some(pk) = private_key {
            Identity::from_private_key(pk)
        } else {
            Identity::generate()
        };

        Self {
            state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            api_client: None,
            proto: None,
            tun: None,
            identity,
            base_url,
            is_nucleus: false,
            nucleus_state: None,
            nucleus_port: 51820, // Default nucleus signaling port
            as_exit_node: Arc::new(AtomicBool::new(
                CliConfig::load().map(|c| c.is_exit_node).unwrap_or(false),
            )),
            exit_node_ip: None,
            cluster_secret: None,
            device_id: None,
            current_network_id: Arc::new(RwLock::new(None)),
            virtual_ip: Arc::new(RwLock::new(None)),
            heartbeat_tx: None,
            shutdown_tx: None,
        }
    }

    pub async fn get_state(&self) -> ConnectionState {
        self.state.read().await.clone()
    }

    pub fn get_state_handle(&self) -> Arc<RwLock<ConnectionState>> {
        self.state.clone()
    }

    pub fn get_network_id_handle(&self) -> Arc<RwLock<Option<String>>> {
        self.current_network_id.clone()
    }

    pub fn get_virtual_ip_handle(&self) -> Arc<RwLock<Option<String>>> {
        self.virtual_ip.clone()
    }

    pub fn get_as_exit_node_handle(&self) -> Arc<AtomicBool> {
        self.as_exit_node.clone()
    }

    pub async fn sync_state(
        &mut self,
        state: ConnectionState,
        network_id: Option<String>,
        virtual_ip: Option<String>,
    ) {
        self.set_state(state).await;
        let mut nid = self.current_network_id.write().await;
        *nid = network_id;
        let mut vip = self.virtual_ip.write().await;
        *vip = virtual_ip;
    }

    async fn set_state(&self, new_state: ConnectionState) {
        let mut state = self.state.write().await;
        info!(
            "Connection state transition: {:?} -> {:?}",
            *state, new_state
        );
        *state = new_state;
    }

    pub async fn try_auto_login(&mut self) -> Result<bool> {
        info!("Attempting auto-login...");
        let config = crate::config::CliConfig::load()?;
        if let Some(auth) = config.auth_response.clone() {
            // Try using the old token directly
            self.api_client = Some(ApiClient::new(
                self.base_url.clone(),
                Some(auth.effective_token().to_string()),
            ));
            if let Ok(_profile) = self.get_profile().await {
                self.set_state(ConnectionState::Authenticated).await;
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            Ok(false)
        }
    }

    pub async fn connect_with_token(
        &mut self,
        token: String,
        network_id: &str,
        device_id: &str,
        hardware_id: &str,
        is_nucleus: bool,
        as_exit_node: bool,
        exit_node_ip: Option<String>,
    ) -> Result<()> {
        self.set_state(ConnectionState::Authenticated).await;
        self.is_nucleus = is_nucleus;
        self.as_exit_node.store(as_exit_node, Ordering::SeqCst);
        self.exit_node_ip = exit_node_ip;

        // Initialize nucleus state if running in nucleus mode
        if is_nucleus {
            info!("Initializing Nucleus signaling server state...");
            self.nucleus_state = Some(Arc::new(Mutex::new(NucleusState::new())));
        }

        let client = ApiClient::new(self.base_url.clone(), Some(token));
        self.api_client = Some(client);

        match self.perform_join(network_id, device_id, hardware_id).await {
            Ok(_) => Ok(()),
            Err(e) => {
                // If join fails, reset state back to Authenticated so we can try again
                self.set_state(ConnectionState::Authenticated).await;
                Err(e)
            }
        }
    }

    pub async fn perform_join(
        &mut self,
        network_id: &str,
        device_id: &str,
        hardware_id: &str,
    ) -> Result<()> {
        info!(
            "Starting perform_join for network: {}, device_id: {}, hardware_id: {}",
            network_id, device_id, hardware_id
        );
        info!("Using API base URL: {}", self.base_url);
        self.set_state(ConnectionState::Joining).await;
        self.device_id = Some(device_id.to_string());
        {
            let mut nid = self.current_network_id.write().await;
            *nid = Some(network_id.to_string());
        }

        let _ = self.cleanup_adapters();
        // Give the OS time to fully release WinTun resources
        tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;

        let client = self.api_client.as_ref().context("Not authenticated")?;
        let dev_service = DeviceService::new(client);
        let net_service = NetworkService::new(client);

        // 0. Register/Update Device
        let os = std::env::consts::OS;
        let hostname =
            ::whoami::fallible::hostname().unwrap_or_else(|_| "OmniEdge Device".to_string());
        info!(
            "Registering/Updating device: {} (OS: {}, hardware_id: {})",
            hostname, os, hardware_id
        );
        let device_resp = dev_service.register(&hostname, hardware_id, os).await;

        let effective_device_id = if let Ok(ref resp) = device_resp {
            info!("Device registered/updated successfully. ID: {}", resp.id);
            // Update self.device_id with the actual device UUID from the API
            self.device_id = Some(resp.id.clone());
            &resp.id
        } else {
            let e = device_resp.as_ref().unwrap_err();
            warn!(
                "Device registration failed: {}. Proceeding with hardware_id: {}",
                e, hardware_id
            );
            hardware_id
        };

        // 1. Join Network
        info!(
            "Attempting to join virtual network: {} as effective_device_id: {}",
            network_id, effective_device_id
        );
        let join_resp = match net_service.join(network_id, effective_device_id).await {
            Ok(resp) => resp,
            Err(e) => {
                let err_msg = format!("Join failed for network {}: {}", network_id, e);
                error!("{}", err_msg);
                self.set_state(ConnectionState::Authenticated).await;
                return Err(anyhow::anyhow!(err_msg));
            }
        };

        info!(
            "Join successful. Received VIP: {}, Cluster: {}",
            join_resp.virtual_ip, join_resp.cluster
        );
        debug!("Full Join response: {:?}", join_resp);

        self.set_state(ConnectionState::Connecting).await;

        // 2. Initialize Proto & Tun
        let vip_addr: std::net::Ipv4Addr = join_resp.virtual_ip.parse()?;
        self.cluster_secret = Some(join_resp.secret_key.clone());
        info!("Initializing OmniProto for VIP: {}", vip_addr);

        let proto = Arc::new(
            OmniProto::new(
                &join_resp.server.host,
                join_resp.cluster,
                join_resp.secret_key,
                vip_addr,
                0,
                self.identity.public_key_bytes(),
            )
            .await?,
        );

        // 2. Setup TUN
        #[allow(unused_assignments)]
        let mut tun_instance: Option<OmniTun> = None;
        let mut port = 51820;

        #[cfg(target_os = "windows")]
        {
            let if_names = ["OmniEdge"];
            let mut setup_success = false;
            let mut last_err = String::new();
            let max_retries = 3;

            for retry in 0..max_retries {
                if retry > 0 {
                    info!("TUN setup retry attempt {} of {}", retry + 1, max_retries);
                    // Run cleanup again before retry
                    let _ = self.cleanup_adapters();
                    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
                }

                for ifname in if_names {
                    debug!("Attempting TUN setup with interface name: {}", ifname);
                    let mut tun = OmniTun::new_userspace(ifname);

                    match tun
                        .setup(
                            &join_resp.virtual_ip,
                            port,
                            &::hex::encode(self.identity.private_key_bytes()),
                        )
                        .await
                    {
                        Ok(_) => {
                            info!("TUN setup completed successfully using name: {}", ifname);
                            setup_success = true;
                            tun_instance = Some(tun);
                            break;
                        }
                        Err(e) => {
                            last_err = e.to_string();
                            warn!("TUN setup failed for name {}: {}", ifname, e);
                        }
                    }
                }

                if setup_success {
                    break;
                }
            }

            if !setup_success {
                let err_msg = format!("Failed to create TUN device after {} attempts. Please ensure you are running OmniEdge as Administrator and no other VPN is conflicting. Error: {}", max_retries, last_err);
                error!("CRITICAL: {}", err_msg);
                return Err(anyhow::anyhow!(err_msg));
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let ifname = "omniedge0";
            info!("Creating Userspace TUN: {}", ifname);
            let mut tun = OmniTun::new_userspace(ifname);
            tun.setup(
                &join_resp.virtual_ip,
                port,
                &::hex::encode(self.identity.private_key_bytes()),
            )
            .await?;
            tun_instance = Some(tun);
        }

        let tun = tun_instance.context("TUN instance not created")?;
        let socket: Arc<UdpSocket> = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);
        port = socket.local_addr()?.port();
        debug!("Bound UDP socket to port: {}", port);

        self.proto = Some(proto.clone());
        self.tun = Some(tun.clone());
        {
            let mut vip = self.virtual_ip.write().await;
            *vip = Some(join_resp.virtual_ip.clone());
        }

        // 4. Start Background Loops
        info!("Starting background loops...");
        let (shutdown_tx, _) = broadcast::channel(1);
        self.shutdown_tx = Some(shutdown_tx.clone());

        let nucleus_state = self.nucleus_state.clone();
        let nucleus_port = self.nucleus_port;
        let is_nucleus = self.is_nucleus;

        self.start_loops(
            socket,
            proto,
            tun,
            effective_device_id.to_string(),
            shutdown_tx,
            nucleus_state,
            nucleus_port,
            is_nucleus,
        )
        .await;

        self.set_state(ConnectionState::Connected).await;

        // 5. Setup Exit Node Routing if requested
        if let Some(ref exit_ip) = self.exit_node_ip {
            info!("Configuring system to use exit node: {}", exit_ip);
            let nucleus_host = &join_resp.server.host;
            if let Err(e) = crate::routing::RoutingManager::setup_exit_node(exit_ip, nucleus_host) {
                error!("Failed to setup exit node routing: {}", e);
            }
        }

        Ok(())
    }

    async fn start_loops(
        &mut self,
        socket: Arc<UdpSocket>,
        proto: Arc<OmniProto>,
        tun: OmniTun,
        device_id: String,
        shutdown_tx: broadcast::Sender<()>,
        nucleus_state: Option<Arc<Mutex<NucleusState>>>,
        nucleus_port: u16,
        is_nucleus: bool,
    ) {
        let (hb_tx, mut hb_rx) = mpsc::channel(1);
        self.heartbeat_tx = Some(hb_tx);
        let mut tun_ctrl = tun.clone();
        let proto_ctrl = proto.clone();
        let socket_inner = socket.clone();
        let secret = self.cluster_secret.clone();

        // Nucleus Signaling Server Loop (only when running in nucleus mode)
        if is_nucleus {
            if let Some(nucleus_state) = nucleus_state.clone() {
                let secret_clone = secret.clone();
                let mut shutdown_rx_nucleus = shutdown_tx.subscribe();

                // Bind nucleus signaling socket on fixed port
                let nucleus_socket =
                    match UdpSocket::bind(format!("0.0.0.0:{}", nucleus_port)).await {
                        Ok(s) => {
                            info!(
                                "Nucleus signaling server listening on UDP port {}",
                                nucleus_port
                            );
                            Arc::new(s)
                        }
                        Err(e) => {
                            error!(
                            "Failed to bind nucleus signaling port {}: {}. Nucleus mode disabled.",
                            nucleus_port, e
                        );
                            // Continue without nucleus mode
                            Arc::new(UdpSocket::bind("0.0.0.0:0").await.unwrap())
                        }
                    };

                tokio::spawn(async move {
                    let mut buf = [0u8; 4096];
                    let mut cleanup_interval =
                        tokio::time::interval(tokio::time::Duration::from_secs(60));

                    loop {
                        tokio::select! {
                            res = nucleus_socket.recv_from(&mut buf) => {
                                match res {
                                    Ok((len, src)) => {
                                        let pkt = &buf[..len];
                                        if pkt.is_empty() || pkt[0] < 0x11 {
                                            continue;
                                        }

                                        // Handle nucleus signaling request
                                        let mut state = nucleus_state.lock().await;
                                        if let Some(response) = handle_nucleus_message(
                                            &mut state,
                                            pkt,
                                            src,
                                            secret_clone.as_deref(),
                                        ) {
                                            if let Err(e) = nucleus_socket.send_to(&response, src).await {
                                                warn!("Failed to send nucleus response to {}: {}", src, e);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        error!("Nucleus socket error: {}", e);
                                    }
                                }
                            }
                            _ = cleanup_interval.tick() => {
                                // Periodic cleanup of stale peers
                                let mut state = nucleus_state.lock().await;
                                state.cleanup();
                                debug!("Nucleus state cleanup complete. {} peers registered.", state.peer_count());
                            }
                            _ = shutdown_rx_nucleus.recv() => {
                                info!("Nucleus Signaling Server shutting down");
                                break;
                            }
                        }
                    }
                });
            }
        }

        // Master Dispatcher Loop
        let mut shutdown_rx1 = shutdown_tx.subscribe();
        tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            loop {
                tokio::select! {
                    res = socket_inner.recv_from(&mut buf) => {
                         match res {
                            Ok((len, src)) => {
                                let pkt = &buf[..len];
                                if pkt.is_empty() {
                                    continue;
                                }

                                let first_byte = pkt[0];

                                if first_byte >= 0x11 {
                                    // Signaling
                                    if let Ok(Some(update)) =
                                        proto_ctrl.handle_packet(pkt, secret.as_deref())
                                    {
                                        for peer in update.peers {
                                            let pubkey = ::hex::encode(peer.public_key);
                                            let allowed_ips = vec![format!("{}/32", peer.vip)];
                                            let _ = tun_ctrl
                                                .add_peer(&pubkey, peer.endpoint, &allowed_ips)
                                                .await;
                                        }
                                    }
                                } else if first_byte >= 0x01 && first_byte <= 0x04 {
                                    // WireGuard
                                    let _ = tun_ctrl.handle_packet(pkt, src, &socket_inner).await;
                                } else {
                                    debug!("Ignored unknown packet type {} from {}", first_byte, src);
                                }
                            }
                            Err(e) => {
                                error!("Master Dispatcher socket error: {}", e);
                            }
                         }
                    }
                    _ = shutdown_rx1.recv() => {
                        info!("Master Dispatcher Loop shutting down");
                        break;
                    }
                }
            }
        });

        // TUN Transmission Loop (TUN -> network) remains necessary for outgoing traffic
        let mut tun_tx = tun.clone();
        let socket_tx = socket.clone();
        let mut shutdown_rx2 = shutdown_tx.subscribe();
        tokio::spawn(async move {
            tokio::select! {
                _ = tun_tx.start_loop(socket_tx) => {}
                _ = shutdown_rx2.recv() => {
                    info!("TUN Transmission Loop shutting down");
                }
            }
        });

        let api_client = self.api_client.as_ref().cloned();
        let proto_hb = proto.clone();
        let socket_hb = socket.clone();
        let is_nucleus_hb = self.is_nucleus;
        let as_exit_node_hb = self.as_exit_node.clone();
        let device_id_hb = device_id.clone();

        // Heartbeat/Poll/Role Loop
        let mut shutdown_rx3 = shutdown_tx.subscribe();
        tokio::spawn(async move {
            let mut api_interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
            let mut proto_interval = tokio::time::interval(tokio::time::Duration::from_secs(30));

            if is_nucleus_hb {
                info!("Running in DUAL MODE: Edge client + Nucleus signaling server active.");
            }

            loop {
                tokio::select! {
                    _ = api_interval.tick() => {
                        if let Some(ref client) = api_client {
                            let ds = DeviceService::new(client);
                            let is_exit = as_exit_node_hb.load(std::sync::atomic::Ordering::SeqCst);
                            let _ = ds.heartbeat(&device_id_hb, is_exit).await;
                        }
                    }
                    _ = hb_rx.recv() => {
                        if let Some(ref client) = api_client {
                            info!("Triggering immediate heartbeat...");
                            let ds = DeviceService::new(client);
                            let is_exit = as_exit_node_hb.load(std::sync::atomic::Ordering::SeqCst);
                            let _ = ds.heartbeat(&device_id_hb, is_exit).await;
                        }
                    }
                    _ = proto_interval.tick() => {
                        let _ = proto_hb.heartbeat(&socket_hb, 0).await;
                    }
                    _ = shutdown_rx3.recv() => {
                        info!("Heartbeat Loop shutting down");
                        break;
                    }
                }
            }
        });
    }

    pub async fn login_with_password(&mut self, email: &str, password: &str) -> Result<AuthResp> {
        self.set_state(ConnectionState::Authenticating).await;
        let client = ApiClient::new(self.base_url.clone(), None);
        let auth = AuthService::new(&client);
        let resp = auth.login_with_password(email, password).await?;

        self.api_client = Some(ApiClient::new(
            self.base_url.clone(),
            Some(resp.effective_token().to_string()),
        ));
        self.set_state(ConnectionState::Authenticated).await;
        Ok(resp)
    }

    pub async fn start_device_flow(&self) -> Result<DeviceCodeResp> {
        let client = ApiClient::new(self.base_url.clone(), None);
        let auth = AuthService::new(&client);
        auth.device_flow_init("omniedge-cli", "openid profile email offline_access")
            .await
    }

    pub async fn poll_device_flow(&mut self, device_code: &str) -> Result<AuthResp> {
        let client = ApiClient::new(self.base_url.clone(), None);
        let auth = AuthService::new(&client);
        let resp = auth.device_flow_token("omniedge-cli", device_code).await?;

        self.api_client = Some(ApiClient::new(
            self.base_url.clone(),
            Some(resp.effective_token().to_string()),
        ));
        self.set_state(ConnectionState::Authenticated).await;
        Ok(resp)
    }

    pub async fn start_session_login(&self) -> Result<SessionResponse> {
        let client = ApiClient::new(self.base_url.clone(), None);
        let auth = AuthService::new(&client);
        auth.generate_session().await
    }

    pub async fn handle_login_token(
        &mut self,
        token_resp: WebSocketTokenResponse,
    ) -> Result<AuthResp> {
        info!("Handling login token from WebSocket...");
        let auth_resp = AuthResp {
            token: token_resp.token.clone(),
            refresh_token: token_resp.refresh_token.clone(),
            access_token: token_resp.token.clone(),
            id_token: "".to_string(),
            expires_in: 3600,
            email: None,
            user_id: None,
        };

        self.api_client = Some(ApiClient::new(
            self.base_url.clone(),
            Some(auth_resp.effective_token().to_string()),
        ));

        // Save to config immediately
        if let Ok(mut config) = CliConfig::load() {
            info!("Saving login tokens to native storage...");
            config.auth_response = Some(auth_resp.clone());
            let _ = config.save();
        }

        self.set_state(ConnectionState::Authenticated).await;
        info!("Authentication successful via session login.");
        Ok(auth_resp)
    }

    pub async fn wait_for_session_login(
        base_url: &str,
        session_id: &str,
    ) -> Result<WebSocketTokenResponse> {
        use futures_util::{SinkExt, StreamExt};
        use tokio::time::{timeout, Duration};
        use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

        let ws_url = if base_url.contains("localhost") || base_url.contains("127.0.0.1") {
            format!("ws://127.0.0.1:8080/auth/login/session/{}", session_id)
        } else {
            let client = ApiClient::new(base_url.to_string(), None);
            client.ws_url(&format!("/auth/login/session/{}", session_id))
        };

        info!(
            "Connecting to WebSocket for session login (ID: {}). URL: {}",
            session_id, ws_url
        );

        use tokio_tungstenite::tungstenite::client::IntoClientRequest;
        let mut request = ws_url.into_client_request()?;
        let headers = request.headers_mut();
        headers.insert("User-Agent", "OmniEdge/2.0.0".parse().unwrap());
        headers.insert("Origin", "https://connect.omniedge.io".parse().unwrap());

        let connect_future = connect_async(request);
        let (ws_stream, _) = timeout(Duration::from_secs(15), connect_future)
            .await
            .context("WebSocket connection timed out during handshake")?
            .context("Failed to connect to login WebSocket")?;

        info!("WebSocket connection established for session login.");

        let (mut write, mut read) = ws_stream.split();

        // Ping loop to keep connection alive
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                if let Err(e) = write.send(Message::Ping(vec![])).await {
                    debug!("WebSocket ping loop stopping: {}", e);
                    break;
                }
            }
        });

        // 2. Wait for message with timeout
        let wait_future = async {
            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        info!("Received WebSocket Text message: {}", text);
                        match serde_json::from_str::<WebSocketTokenResponse>(&text) {
                            Ok(tokens) => {
                                if !tokens.token.is_empty() {
                                    info!("Successfully received tokens via WebSocket.");
                                    return Ok(tokens);
                                } else {
                                    warn!(
                                        "Received WebSocket message but token field was empty: {}",
                                        text
                                    );
                                }
                            }
                            Err(e) => {
                                info!(
                                    "Could not parse WebSocket message as tokens: {} (Raw: {})",
                                    e, text
                                );
                            }
                        }
                    }
                    Ok(Message::Binary(bin)) => {
                        let text = String::from_utf8_lossy(&bin);
                        info!("Received WebSocket Binary message: {}", text);
                        match serde_json::from_str::<WebSocketTokenResponse>(&text) {
                            Ok(tokens) => {
                                if !tokens.token.is_empty() {
                                    info!("Successfully received tokens via WebSocket.");
                                    return Ok(tokens);
                                }
                            }
                            Err(e) => {
                                info!("Could not parse binary WebSocket message as tokens: {}", e);
                            }
                        }
                    }
                    Ok(Message::Close(frame)) => {
                        info!("WebSocket closed by server: {:?}", frame);
                        return Err(anyhow::anyhow!("WebSocket closed by server: {:?}", frame));
                    }
                    Err(e) => {
                        error!("WebSocket error: {}", e);
                        return Err(anyhow::anyhow!("WebSocket error: {}", e));
                    }
                    msg => {
                        info!("Received other WebSocket message: {:?}", msg);
                    }
                }
            }
            Err(anyhow::anyhow!("WebSocket closed without receiving tokens"))
        };

        // 15 minutes timeout to match Go implementation
        let result = timeout(Duration::from_secs(900), wait_future).await;

        match result {
            Ok(res) => res,
            Err(_) => {
                error!(
                    "Login session timed out after 15 minutes for session {}",
                    session_id
                );
                Err(anyhow::anyhow!(
                    "Login session timed out after 15 minutes. Please try again."
                ))
            }
        }
    }

    pub async fn get_networks(&self) -> Result<Vec<VirtualNetworkResponse>> {
        let client = self.api_client.as_ref().context("Not authenticated")?;
        let net_service = NetworkService::new(client);
        net_service.list_all().await
    }

    pub async fn get_profile(&self) -> Result<ProfileResponse> {
        let client = self.api_client.as_ref().context("Not authenticated")?;
        let auth_service = AuthService::new(client);
        auth_service.me().await
    }

    pub async fn get_network_devices(
        &self,
        network_id: &str,
    ) -> Result<Vec<VirtualNetworkDeviceResponse>> {
        let client = self.api_client.as_ref().context("Not authenticated")?;
        let net_service = NetworkService::new(client);
        net_service.get_devices(network_id).await
    }

    pub async fn set_exit_node(
        &mut self,
        network_id: &str,
        exit_node_id: &str,
        exit_node_ip: Option<&str>,
    ) -> Result<()> {
        let client = self.api_client.as_ref().context("Not authenticated")?;
        let net_service = NetworkService::new(client);
        let device_id = self.device_id.as_deref().context("Device ID not set")?;

        let node_id = if exit_node_id.is_empty() {
            None
        } else {
            Some(exit_node_id)
        };
        net_service
            .select_exit_node(network_id, device_id, node_id)
            .await?;

        // Update local state
        self.exit_node_ip = exit_node_ip.map(|s| s.to_string());

        // Refresh routing if connected
        if let ConnectionState::Connected = *self.state.read().await {
            if let Some(ip) = exit_node_ip {
                info!("Enabling exit node routing to: {}", ip);
                // We need the nucleus host to add a persistent route to it
                // For simplicity, we can try to get it from the current proto if available
                if let Some(ref proto) = self.proto {
                    let _ = crate::routing::RoutingManager::setup_exit_node(
                        ip,
                        proto.get_nucleus_host(),
                    );
                }
            } else {
                info!("Restoring original routing (no exit node)");
                let _ = crate::routing::RoutingManager::restore_exit_node();
            }
        }
        Ok(())
    }

    pub async fn set_as_exit_node(&mut self, enabled: bool) -> Result<()> {
        info!("Setting as_exit_node to: {}", enabled);
        self.as_exit_node.store(enabled, Ordering::SeqCst);

        // Persist to config
        if let Ok(mut config) = CliConfig::load() {
            config.is_exit_node = enabled;
            let _ = config.save();
        }

        // Sync with backend if connected
        // IMPORTANT: Must send heartbeat FIRST to update device's is_exit_node status,
        // then call update_device() to allow it in the network
        let current_net_id = self.current_network_id.read().await.clone();
        if let (Some(client), Some(net_id), Some(dev_id)) =
            (&self.api_client, &current_net_id, &self.device_id)
        {
            // Step 1: Send heartbeat with new is_exit_node status and wait for it
            let dev_service = DeviceService::new(client);
            match dev_service.heartbeat(dev_id, enabled).await {
                Ok(_) => {
                    info!("Heartbeat sent with is_exit_node={}", enabled);
                }
                Err(e) => {
                    error!("Failed to send heartbeat with exit node status: {}", e);
                    // Continue anyway, the periodic heartbeat will eventually sync
                }
            }

            // Step 2: Now update the device in the network
            let net_service = NetworkService::new(client);
            if let Err(e) = net_service.update_device(net_id, dev_id, enabled).await {
                error!("Failed to sync exit node status to backend: {}", e);
                // We continue because local state is updated, but this indicates a sync issue
            } else {
                info!("Successfully synced exit node status to backend.");
            }
        }

        Ok(())
    }

    pub fn is_exit_node(&self) -> bool {
        self.as_exit_node.load(Ordering::SeqCst)
    }

    pub async fn get_connected_network_id(&self) -> Option<String> {
        self.current_network_id.read().await.clone()
    }

    pub async fn get_devices(&self) -> Result<Vec<DeviceResponse>> {
        let client = self.api_client.as_ref().context("Not authenticated")?;
        let dev_service = DeviceService::new(client);
        dev_service.list_all().await
    }

    pub async fn get_virtual_ip(&self) -> String {
        // First priority: active session IP
        if let Some(ref ip) = *self.virtual_ip.read().await {
            return ip.clone();
        }

        // Fallback to last recorded IP in config
        if let Ok(config) = CliConfig::load() {
            if let Some(info) = config.last_join_info {
                return info.virtual_ip;
            }
        }
        "".to_string()
    }

    pub fn get_identity_private_key(&self) -> [u8; 32] {
        self.identity.private_key_bytes()
    }

    pub fn get_base_url(&self) -> &str {
        &self.base_url
    }

    /// Configure nucleus settings for dual mode operation
    pub fn set_nucleus_config(&mut self, port: u16, secret: Option<String>) {
        self.nucleus_port = port;
        self.cluster_secret = secret;
    }

    pub async fn disconnect(&mut self) -> Result<()> {
        self.set_state(ConnectionState::Stopping).await;

        if let Some(tx) = self.shutdown_tx.take() {
            info!("Sending shutdown signal to background loops...");
            let _ = tx.send(());
        }

        self.proto = None;
        self.tun = None;

        {
            let mut nid = self.current_network_id.write().await;
            *nid = None;
        }
        {
            let mut vip = self.virtual_ip.write().await;
            *vip = None;
        }

        let _ = self.cleanup_adapters();

        if self.exit_node_ip.is_some() {
            let _ = crate::routing::RoutingManager::restore_exit_node();
        }

        self.set_state(ConnectionState::Disconnected).await;
        Ok(())
    }

    pub fn cleanup_adapters(&self) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            info!("Cleaning up all OmniEdge network adapters (Windows)...");

            // Method 1: Try to disable and remove via netsh
            let ps_cmd = "Get-NetAdapter -IncludeHidden | Where-Object { $_.Name -like 'OmniEdge*' } | ForEach-Object { Disable-NetAdapter -Name $_.Name -Confirm:$false -ErrorAction SilentlyContinue }";
            let _ = std::process::Command::new("powershell")
                .args(["-Command", ps_cmd])
                .output();

            // Method 2: Use pnputil to remove WinTun devices
            // Find and remove OmniEdge WinTun adapter instances
            let pnp_find = r#"Get-PnpDevice -FriendlyName '*OmniEdge*' -ErrorAction SilentlyContinue | ForEach-Object { pnputil /remove-device $_.InstanceId 2>$null }"#;
            let _ = std::process::Command::new("powershell")
                .args(["-Command", pnp_find])
                .output();

            // Method 3: Reset WinTun driver state by stopping/starting
            // This can help clear stale ring buffer registrations
            let reset_cmd = r#"
                $wintunService = Get-Service -Name 'WinTun' -ErrorAction SilentlyContinue;
                if ($wintunService) {
                    Stop-Service -Name 'WinTun' -Force -ErrorAction SilentlyContinue;
                    Start-Sleep -Milliseconds 500;
                    Start-Service -Name 'WinTun' -ErrorAction SilentlyContinue;
                }
            "#;
            let _ = std::process::Command::new("powershell")
                .args(["-Command", reset_cmd])
                .output();
        }

        #[cfg(target_os = "linux")]
        {
            info!("Cleaning up all OmniEdge network adapters (Linux)...");
            // Find all omniedge[0-9]+ interfaces and delete them
            let output = std::process::Command::new("sh")
                .arg("-c")
                .arg("ip link show | grep -oE 'omniedge[0-9]+'")
                .output();

            if let Ok(out) = output {
                let list = String::from_utf8_lossy(&out.stdout);
                for iface in list.lines() {
                    let iface = iface.trim();
                    if !iface.is_empty() {
                        debug!("Deleting linux interface: {}", iface);
                        let _ = std::process::Command::new("sudo")
                            .args(["ip", "link", "delete", iface])
                            .output();
                    }
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            info!("Cleaning up all OmniEdge network adapters (macOS)...");
            // macOS utun interfaces are usually ephemeral, but we look for any residue if possible
            // Most userspace implementations on macOS don't survive process death,
            // but we can try to find utun interfaces with our specific parameters if they ghost.
        }

        Ok(())
    }
}
