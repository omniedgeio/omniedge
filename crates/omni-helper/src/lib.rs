use omni_core::{state::ConnectionState, ConnectionManager};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

#[derive(Debug, Serialize, Deserialize)]
pub struct HelperRequest {
    pub command: String,
    pub args: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StartArgs {
    pub token: String,
    pub network_id: String,
    pub device_id: String,
    pub hardware_id: String,
    #[serde(default)]
    pub as_exit_node: bool,
    #[serde(default)]
    pub nucleus: bool,
    pub exit_node_ip: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HelperResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

pub struct HelperServer {
    manager: Arc<Mutex<ConnectionManager>>,
    state: Arc<RwLock<ConnectionState>>,
    network_id: Arc<RwLock<Option<String>>>,
    virtual_ip: Arc<RwLock<Option<String>>>,
    as_exit_node: Arc<AtomicBool>,
}

impl HelperServer {
    pub fn new(base_url: String) -> Self {
        let manager = ConnectionManager::new(base_url, None);
        let state = manager.get_state_handle();
        let network_id = manager.get_network_id_handle();
        let virtual_ip = manager.get_virtual_ip_handle();
        let as_exit_node = manager.get_as_exit_node_handle();
        Self {
            manager: Arc::new(Mutex::new(manager)),
            state,
            network_id,
            virtual_ip,
            as_exit_node,
        }
    }

    pub async fn handle_request(&self, req: HelperRequest) -> HelperResponse {
        match req.command.as_str() {
            "ping" => HelperResponse {
                success: true,
                message: "pong".to_string(),
                data: None,
            },
            "version" => HelperResponse {
                success: true,
                message: "omniedge-helper".to_string(),
                data: Some(serde_json::json!({
                    "version": env!("CARGO_PKG_VERSION"),
                    "protocol": "rust-v2",
                })),
            },
            "status" => {
                let state = self.state.read().await.clone();
                let network_id = self.network_id.read().await.clone();
                let virtual_ip = self.virtual_ip.read().await.clone();
                // We could still lock for virtual_ip fallback if needed, but usually it's in the handle

                HelperResponse {
                    success: true,
                    message: format!("{:?}", state),
                    data: Some(serde_json::json!({
                        "state": state,
                        "network_id": network_id,
                        "virtual_ip": virtual_ip,
                    })),
                }
            }
            "start_vpn" => {
                let args: StartArgs = match serde_json::from_value(req.args) {
                    Ok(a) => a,
                    Err(e) => {
                        return HelperResponse {
                            success: false,
                            message: format!("Invalid start args: {}", e),
                            data: None,
                        }
                    }
                };

                let mut manager = self.manager.lock().await;
                match manager
                    .connect_with_token(
                        args.token,
                        &args.network_id,
                        &args.device_id,
                        &args.hardware_id,
                        args.nucleus,
                        args.as_exit_node,
                        args.exit_node_ip,
                    )
                    .await
                {
                    Ok(_) => HelperResponse {
                        success: true,
                        message: "VPN start initiated".to_string(),
                        data: None,
                    },
                    Err(e) => HelperResponse {
                        success: false,
                        message: format!("Failed to start VPN: {}", e),
                        data: None,
                    },
                }
            }
            "set_as_exit_node" => {
                let enabled = req.args["enabled"].as_bool().unwrap_or(false);
                let mut manager = self.manager.lock().await;
                match manager.set_as_exit_node(enabled).await {
                    Ok(_) => HelperResponse {
                        success: true,
                        message: format!("Exit node status updated to: {}", enabled),
                        data: None,
                    },
                    Err(e) => HelperResponse {
                        success: false,
                        message: format!("Failed to update exit node status: {}", e),
                        data: None,
                    },
                }
            }
            "is_exit_node" => HelperResponse {
                success: true,
                message: "Current exit node status".to_string(),
                data: Some(serde_json::to_value(self.as_exit_node.load(Ordering::SeqCst)).unwrap()),
            },
            "get_virtual_ip" => {
                let vip = self.virtual_ip.read().await.clone().unwrap_or_default();
                HelperResponse {
                    success: true,
                    message: "Current virtual IP".to_string(),
                    data: Some(serde_json::to_value(vip).unwrap()),
                }
            }
            "stop_vpn" => {
                let mut manager = self.manager.lock().await;
                let _ = manager.disconnect().await;
                HelperResponse {
                    success: true,
                    message: "VPN stopped".to_string(),
                    data: None,
                }
            }
            _ => HelperResponse {
                success: false,
                message: "Unknown command".to_string(),
                data: None,
            },
        }
    }
}
