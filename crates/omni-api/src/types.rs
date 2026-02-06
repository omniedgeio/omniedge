use serde::{Deserialize, Deserializer, Serialize};

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

/// Auth response - uses only snake_case field names to avoid duplicate field errors
/// when API returns both camelCase and snake_case versions of the same field
#[derive(Serialize, Clone)]
pub struct AuthResp {
    #[serde(default)]
    pub token: String,
    pub refresh_token: String,
    pub access_token: String,
    pub id_token: String,
    pub expires_in: i32,
    pub email: Option<String>,
    pub user_id: Option<String>,
}

// Custom deserializer to handle APIs that return both camelCase and snake_case field names
impl<'de> Deserialize<'de> for AuthResp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Deserialize into a generic JSON Value first to handle duplicate fields
        let value = serde_json::Value::deserialize(deserializer)?;
        let obj = value
            .as_object()
            .ok_or_else(|| serde::de::Error::custom("expected object"))?;

        // Helper to get string from either camelCase or snake_case field
        let get_string = |camel: &str, snake: &str| -> String {
            obj.get(snake)
                .or_else(|| obj.get(camel))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };

        let get_i32 = |camel: &str, snake: &str| -> i32 {
            obj.get(snake)
                .or_else(|| obj.get(camel))
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32
        };

        let get_optional_string = |camel: &str, snake: &str| -> Option<String> {
            obj.get(snake)
                .or_else(|| obj.get(camel))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        };

        Ok(AuthResp {
            token: obj
                .get("token")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            refresh_token: get_string("refreshToken", "refresh_token"),
            access_token: get_string("accessToken", "access_token"),
            id_token: get_string("idToken", "id_token"),
            expires_in: get_i32("expiresIn", "expires_in"),
            email: get_optional_string("email", "email"),
            user_id: get_optional_string("userId", "user_id"),
        })
    }
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

#[derive(Debug, Serialize, Clone)]
pub struct WebSocketTokenResponse {
    pub token: String,
    pub refresh_token: String,
}

// Custom deserializer to handle APIs that return both camelCase and snake_case field names
impl<'de> Deserialize<'de> for WebSocketTokenResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let obj = value
            .as_object()
            .ok_or_else(|| serde::de::Error::custom("expected object"))?;

        // Helper to get string from multiple possible field names
        let get_string = |names: &[&str]| -> String {
            for name in names {
                if let Some(v) = obj.get(*name).and_then(|v| v.as_str()) {
                    return v.to_string();
                }
            }
            String::new()
        };

        Ok(WebSocketTokenResponse {
            // Server may send: "token", "accessToken", or "access_token"
            token: get_string(&["token", "accessToken", "access_token"]),
            // Server may send: "refresh_token" or "refreshToken"
            refresh_token: get_string(&["refresh_token", "refreshToken"]),
        })
    }
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
    #[serde(rename = "ip_range", alias = "ipRange", default)]
    pub ip_range: String,
    /// IPv6 IP range for the network (e.g., "fd00::/120")
    #[serde(rename = "ip_range_v6", alias = "ipRangeV6", default)]
    pub ip_range_v6: Option<String>,
    #[serde(default)]
    pub role: i32,
    pub server: Option<ServerResponse>,
    #[serde(
        rename = "selected_exit_node_id",
        alias = "selectedExitNodeId",
        default
    )]
    pub selected_exit_node_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VirtualNetworkDeviceResponse {
    #[serde(alias = "uuid")]
    pub id: String,
    pub name: String,
    #[serde(rename = "hardware_id", alias = "hardwareId", default)]
    pub hardware_id: String,
    #[serde(rename = "os", alias = "platform", alias = "os", default)]
    pub os: String,
    #[serde(
        rename = "virtual_ip",
        alias = "virtualIp",
        alias = "virtual_IP",
        default
    )]
    pub virtual_ip: String,
    /// IPv6 virtual IP address (dual-stack support)
    #[serde(rename = "virtual_ip_v6", alias = "virtualIpV6", default)]
    pub virtual_ip_v6: Option<String>,
    #[serde(rename = "is_exit_node", alias = "isExitNode", default)]
    pub is_exit_node: bool,
    #[serde(alias = "isOnline", default)]
    pub online: bool,
    #[serde(rename = "exit_node_enabled", alias = "exitNodeEnabled", default)]
    pub exit_node_enabled: bool,
    #[serde(
        rename = "selected_exit_node_id",
        alias = "selectedExitNodeId",
        default
    )]
    pub selected_exit_node_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct JoinVirtualNetworkResponse {
    /// The cluster/community name for the VPN connection.
    /// API may return both "cluster" and "community_name" fields with the same value.
    /// We use a custom deserializer to handle this without serde's duplicate field error.
    #[serde(default)]
    pub cluster: String,
    /// Ignored during deserialization - we use `cluster` field instead.
    /// This field exists because the API returns both "cluster" and "community_name".
    #[serde(
        rename = "community_name",
        alias = "communityName",
        default,
        skip_serializing
    )]
    pub _community_name: Option<String>,
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
    /// IPv6 virtual IP address (dual-stack support)
    #[serde(rename = "virtualIpV6", alias = "virtual_ip_v6", default)]
    pub virtual_ip_v6: Option<String>,
    /// IPv6 subnet prefix length (e.g., 120 for /120)
    #[serde(rename = "subnetPrefixV6", alias = "subnet_prefix_v6", default)]
    pub subnet_prefix_v6: Option<u8>,
    /// IPv6 IP range for the network (e.g., "fd00::/120")
    #[serde(rename = "ipRangeV6", alias = "ip_range_v6", default)]
    pub ip_range_v6: Option<String>,
    pub server: ServerResponse,
}

impl std::fmt::Debug for JoinVirtualNetworkResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JoinVirtualNetworkResponse")
            .field("cluster", &self.cluster)
            .field("secret_key", &"***REDACTED***")
            .field("virtual_ip", &self.virtual_ip)
            .field("subnet_mask", &self.subnet_mask)
            .field("virtual_ip_v6", &self.virtual_ip_v6)
            .field("subnet_prefix_v6", &self.subnet_prefix_v6)
            .field("ip_range_v6", &self.ip_range_v6)
            .field("server", &self.server)
            .finish()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerResponse {
    #[serde(default)]
    pub name: Option<String>,
    pub host: String,
    #[serde(default)]
    pub country: Option<String>,
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
    /// IPv6 virtual IP address (dual-stack support)
    #[serde(rename = "virtual_ip_v6", alias = "virtualIpV6", default)]
    pub virtual_ip_v6: Option<String>,
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserServer {
    pub id: String,
    pub name: String,
    pub host: String,
    #[serde(default)]
    pub country: Option<String>,
    #[serde(rename = "type", default)]
    pub server_type: Option<i32>,
    #[serde(default)]
    pub is_default: bool,
    #[serde(default)]
    pub created_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CreateUserServerRequest {
    pub name: String,
    pub host: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpdateUserServerRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
}
