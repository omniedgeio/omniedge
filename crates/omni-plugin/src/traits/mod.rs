//! Plugin traits defining the interfaces for all plugin categories
//!
//! # Plugin Categories
//!
//! 1. **OmniEdgePlugin** - Base trait all plugins must implement
//! 2. **EventPlugin** - React to VPN lifecycle events
//! 3. **AuthPlugin** - Custom authentication providers
//! 4. **PolicyPlugin** - Network/exit node policy decisions
//! 5. **DataTriagePlugin** - High-bandwidth sensor data buffering
//! 6. **QoSPlugin** - Traffic classification and prioritization
//! 7. **PdMPlugin** - Predictive maintenance reporting
//! 8. **CompliancePlugin** - Privacy compliance and federated learning

use crate::error::PluginResult;
use crate::manifest::PluginManifest;
use crate::types::*;
use async_trait::async_trait;

/// Base trait that all plugins must implement
#[async_trait]
pub trait OmniEdgePlugin: Send + Sync {
    /// Return the plugin manifest
    fn manifest(&self) -> &PluginManifest;

    /// Called when the plugin is loaded
    async fn on_load(&mut self, ctx: &PluginContext) -> PluginResult<()>;

    /// Called when the plugin is about to be unloaded
    async fn on_unload(&mut self) -> PluginResult<()>;

    /// Get the plugin's unique identifier
    fn id(&self) -> &str {
        &self.manifest().id
    }

    /// Get the plugin's display name
    fn name(&self) -> &str {
        &self.manifest().name
    }

    /// Get the plugin's version
    fn version(&self) -> &str {
        &self.manifest().version
    }
}

// ============================================================================
// Category 1: Event Hooks
// ============================================================================

/// Event plugin trait for reacting to VPN lifecycle events
///
/// # Use Cases
/// - Audit logging to SIEM systems
/// - Slack/Teams notifications on connect/disconnect
/// - Trigger automation workflows (n8n, Zapier)
/// - Custom metrics collection
#[async_trait]
pub trait EventPlugin: OmniEdgePlugin {
    /// Called when connection state changes
    async fn on_state_change(&mut self, event: StateChangeEvent);

    /// Called when a new peer is discovered
    async fn on_peer_discovered(&mut self, peer: PeerInfo);

    /// Called when a peer disconnects
    async fn on_peer_disconnected(&mut self, peer: PeerInfo);

    /// Called on network join/leave
    async fn on_network_change(&mut self, event: NetworkEvent);

    /// Called periodically with connection statistics
    async fn on_stats_update(&mut self, stats: ConnectionStats);
}

// ============================================================================
// Category 2: Authentication Providers
// ============================================================================

/// Authentication plugin trait for custom SSO/identity providers
///
/// # Use Cases
/// - Enterprise SAML/OIDC integration (Okta, Auth0, Azure AD)
/// - Hardware security key authentication
/// - Custom enterprise auth systems
#[async_trait]
pub trait AuthPlugin: OmniEdgePlugin {
    /// Returns supported authentication methods
    fn supported_methods(&self) -> Vec<AuthMethod>;

    /// Initiate authentication flow
    async fn authenticate(&mut self, req: AuthRequest) -> PluginResult<AuthResponse>;

    /// Refresh expired tokens
    async fn refresh_token(&mut self, token: &str) -> PluginResult<AuthResponse>;

    /// Validate a session
    fn validate_session(&self, session: &Session) -> bool;
}

// ============================================================================
// Category 3: Network Policy Engines
// ============================================================================

/// Policy plugin trait for network selection and connection rules
///
/// # Use Cases
/// - Geo-fencing (auto-connect when at work location)
/// - Time-based policies (enable during work hours)
/// - Device posture checks (require up-to-date OS)
/// - Compliance enforcement (GDPR, HIPAA routing rules)
#[async_trait]
pub trait PolicyPlugin: OmniEdgePlugin {
    /// Select which network to join (return network_id)
    fn select_network(&self, ctx: &PolicyContext) -> Option<String>;

    /// Select exit node for traffic routing
    fn select_exit_node(&self, ctx: &PolicyContext) -> Option<String>;

    /// Validate if connection is allowed
    fn validate_connection(&self, ctx: &PolicyContext) -> PolicyDecision;

    /// Called when policy context changes (location, time, device state)
    async fn on_context_change(&mut self, ctx: &PolicyContext);
}

// ============================================================================
// Category 4: Black Box Data Triage (Robotics)
// ============================================================================

/// Buffer configuration for data triage
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BufferConfig {
    /// Maximum buffer size in bytes
    pub max_size_bytes: u64,
    /// Maximum duration to keep in buffer (seconds)
    pub max_duration_seconds: u32,
    /// Data sources to buffer (ROS topics, sensor IDs)
    pub sources: Vec<String>,
}

/// Trigger definition for data capture
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TriggerDefinition {
    /// Trigger name
    pub name: String,
    /// Trigger type
    pub trigger_type: TriggerType,
    /// Trigger conditions
    pub conditions: Vec<TriggerCondition>,
}

/// Types of triggers
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TriggerType {
    /// Manual trigger via API
    Manual,
    /// Anomaly detection threshold
    AnomalyThreshold { threshold: f32 },
    /// Error condition detected
    ErrorCondition,
    /// Time-based periodic capture
    Periodic { interval_seconds: u32 },
    /// Custom expression
    Custom { expression: String },
}

/// Trigger condition
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TriggerCondition {
    /// Field to check
    pub field: String,
    /// Comparison operator
    pub operator: ComparisonOperator,
    /// Value to compare against
    pub value: serde_json::Value,
}

/// Comparison operators
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonOperator {
    Equals,
    NotEquals,
    GreaterThan,
    LessThan,
    Contains,
    Regex,
}

/// Trigger event
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TriggerEvent {
    /// Trigger name that fired
    pub trigger_name: String,
    /// Timestamp when trigger fired
    pub timestamp: u64,
    /// Additional context
    pub context: std::collections::HashMap<String, serde_json::Value>,
}

/// Data triage plugin for high-bandwidth sensor data buffering
///
/// # Use Cases
/// - Robotics black box recording
/// - Event-triggered data capture
/// - Intelligent data upload on anomaly detection
#[async_trait]
pub trait DataTriagePlugin: OmniEdgePlugin {
    /// Configure the ring buffer for sensor data
    async fn configure_buffer(&mut self, config: BufferConfig) -> PluginResult<()>;

    /// Called for each incoming sensor packet (high frequency)
    fn on_sensor_data(&mut self, data: &[u8], source: &str, timestamp: u64);

    /// Define event triggers that cause data capture
    fn register_triggers(&self) -> Vec<TriggerDefinition>;

    /// Called when a trigger fires - plugin decides what to do
    async fn on_trigger(&mut self, trigger: &TriggerEvent) -> TriageAction;
}

// ============================================================================
// Category 5: QoS Enforcement - Synapse
// ============================================================================

/// QoS policy configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QoSPolicy {
    /// Policy name
    pub name: String,
    /// Rules for traffic classification
    pub rules: Vec<QoSRule>,
    /// Default traffic class for unmatched traffic
    pub default_class: TrafficClass,
}

/// QoS rule definition
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QoSRule {
    /// Rule name
    pub name: String,
    /// Match conditions
    pub match_conditions: QoSMatchCondition,
    /// Traffic class to assign
    pub traffic_class: TrafficClass,
    /// Priority (higher = evaluated first)
    pub priority: u32,
}

/// Match conditions for QoS rules
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QoSMatchCondition {
    /// Source address pattern (CIDR or regex)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Destination address pattern (CIDR or regex)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destination: Option<String>,
    /// Protocol (6=TCP, 17=UDP, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol: Option<u8>,
    /// Port range
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_range: Option<(u16, u16)>,
    /// ROS topic pattern (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ros_topic: Option<String>,
}

/// QoS plugin for traffic classification and prioritization
///
/// # Use Cases
/// - Software-defined network slicing (simulate 5G slicing)
/// - URLLC for teleop control packets
/// - Background class for bulk transfers
#[async_trait]
pub trait QoSPlugin: OmniEdgePlugin {
    /// Classify a packet and return its priority class
    fn classify_packet(&self, packet: &PacketInfo) -> TrafficClass;

    /// Get current QoS policy rules
    fn get_policy(&self) -> &QoSPolicy;

    /// Update policy dynamically
    async fn update_policy(&mut self, policy: QoSPolicy) -> PluginResult<()>;
}

// ============================================================================
// Category 6: PdM Reporter - Mechanic
// ============================================================================

/// PdM monitoring configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PdMConfig {
    /// Actuators to monitor
    pub actuator_ids: Vec<String>,
    /// Sampling frequency in Hz
    pub sampling_frequency_hz: u32,
    /// Health report interval in seconds
    pub report_interval_seconds: u32,
    /// Anomaly detection sensitivity (0.0 - 1.0)
    pub anomaly_sensitivity: f32,
}

/// Anomaly report
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AnomalyReport {
    /// Actuator ID with anomaly
    pub actuator_id: String,
    /// Anomaly type detected
    pub anomaly_type: AnomalyType,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,
    /// Timestamp of detection
    pub timestamp: u64,
    /// Recommended action
    pub recommended_action: String,
}

/// Types of anomalies detected
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnomalyType {
    Overheating,
    ExcessiveTorque,
    Vibration,
    CurrentSpike,
    PositionDrift,
    Degradation,
    Custom(String),
}

/// PdM plugin for predictive maintenance reporting
///
/// # Use Cases
/// - High-frequency actuator state monitoring (1kHz)
/// - Health score computation
/// - Failure prediction and alerting
#[async_trait]
pub trait PdMPlugin: OmniEdgePlugin {
    /// Configure which actuator states to monitor
    async fn configure_monitoring(&mut self, config: PdMConfig) -> PluginResult<()>;

    /// Called at high frequency with actuator states
    fn on_actuator_sample(&mut self, sample: ActuatorSample);

    /// Compute and return health report (called periodically)
    fn compute_health_report(&self) -> HealthReport;

    /// Get current anomaly state
    fn get_anomaly_state(&self) -> Option<AnomalyReport>;
}

// ============================================================================
// Category 7: Compliance & Federated Learning - Geneva
// ============================================================================

/// Data packet for compliance checking
#[derive(Debug, Clone)]
pub struct DataPacket {
    /// Packet source identifier
    pub source: String,
    /// Data type (e.g., "video", "telemetry", "logs")
    pub data_type: String,
    /// Raw data bytes
    pub data: Vec<u8>,
    /// Metadata
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
}

/// Compliance check result
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ComplianceResult {
    /// Data is compliant for transmission
    Compliant,
    /// Data requires transformation before transmission
    RequiresTransformation { transformations: Vec<String> },
    /// Data cannot be transmitted due to compliance rules
    Blocked { reason: String, regulation: String },
}

/// Federated learning gradient
#[derive(Debug, Clone)]
pub struct FLGradient {
    /// Model identifier
    pub model_id: String,
    /// Gradient values
    pub gradient: Vec<f32>,
    /// Training round
    pub round: u32,
    /// Number of samples used
    pub sample_count: u64,
}

/// Federated learning action
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum FLAction {
    /// Transmit gradient as-is
    TransmitGradient { gradient: Vec<f32> },
    /// Reject due to PII detection
    RejectPII { reason: String },
    /// Apply differential privacy before transmitting
    RequiresDifferentialPrivacy { epsilon: f32, gradient: Vec<f32> },
}

/// Compliance plugin for privacy and federated learning
///
/// # Use Cases
/// - GDPR compliance (data anonymization, residency)
/// - HIPAA compliance (encryption, access control)
/// - EU AI Act compliance
/// - Federated learning gradient handling
#[async_trait]
pub trait CompliancePlugin: OmniEdgePlugin {
    /// Check if data is compliant for transmission
    fn check_compliance(&self, data: &DataPacket) -> ComplianceResult;

    /// Apply privacy transformations (blur, mask, anonymize)
    async fn apply_privacy_filter(&self, data: &mut DataPacket) -> PluginResult<()>;

    /// Handle federated learning gradient
    fn process_fl_gradient(&self, gradient: &FLGradient) -> FLAction;

    /// Get current compliance mode
    fn get_compliance_mode(&self) -> ComplianceMode;
}

// ============================================================================
// Re-exports
// ============================================================================

pub use crate::types::{
    ActuatorHealth, ActuatorSample, ComplianceMode, HealthReport, OutputFormat, PacketInfo,
    PredictedFailure, Priority, TrafficClass, TriageAction,
};
