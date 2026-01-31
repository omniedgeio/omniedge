//! Core types for the plugin system

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;

/// Plugin capability enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Capability {
    /// React to VPN lifecycle events
    EventHooks,
    /// Custom authentication providers
    Authentication,
    /// Network and exit node policy
    NetworkPolicy,
    /// High-bandwidth sensor data triage
    DataTriage,
    /// Traffic prioritization and QoS
    QosEnforcement,
    /// Predictive maintenance reporting
    PdmReporting,
    /// Privacy compliance and federated learning
    Compliance,
    /// UI widget panels
    UiWidgets,
}

impl std::fmt::Display for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Capability::EventHooks => "event-hooks",
            Capability::Authentication => "authentication",
            Capability::NetworkPolicy => "network-policy",
            Capability::DataTriage => "data-triage",
            Capability::QosEnforcement => "qos-enforcement",
            Capability::PdmReporting => "pdm-reporting",
            Capability::Compliance => "compliance",
            Capability::UiWidgets => "ui-widgets",
        };
        write!(f, "{}", s)
    }
}

/// VPN connection state
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    Disconnected,
    Authenticating,
    Authenticated,
    Joining { network_id: String },
    Connected { network_id: String },
}

/// State change event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateChangeEvent {
    pub old_state: ConnectionState,
    pub new_state: ConnectionState,
    pub timestamp: u64,
    pub network_id: Option<String>,
}

/// Peer information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub public_key: String,
    pub virtual_ip: String,
    pub endpoint: Option<SocketAddr>,
    pub last_handshake: u64,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

/// Network information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInfo {
    pub id: String,
    pub name: String,
    pub subnet: String,
    pub peer_count: usize,
}

/// Device information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub virtual_ip: String,
    pub is_online: bool,
    pub is_exit_node: bool,
}

/// Network event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkEvent {
    pub event_type: NetworkEventType,
    pub network_id: String,
    pub timestamp: u64,
}

/// Network event type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkEventType {
    Joined,
    Left,
    PeerAdded,
    PeerRemoved,
}

/// Connection statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConnectionStats {
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub connected_peers: usize,
    pub latency_ms: Option<f64>,
    pub uptime_seconds: u64,
}

/// Geographic location
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoLocation {
    pub latitude: f64,
    pub longitude: f64,
    pub country: Option<String>,
    pub city: Option<String>,
}

/// Network type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkType {
    Wifi,
    Cellular,
    Ethernet,
    Unknown,
}

/// Policy context for policy plugins
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyContext {
    pub available_networks: Vec<NetworkInfo>,
    pub available_exit_nodes: Vec<DeviceInfo>,
    pub device_info: DeviceInfo,
    pub geo_location: Option<GeoLocation>,
    pub time_of_day: u64,
    pub network_type: NetworkType,
}

/// Policy decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PolicyDecision {
    Allow,
    Deny { reason: String },
    RequireAuth { method: String },
}

/// Authentication method
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthMethod {
    OAuth2 { provider: String },
    Saml { idp_url: String },
    Oidc { issuer: String },
    ApiKey,
    Custom { name: String },
}

/// Authentication request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthRequest {
    pub method: AuthMethod,
    pub redirect_uri: Option<String>,
    pub state: Option<String>,
}

/// Authentication response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    pub success: bool,
    pub token: Option<String>,
    pub refresh_token: Option<String>,
    pub expires_at: Option<u64>,
    pub error: Option<String>,
}

/// Session information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub token: String,
    pub user_id: String,
    pub expires_at: u64,
    pub claims: HashMap<String, serde_json::Value>,
}

/// Traffic class for QoS
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrafficClass {
    /// URLLC: Teleop control, Safety heartbeats (< 10ms budget)
    UltraReliableLowLatency { dscp: u8 },
    /// Standard: Routine telemetry
    Standard { dscp: u8 },
    /// Background: Bulk logs, OTA downloads
    Background { dscp: u8 },
    /// Drop: Non-compliant traffic
    Drop,
}

/// Packet information for QoS classification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacketInfo {
    pub source: SocketAddr,
    pub destination: SocketAddr,
    pub protocol: u8,
    pub size: usize,
    pub ros_topic: Option<String>,
}

/// Actuator sample for PdM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActuatorSample {
    pub actuator_id: String,
    pub timestamp_ns: u64,
    pub torque: f32,
    pub current: f32,
    pub temperature: f32,
    pub position: f32,
    pub velocity: f32,
}

/// Health report from PdM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    pub actuators: Vec<ActuatorHealth>,
    pub overall_health_score: f32,
    pub predicted_failures: Vec<PredictedFailure>,
}

/// Individual actuator health
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActuatorHealth {
    pub actuator_id: String,
    pub health_score: f32,
    pub anomaly_detected: bool,
    pub last_sample: Option<ActuatorSample>,
}

/// Predicted failure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictedFailure {
    pub actuator_id: String,
    pub failure_type: String,
    pub probability: f32,
    pub estimated_time_hours: Option<f32>,
}

/// Compliance mode
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ComplianceMode {
    Standard,
    Gdpr {
        anonymize_video: bool,
        data_residency: String,
    },
    Hipaa {
        encrypt_at_rest: bool,
    },
    Custom {
        name: String,
    },
}

/// Data triage action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TriageAction {
    Clip {
        duration_before: u32,
        duration_after: u32,
        format: OutputFormat,
    },
    Discard,
    StreamToCloud {
        priority: Priority,
    },
}

/// Output format for data triage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutputFormat {
    Mcap,
    Ros2Bag,
    Custom(String),
}

/// Priority level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Priority {
    Critical,
    High,
    Normal,
    Low,
}

/// Event types that can be emitted to plugins
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    StateChange(StateChangeEvent),
    PeerDiscovered { peer: PeerInfo },
    PeerDisconnected { peer: PeerInfo },
    NetworkChange(NetworkEvent),
    StatsUpdate { stats: ConnectionStats },
}

/// Plugin context provided to plugins during lifecycle
#[derive(Debug, Clone)]
pub struct PluginContext {
    /// Plugin-specific configuration
    pub config: HashMap<String, serde_json::Value>,
    /// Plugin data directory
    pub data_dir: std::path::PathBuf,
    /// Current network ID (if connected)
    pub network_id: Option<String>,
    /// Current device info
    pub device_info: Option<DeviceInfo>,
}

impl Default for PluginContext {
    fn default() -> Self {
        Self {
            config: HashMap::new(),
            data_dir: std::path::PathBuf::new(),
            network_id: None,
            device_info: None,
        }
    }
}
