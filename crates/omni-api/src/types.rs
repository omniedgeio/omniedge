use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SuccessResponse<T> {
    #[serde(default)]
    pub code: Option<i32>,
    pub message: Option<String>,
    pub data: T,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ErrorResponse {
    pub code: Option<serde_json::Value>,
    pub message: Option<String>,
    pub errors: Option<serde_json::Value>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ListWrapper<T> {
    pub data: T,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AuthResp {
    #[serde(default)]
    pub token: String,
    #[serde(alias = "refreshToken", alias = "refresh_token", default)]
    pub refresh_token: String,
    #[serde(alias = "accessToken", alias = "access_token", default)]
    pub access_token: String,
    #[serde(alias = "idToken", alias = "id_token", default)]
    pub id_token: String,
    #[serde(alias = "expiresIn", alias = "expires_in", default)]
    pub expires_in: i32,
    pub email: Option<String>,
    #[serde(alias = "userId", alias = "user_id", default)]
    pub user_id: Option<String>,
}

impl AuthResp {
    pub fn effective_token(&self) -> &str {
        if !self.token.is_empty() {
            &self.token
        } else {
            &self.access_token
        }
    }
}

impl std::fmt::Debug for AuthResp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthResp")
            .field("token", &"***REDACTED***")
            .field("refresh_token", &"***REDACTED***")
            .field("access_token", &"***REDACTED***")
            .field("id_token", &"***REDACTED***")
            .field("expires_in", &self.expires_in)
            .field("email", &self.email)
            .field("user_id", &self.user_id)
            .finish()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeviceCodeResp {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: String,
    pub expires_in: i32,
    pub interval: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SessionResponse {
    #[serde(alias = "session_id")]
    pub id: String,
    #[serde(rename = "auth_url")]
    pub auth_url: String,
    #[serde(rename = "expired_at", alias = "expires_at")]
    pub expires_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WebSocketTokenResponse {
    #[serde(alias = "accessToken", alias = "access_token", default)]
    pub token: String,
    #[serde(alias = "refreshToken", alias = "refresh_token", default)]
    pub refresh_token: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProfileResponse {
    #[serde(alias = "uuid", default)]
    pub id: String,
    pub name: Option<String>,
    pub email: Option<String>,
    #[serde(rename = "picture", alias = "avatar", default)]
    pub picture: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VirtualNetworkResponse {
    #[serde(alias = "uuid")]
    pub id: String,
    pub name: String,
    #[serde(rename = "ipRange", alias = "ip_range", default)]
    pub ip_range: String,
    #[serde(default)]
    pub role: i32,
    pub server: Option<ServerResponse>,
    #[serde(
        rename = "selectedExitNodeId",
        alias = "selected_exit_node_id",
        default
    )]
    pub selected_exit_node_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VirtualNetworkDeviceResponse {
    #[serde(alias = "uuid")]
    pub id: String,
    pub name: String,
    #[serde(rename = "hardwareId", alias = "hardware_id", default)]
    pub hardware_id: String,
    #[serde(rename = "platform", alias = "os", default)]
    pub os: String,
    #[serde(
        rename = "virtualIp",
        alias = "virtual_ip",
        alias = "virtual_IP",
        default
    )]
    pub virtual_ip: String,
    #[serde(rename = "isExitNode", alias = "is_exit_node", default)]
    pub is_exit_node: bool,
    #[serde(default)]
    pub online: bool,
    #[serde(rename = "exitNodeEnabled", alias = "exit_node_enabled", default)]
    pub exit_node_enabled: bool,
    #[serde(
        rename = "selectedExitNodeId",
        alias = "selected_exit_node_id",
        default
    )]
    pub selected_exit_node_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct JoinVirtualNetworkResponse {
    #[serde(rename = "communityName", alias = "community_name", default)]
    pub community_name: String,
    #[serde(rename = "secretKey", alias = "secret_key", default)]
    pub secret_key: String,
    #[serde(
        rename = "virtualIp",
        alias = "virtual_ip",
        alias = "virtual_IP",
        default
    )]
    pub virtual_ip: String,
    #[serde(rename = "subnetMask", alias = "subnet_mask", alias = "mask", default)]
    pub subnet_mask: String,
    pub server: ServerResponse,
}

impl std::fmt::Debug for JoinVirtualNetworkResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JoinVirtualNetworkResponse")
            .field("community_name", &self.community_name)
            .field("secret_key", &"***REDACTED***")
            .field("virtual_ip", &self.virtual_ip)
            .field("subnet_mask", &self.subnet_mask)
            .field("server", &self.server)
            .finish()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerResponse {
    pub host: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeviceResponse {
    #[serde(alias = "uuid")]
    pub id: String,
    pub name: String,
    #[serde(rename = "hardware_id", alias = "hardwareId", default)]
    pub hardware_id: String,
    #[serde(rename = "os", alias = "platform", default)]
    pub os: String,
    #[serde(
        rename = "virtual_ip",
        alias = "virtualIp",
        alias = "virtual_IP",
        default
    )]
    pub virtual_ip: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HeartbeatResponse {
    pub message: String,
    pub last_seen: String,
    pub exit_nodes: std::collections::HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScanResult {
    #[serde(rename = "hostName")]
    pub host_name: String,
    pub ipv4: String,
    pub ipv6: String,
    #[serde(rename = "mac_address", alias = "macAddress")]
    pub mac_address: String,
    pub vendor: String,
    pub os: String,
}
