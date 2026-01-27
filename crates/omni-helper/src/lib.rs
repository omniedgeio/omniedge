use omni_core::ConnectionManager;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

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
}

impl HelperServer {
    pub fn new(base_url: String) -> Self {
        let manager = ConnectionManager::new(base_url, None);
        Self {
            manager: Arc::new(Mutex::new(manager)),
        }
    }

    pub async fn handle_request(&self, req: HelperRequest) -> HelperResponse {
        match req.command.as_str() {
            "ping" => HelperResponse {
                success: true,
                message: "pong".to_string(),
                data: None,
            },
            "status" => {
                let state = self.manager.lock().await.get_state().await;
                HelperResponse {
                    success: true,
                    message: format!("{:?}", state),
                    data: Some(serde_json::to_value(state).unwrap()),
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
                manager.set_as_exit_node(enabled);
                HelperResponse {
                    success: true,
                    message: format!("Exit node status updated to: {}", enabled),
                    data: None,
                }
            }
            "is_exit_node" => {
                let manager = self.manager.lock().await;
                HelperResponse {
                    success: true,
                    message: "Current exit node status".to_string(),
                    data: Some(serde_json::to_value(manager.is_exit_node()).unwrap()),
                }
            }
            "get_virtual_ip" => {
                let manager = self.manager.lock().await;
                HelperResponse {
                    success: true,
                    message: "Current virtual IP".to_string(),
                    data: Some(serde_json::to_value(manager.get_virtual_ip().await).unwrap()),
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
