//! Emergency access (break-glass) mechanism
//!
//! This module provides emergency access capabilities for safety-critical
//! situations where normal SSH access policies need to be bypassed.
//!
//! ## Features
//!
//! - Request emergency access with justification
//! - Multi-level approval workflow
//! - Time-limited access grants
//! - Token validation and caching
//! - Audit logging for compliance
//! - Webhook support for real-time status updates
//!
//! ## Usage
//!
//! ```rust,ignore
//! use omni_ssh::emergency::{EmergencyAccessManager, EmergencyAccessRequest};
//!
//! let manager = EmergencyAccessManager::new(config);
//!
//! // Request emergency access
//! let request = EmergencyAccessRequest::builder()
//!     .requester_id("user-123")
//!     .target_node_id("node-456")
//!     .reason("Critical production incident INC-789")
//!     .duration_secs(3600)
//!     .build();
//!
//! let request_id = manager.request_access(request).await?;
//!
//! // Wait for approval (or poll)
//! let grant = manager.wait_for_approval(&request_id, Duration::from_secs(300)).await?;
//!
//! // Use the access token for SSH
//! let token = grant.access_token;
//! ```

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Configuration for emergency access
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergencyAccessConfig {
    /// Cloud API endpoint for emergency access
    pub api_endpoint: String,
    /// API key for authentication
    pub api_key: String,
    /// Maximum duration for emergency access (seconds)
    pub max_duration_secs: u64,
    /// Default duration if not specified (seconds)
    pub default_duration_secs: u64,
    /// Whether auto-approval is enabled for certain conditions
    pub auto_approve_enabled: bool,
    /// Roles that can auto-approve
    pub auto_approve_roles: Vec<String>,
    /// Webhook URL for status updates
    pub webhook_url: Option<String>,
    /// Token cache TTL (seconds)
    pub token_cache_ttl_secs: u64,
    /// Require MFA for emergency access
    pub require_mfa: bool,
    /// Minimum approvers required
    pub min_approvers: u32,
}

impl Default for EmergencyAccessConfig {
    fn default() -> Self {
        Self {
            api_endpoint: "https://api.omniedge.io/v1/emergency".to_string(),
            api_key: String::new(),
            max_duration_secs: 14400,    // 4 hours max
            default_duration_secs: 3600, // 1 hour default
            auto_approve_enabled: false,
            auto_approve_roles: vec!["admin".to_string(), "oncall".to_string()],
            webhook_url: None,
            token_cache_ttl_secs: 60, // Cache tokens for 1 minute
            require_mfa: true,
            min_approvers: 1,
        }
    }
}

/// Emergency access request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergencyAccessRequest {
    /// Request ID (generated if not provided)
    #[serde(default)]
    pub request_id: String,
    /// User making the request
    pub requester_id: String,
    /// Requester's email for notifications
    #[serde(default)]
    pub requester_email: Option<String>,
    /// Target node ID
    pub target_node_id: String,
    /// Target node name (optional, for display)
    #[serde(default)]
    pub target_node_name: Option<String>,
    /// Network ID
    #[serde(default)]
    pub network_id: Option<String>,
    /// Reason for emergency access
    pub reason: String,
    /// Incident ticket ID (optional but recommended)
    #[serde(default)]
    pub incident_id: Option<String>,
    /// Severity level
    #[serde(default)]
    pub severity: EmergencySeverity,
    /// When the request was made
    #[serde(default = "Utc::now")]
    pub requested_at: DateTime<Utc>,
    /// How long the access should last (seconds)
    pub duration_secs: u64,
    /// Commands to be allowed (if restricted)
    #[serde(default)]
    pub allowed_commands: Vec<String>,
    /// MFA token if required
    #[serde(default)]
    pub mfa_token: Option<String>,
    /// Additional metadata
    #[serde(default)]
    pub metadata: std::collections::HashMap<String, String>,
}

impl EmergencyAccessRequest {
    /// Create a new builder for EmergencyAccessRequest
    pub fn builder() -> EmergencyAccessRequestBuilder {
        EmergencyAccessRequestBuilder::default()
    }
}

/// Builder for EmergencyAccessRequest
#[derive(Debug, Default)]
pub struct EmergencyAccessRequestBuilder {
    requester_id: Option<String>,
    requester_email: Option<String>,
    target_node_id: Option<String>,
    target_node_name: Option<String>,
    network_id: Option<String>,
    reason: Option<String>,
    incident_id: Option<String>,
    severity: EmergencySeverity,
    duration_secs: Option<u64>,
    allowed_commands: Vec<String>,
    mfa_token: Option<String>,
    metadata: std::collections::HashMap<String, String>,
}

impl EmergencyAccessRequestBuilder {
    /// Set the requester ID
    pub fn requester_id(mut self, id: impl Into<String>) -> Self {
        self.requester_id = Some(id.into());
        self
    }

    /// Set the requester email
    pub fn requester_email(mut self, email: impl Into<String>) -> Self {
        self.requester_email = Some(email.into());
        self
    }

    /// Set the target node ID
    pub fn target_node_id(mut self, id: impl Into<String>) -> Self {
        self.target_node_id = Some(id.into());
        self
    }

    /// Set the target node name
    pub fn target_node_name(mut self, name: impl Into<String>) -> Self {
        self.target_node_name = Some(name.into());
        self
    }

    /// Set the network ID
    pub fn network_id(mut self, id: impl Into<String>) -> Self {
        self.network_id = Some(id.into());
        self
    }

    /// Set the reason for access
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// Set the incident ID
    pub fn incident_id(mut self, id: impl Into<String>) -> Self {
        self.incident_id = Some(id.into());
        self
    }

    /// Set the severity level
    pub fn severity(mut self, severity: EmergencySeverity) -> Self {
        self.severity = severity;
        self
    }

    /// Set the duration in seconds
    pub fn duration_secs(mut self, secs: u64) -> Self {
        self.duration_secs = Some(secs);
        self
    }

    /// Add an allowed command
    pub fn allowed_command(mut self, cmd: impl Into<String>) -> Self {
        self.allowed_commands.push(cmd.into());
        self
    }

    /// Set all allowed commands
    pub fn allowed_commands(mut self, cmds: Vec<String>) -> Self {
        self.allowed_commands = cmds;
        self
    }

    /// Set the MFA token
    pub fn mfa_token(mut self, token: impl Into<String>) -> Self {
        self.mfa_token = Some(token.into());
        self
    }

    /// Add metadata
    pub fn metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Build the request
    pub fn build(self) -> anyhow::Result<EmergencyAccessRequest> {
        let requester_id = self
            .requester_id
            .ok_or_else(|| anyhow::anyhow!("requester_id is required"))?;
        let target_node_id = self
            .target_node_id
            .ok_or_else(|| anyhow::anyhow!("target_node_id is required"))?;
        let reason = self
            .reason
            .ok_or_else(|| anyhow::anyhow!("reason is required"))?;

        Ok(EmergencyAccessRequest {
            request_id: Uuid::new_v4().to_string(),
            requester_id,
            requester_email: self.requester_email,
            target_node_id,
            target_node_name: self.target_node_name,
            network_id: self.network_id,
            reason,
            incident_id: self.incident_id,
            severity: self.severity,
            requested_at: Utc::now(),
            duration_secs: self.duration_secs.unwrap_or(3600),
            allowed_commands: self.allowed_commands,
            mfa_token: self.mfa_token,
            metadata: self.metadata,
        })
    }
}

/// Emergency severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum EmergencySeverity {
    /// Critical - requires immediate attention
    Critical,
    /// High - significant impact
    High,
    /// Medium - moderate impact
    #[default]
    Medium,
    /// Low - minor impact
    Low,
}

impl std::fmt::Display for EmergencySeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmergencySeverity::Critical => write!(f, "critical"),
            EmergencySeverity::High => write!(f, "high"),
            EmergencySeverity::Medium => write!(f, "medium"),
            EmergencySeverity::Low => write!(f, "low"),
        }
    }
}

/// Emergency access grant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergencyAccessGrant {
    /// Grant ID
    pub grant_id: String,
    /// Original request
    pub request: EmergencyAccessRequest,
    /// Who approved the access
    pub approver_id: Option<String>,
    /// Approver's name (for display)
    pub approver_name: Option<String>,
    /// When access was granted
    pub granted_at: DateTime<Utc>,
    /// When access expires
    pub expires_at: DateTime<Utc>,
    /// Token for accessing
    pub access_token: String,
    /// Whether this was auto-approved
    pub auto_approved: bool,
    /// Approval notes
    pub approval_notes: Option<String>,
    /// Restrictions applied
    pub restrictions: AccessRestrictions,
}

impl EmergencyAccessGrant {
    /// Check if the grant is still valid
    pub fn is_valid(&self) -> bool {
        Utc::now() < self.expires_at
    }

    /// Get remaining duration
    pub fn remaining_duration(&self) -> Option<ChronoDuration> {
        let remaining = self.expires_at - Utc::now();
        if remaining > ChronoDuration::zero() {
            Some(remaining)
        } else {
            None
        }
    }
}

/// Access restrictions for emergency grants
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AccessRestrictions {
    /// Read-only access (no modifications)
    pub read_only: bool,
    /// Allowed commands (empty = all allowed)
    pub allowed_commands: Vec<String>,
    /// Blocked commands
    pub blocked_commands: Vec<String>,
    /// Source IP restrictions
    pub allowed_source_ips: Vec<String>,
    /// Recording required
    pub recording_required: bool,
    /// SFTP disabled
    pub sftp_disabled: bool,
    /// Port forwarding disabled
    pub port_forwarding_disabled: bool,
}

/// Emergency access status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EmergencyAccessStatus {
    /// Request is pending approval
    Pending,
    /// Request is awaiting additional approvers
    AwaitingApprovers {
        /// Current number of approvals
        current: u32,
        /// Required number of approvals
        required: u32,
    },
    /// Request was approved
    Approved,
    /// Request was denied
    Denied {
        /// Reason for denial
        reason: Option<String>,
    },
    /// Access has expired
    Expired,
    /// Access was revoked
    Revoked {
        /// Who revoked access
        revoked_by: Option<String>,
        /// Reason for revocation
        reason: Option<String>,
    },
    /// Request timed out waiting for approval
    TimedOut,
}

impl std::fmt::Display for EmergencyAccessStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmergencyAccessStatus::Pending => write!(f, "pending"),
            EmergencyAccessStatus::AwaitingApprovers { current, required } => {
                write!(f, "awaiting_approvers ({}/{})", current, required)
            }
            EmergencyAccessStatus::Approved => write!(f, "approved"),
            EmergencyAccessStatus::Denied { reason } => {
                if let Some(r) = reason {
                    write!(f, "denied: {}", r)
                } else {
                    write!(f, "denied")
                }
            }
            EmergencyAccessStatus::Expired => write!(f, "expired"),
            EmergencyAccessStatus::Revoked { revoked_by, reason } => {
                write!(f, "revoked")?;
                if let Some(by) = revoked_by {
                    write!(f, " by {}", by)?;
                }
                if let Some(r) = reason {
                    write!(f, ": {}", r)?;
                }
                Ok(())
            }
            EmergencyAccessStatus::TimedOut => write!(f, "timed_out"),
        }
    }
}

/// Status response from the API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergencyStatusResponse {
    /// Request ID
    pub request_id: String,
    /// Current status
    pub status: EmergencyAccessStatus,
    /// Grant details (if approved)
    pub grant: Option<EmergencyAccessGrant>,
    /// Estimated wait time in seconds (if pending)
    pub estimated_wait_secs: Option<u64>,
    /// Approvers who have responded
    pub approver_responses: Vec<ApproverResponse>,
}

/// Response from an approver
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApproverResponse {
    /// Approver ID
    pub approver_id: String,
    /// Approver name
    pub approver_name: Option<String>,
    /// Whether they approved
    pub approved: bool,
    /// Their response message
    pub message: Option<String>,
    /// When they responded
    pub responded_at: DateTime<Utc>,
}

/// Audit log entry for emergency access
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergencyAuditEntry {
    /// Entry ID
    pub entry_id: String,
    /// Request ID
    pub request_id: String,
    /// Event type
    pub event_type: EmergencyAuditEvent,
    /// Actor who triggered the event
    pub actor_id: String,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Additional details
    pub details: std::collections::HashMap<String, serde_json::Value>,
    /// Source IP
    pub source_ip: Option<String>,
}

/// Emergency access audit events
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmergencyAuditEvent {
    /// Access requested
    AccessRequested,
    /// Access approved
    AccessApproved,
    /// Access denied
    AccessDenied,
    /// Access granted (token issued)
    AccessGranted,
    /// Session started
    SessionStarted,
    /// Command executed
    CommandExecuted,
    /// File accessed
    FileAccessed,
    /// Session ended
    SessionEnded,
    /// Access expired
    AccessExpired,
    /// Access revoked
    AccessRevoked,
    /// Token validated
    TokenValidated,
    /// Token validation failed
    TokenValidationFailed,
}

/// Cached token entry
#[derive(Debug, Clone)]
struct CachedGrant {
    grant: EmergencyAccessGrant,
    cached_at: DateTime<Utc>,
}

/// Event sent via broadcast when status changes
#[derive(Debug, Clone)]
pub struct StatusChangeEvent {
    /// Request ID
    pub request_id: String,
    /// New status
    pub status: EmergencyAccessStatus,
    /// Grant if approved
    pub grant: Option<EmergencyAccessGrant>,
}

/// HTTP client abstraction for testing
#[async_trait::async_trait]
pub trait EmergencyHttpClient: Send + Sync {
    /// Submit an emergency access request
    async fn submit_request(&self, request: &EmergencyAccessRequest) -> anyhow::Result<String>;

    /// Check status of a request
    async fn check_status(&self, request_id: &str) -> anyhow::Result<EmergencyStatusResponse>;

    /// Validate a token
    async fn validate_token(
        &self,
        token: &str,
        target_node_id: &str,
    ) -> anyhow::Result<Option<EmergencyAccessGrant>>;

    /// Revoke access
    async fn revoke_access(&self, request_id: &str, reason: Option<&str>) -> anyhow::Result<()>;

    /// Submit audit event
    async fn submit_audit(&self, entry: &EmergencyAuditEntry) -> anyhow::Result<()>;
}

/// Default HTTP client implementation using reqwest
pub struct DefaultEmergencyHttpClient {
    client: reqwest::Client,
    config: EmergencyAccessConfig,
}

impl DefaultEmergencyHttpClient {
    /// Create a new HTTP client
    pub fn new(config: EmergencyAccessConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self { client, config }
    }
}

#[async_trait::async_trait]
impl EmergencyHttpClient for DefaultEmergencyHttpClient {
    async fn submit_request(&self, request: &EmergencyAccessRequest) -> anyhow::Result<String> {
        let url = format!("{}/requests", self.config.api_endpoint);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to submit request: {} - {}", status, body);
        }

        #[derive(Deserialize)]
        struct SubmitResponse {
            request_id: String,
        }

        let result: SubmitResponse = response.json().await?;
        Ok(result.request_id)
    }

    async fn check_status(&self, request_id: &str) -> anyhow::Result<EmergencyStatusResponse> {
        let url = format!("{}/requests/{}", self.config.api_endpoint, request_id);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to check status: {} - {}", status, body);
        }

        let result: EmergencyStatusResponse = response.json().await?;
        Ok(result)
    }

    async fn validate_token(
        &self,
        token: &str,
        target_node_id: &str,
    ) -> anyhow::Result<Option<EmergencyAccessGrant>> {
        let url = format!("{}/validate", self.config.api_endpoint);

        #[derive(Serialize)]
        struct ValidateRequest<'a> {
            token: &'a str,
            target_node_id: &'a str,
        }

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&ValidateRequest {
                token,
                target_node_id,
            })
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to validate token: {} - {}", status, body);
        }

        #[derive(Deserialize)]
        struct ValidateResponse {
            valid: bool,
            grant: Option<EmergencyAccessGrant>,
        }

        let result: ValidateResponse = response.json().await?;
        if result.valid {
            Ok(result.grant)
        } else {
            Ok(None)
        }
    }

    async fn revoke_access(&self, request_id: &str, reason: Option<&str>) -> anyhow::Result<()> {
        let url = format!(
            "{}/requests/{}/revoke",
            self.config.api_endpoint, request_id
        );

        #[derive(Serialize)]
        struct RevokeRequest<'a> {
            reason: Option<&'a str>,
        }

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&RevokeRequest { reason })
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to revoke access: {} - {}", status, body);
        }

        Ok(())
    }

    async fn submit_audit(&self, entry: &EmergencyAuditEntry) -> anyhow::Result<()> {
        let url = format!("{}/audit", self.config.api_endpoint);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(entry)
            .send()
            .await?;

        if !response.status().is_success() {
            // Don't fail on audit errors, just log
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            warn!("Failed to submit audit entry: {} - {}", status, body);
        }

        Ok(())
    }
}

/// Emergency access manager
pub struct EmergencyAccessManager {
    config: EmergencyAccessConfig,
    http_client: Arc<dyn EmergencyHttpClient>,
    /// Cache of validated tokens
    token_cache: DashMap<String, CachedGrant>,
    /// Active grants by request ID
    active_grants: DashMap<String, EmergencyAccessGrant>,
    /// Status change broadcaster
    status_tx: broadcast::Sender<StatusChangeEvent>,
    /// Pending requests being polled
    pending_polls: RwLock<std::collections::HashSet<String>>,
}

impl EmergencyAccessManager {
    /// Create a new emergency access manager with default HTTP client
    pub fn new(config: EmergencyAccessConfig) -> Self {
        let http_client = Arc::new(DefaultEmergencyHttpClient::new(config.clone()));
        Self::with_http_client(config, http_client)
    }

    /// Create with a custom HTTP client (for testing)
    pub fn with_http_client(
        config: EmergencyAccessConfig,
        http_client: Arc<dyn EmergencyHttpClient>,
    ) -> Self {
        let (status_tx, _) = broadcast::channel(100);

        Self {
            config,
            http_client,
            token_cache: DashMap::new(),
            active_grants: DashMap::new(),
            status_tx,
            pending_polls: RwLock::new(std::collections::HashSet::new()),
        }
    }

    /// Subscribe to status change events
    pub fn subscribe(&self) -> broadcast::Receiver<StatusChangeEvent> {
        self.status_tx.subscribe()
    }

    /// Request emergency access
    pub async fn request_access(&self, request: EmergencyAccessRequest) -> anyhow::Result<String> {
        // Validate request
        if request.reason.trim().is_empty() {
            anyhow::bail!("Reason is required for emergency access");
        }

        if request.duration_secs > self.config.max_duration_secs {
            anyhow::bail!(
                "Requested duration {} exceeds maximum allowed {}",
                request.duration_secs,
                self.config.max_duration_secs
            );
        }

        if self.config.require_mfa && request.mfa_token.is_none() {
            anyhow::bail!("MFA token is required for emergency access");
        }

        info!(
            requester = %request.requester_id,
            target = %request.target_node_id,
            severity = %request.severity,
            reason = %request.reason,
            "Submitting emergency access request"
        );

        // Submit to cloud
        let request_id = self.http_client.submit_request(&request).await?;

        // Log audit event
        self.audit(EmergencyAuditEntry {
            entry_id: Uuid::new_v4().to_string(),
            request_id: request_id.clone(),
            event_type: EmergencyAuditEvent::AccessRequested,
            actor_id: request.requester_id.clone(),
            timestamp: Utc::now(),
            details: {
                let mut map = std::collections::HashMap::new();
                map.insert(
                    "target_node_id".to_string(),
                    serde_json::json!(request.target_node_id),
                );
                map.insert(
                    "severity".to_string(),
                    serde_json::json!(request.severity.to_string()),
                );
                map.insert(
                    "duration_secs".to_string(),
                    serde_json::json!(request.duration_secs),
                );
                if let Some(ref incident_id) = request.incident_id {
                    map.insert("incident_id".to_string(), serde_json::json!(incident_id));
                }
                map
            },
            source_ip: None,
        })
        .await;

        Ok(request_id)
    }

    /// Check status of an access request
    pub async fn check_status(&self, request_id: &str) -> anyhow::Result<EmergencyStatusResponse> {
        debug!(request_id = %request_id, "Checking emergency access status");

        let response = self.http_client.check_status(request_id).await?;

        // If approved, cache the grant
        if let Some(ref grant) = response.grant {
            self.active_grants
                .insert(request_id.to_string(), grant.clone());

            // Notify subscribers
            let _ = self.status_tx.send(StatusChangeEvent {
                request_id: request_id.to_string(),
                status: response.status.clone(),
                grant: Some(grant.clone()),
            });
        }

        Ok(response)
    }

    /// Wait for approval with timeout
    pub async fn wait_for_approval(
        &self,
        request_id: &str,
        timeout: Duration,
    ) -> anyhow::Result<EmergencyAccessGrant> {
        let start = std::time::Instant::now();
        let poll_interval = Duration::from_secs(5);

        // Mark as being polled
        {
            let mut pending = self.pending_polls.write().await;
            pending.insert(request_id.to_string());
        }

        loop {
            if start.elapsed() > timeout {
                // Remove from pending
                let mut pending = self.pending_polls.write().await;
                pending.remove(request_id);

                anyhow::bail!("Timed out waiting for approval");
            }

            let response = self.check_status(request_id).await?;

            match response.status {
                EmergencyAccessStatus::Approved => {
                    if let Some(grant) = response.grant {
                        // Remove from pending
                        let mut pending = self.pending_polls.write().await;
                        pending.remove(request_id);

                        info!(
                            request_id = %request_id,
                            expires_at = %grant.expires_at,
                            "Emergency access approved"
                        );

                        return Ok(grant);
                    }
                }
                EmergencyAccessStatus::Denied { reason } => {
                    // Remove from pending
                    let mut pending = self.pending_polls.write().await;
                    pending.remove(request_id);

                    let msg = reason.unwrap_or_else(|| "No reason provided".to_string());
                    anyhow::bail!("Emergency access denied: {}", msg);
                }
                EmergencyAccessStatus::TimedOut => {
                    // Remove from pending
                    let mut pending = self.pending_polls.write().await;
                    pending.remove(request_id);

                    anyhow::bail!("Emergency access request timed out");
                }
                EmergencyAccessStatus::Revoked { reason, .. } => {
                    // Remove from pending
                    let mut pending = self.pending_polls.write().await;
                    pending.remove(request_id);

                    let msg = reason.unwrap_or_else(|| "No reason provided".to_string());
                    anyhow::bail!("Emergency access was revoked: {}", msg);
                }
                EmergencyAccessStatus::Pending
                | EmergencyAccessStatus::AwaitingApprovers { .. } => {
                    // Continue waiting
                    debug!(
                        request_id = %request_id,
                        status = %response.status,
                        "Still waiting for approval"
                    );
                }
                EmergencyAccessStatus::Expired => {
                    // Remove from pending
                    let mut pending = self.pending_polls.write().await;
                    pending.remove(request_id);

                    anyhow::bail!("Emergency access request expired before approval");
                }
            }

            tokio::time::sleep(poll_interval).await;
        }
    }

    /// Validate an emergency access token
    pub async fn validate_token(
        &self,
        token: &str,
        target_node_id: &str,
    ) -> anyhow::Result<Option<EmergencyAccessGrant>> {
        // Check cache first
        let cache_key = format!("{}:{}", token, target_node_id);

        if let Some(cached) = self.token_cache.get(&cache_key) {
            let cache_age = Utc::now() - cached.cached_at;
            if cache_age.num_seconds() < self.config.token_cache_ttl_secs as i64 {
                // Check if grant is still valid
                if cached.grant.is_valid() {
                    debug!(target = %target_node_id, "Token validated from cache");
                    return Ok(Some(cached.grant.clone()));
                } else {
                    // Remove expired grant from cache
                    drop(cached);
                    self.token_cache.remove(&cache_key);
                }
            }
        }

        // Validate with cloud
        let result = self
            .http_client
            .validate_token(token, target_node_id)
            .await?;

        if let Some(ref grant) = result {
            // Cache the valid grant
            self.token_cache.insert(
                cache_key,
                CachedGrant {
                    grant: grant.clone(),
                    cached_at: Utc::now(),
                },
            );

            // Log audit event
            self.audit(EmergencyAuditEntry {
                entry_id: Uuid::new_v4().to_string(),
                request_id: grant.request.request_id.clone(),
                event_type: EmergencyAuditEvent::TokenValidated,
                actor_id: grant.request.requester_id.clone(),
                timestamp: Utc::now(),
                details: {
                    let mut map = std::collections::HashMap::new();
                    map.insert(
                        "target_node_id".to_string(),
                        serde_json::json!(target_node_id),
                    );
                    map
                },
                source_ip: None,
            })
            .await;

            info!(
                request_id = %grant.request.request_id,
                target = %target_node_id,
                expires_at = %grant.expires_at,
                "Emergency access token validated"
            );
        } else {
            warn!(target = %target_node_id, "Emergency access token validation failed");

            // Log failed validation (we don't have request_id, so use a placeholder)
            self.audit(EmergencyAuditEntry {
                entry_id: Uuid::new_v4().to_string(),
                request_id: "unknown".to_string(),
                event_type: EmergencyAuditEvent::TokenValidationFailed,
                actor_id: "unknown".to_string(),
                timestamp: Utc::now(),
                details: {
                    let mut map = std::collections::HashMap::new();
                    map.insert(
                        "target_node_id".to_string(),
                        serde_json::json!(target_node_id),
                    );
                    map
                },
                source_ip: None,
            })
            .await;
        }

        Ok(result)
    }

    /// Revoke emergency access
    pub async fn revoke_access(
        &self,
        request_id: &str,
        reason: Option<&str>,
    ) -> anyhow::Result<()> {
        info!(request_id = %request_id, reason = ?reason, "Revoking emergency access");

        self.http_client.revoke_access(request_id, reason).await?;

        // Remove from active grants
        if let Some((_, grant)) = self.active_grants.remove(request_id) {
            // Clear related cache entries
            let token = &grant.access_token;
            let target = &grant.request.target_node_id;
            let cache_key = format!("{}:{}", token, target);
            self.token_cache.remove(&cache_key);

            // Notify subscribers
            let _ = self.status_tx.send(StatusChangeEvent {
                request_id: request_id.to_string(),
                status: EmergencyAccessStatus::Revoked {
                    revoked_by: None,
                    reason: reason.map(String::from),
                },
                grant: None,
            });

            // Log audit
            self.audit(EmergencyAuditEntry {
                entry_id: Uuid::new_v4().to_string(),
                request_id: request_id.to_string(),
                event_type: EmergencyAuditEvent::AccessRevoked,
                actor_id: grant.request.requester_id.clone(),
                timestamp: Utc::now(),
                details: {
                    let mut map = std::collections::HashMap::new();
                    if let Some(r) = reason {
                        map.insert("reason".to_string(), serde_json::json!(r));
                    }
                    map
                },
                source_ip: None,
            })
            .await;
        }

        Ok(())
    }

    /// Get an active grant by request ID
    pub fn get_active_grant(&self, request_id: &str) -> Option<EmergencyAccessGrant> {
        self.active_grants.get(request_id).map(|g| g.clone())
    }

    /// List all active grants
    pub fn list_active_grants(&self) -> Vec<EmergencyAccessGrant> {
        self.active_grants
            .iter()
            .filter(|g| g.is_valid())
            .map(|g| g.clone())
            .collect()
    }

    /// Clean up expired grants and cache entries
    pub async fn cleanup_expired(&self) {
        let now = Utc::now();

        // Remove expired active grants
        let expired_requests: Vec<String> = self
            .active_grants
            .iter()
            .filter(|g| !g.is_valid())
            .map(|g| g.request.request_id.clone())
            .collect();

        for request_id in expired_requests {
            if let Some((_, grant)) = self.active_grants.remove(&request_id) {
                info!(request_id = %request_id, "Emergency access grant expired");

                // Notify subscribers
                let _ = self.status_tx.send(StatusChangeEvent {
                    request_id: request_id.clone(),
                    status: EmergencyAccessStatus::Expired,
                    grant: None,
                });

                // Log audit
                self.audit(EmergencyAuditEntry {
                    entry_id: Uuid::new_v4().to_string(),
                    request_id,
                    event_type: EmergencyAuditEvent::AccessExpired,
                    actor_id: grant.request.requester_id.clone(),
                    timestamp: now,
                    details: std::collections::HashMap::new(),
                    source_ip: None,
                })
                .await;
            }
        }

        // Remove stale cache entries
        let stale_keys: Vec<String> = self
            .token_cache
            .iter()
            .filter(|entry| {
                let age = now - entry.cached_at;
                age.num_seconds() > self.config.token_cache_ttl_secs as i64 * 2
            })
            .map(|entry| entry.key().clone())
            .collect();

        for key in stale_keys {
            self.token_cache.remove(&key);
        }
    }

    /// Start background cleanup task
    pub fn start_cleanup_task(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));

            loop {
                interval.tick().await;
                self.cleanup_expired().await;
            }
        })
    }

    /// Submit an audit entry
    async fn audit(&self, entry: EmergencyAuditEntry) {
        if let Err(e) = self.http_client.submit_audit(&entry).await {
            error!(error = %e, "Failed to submit audit entry");
        }
    }

    /// Record session start for an emergency access grant
    pub async fn record_session_start(&self, request_id: &str, source_ip: Option<String>) {
        if let Some(grant) = self.active_grants.get(request_id) {
            self.audit(EmergencyAuditEntry {
                entry_id: Uuid::new_v4().to_string(),
                request_id: request_id.to_string(),
                event_type: EmergencyAuditEvent::SessionStarted,
                actor_id: grant.request.requester_id.clone(),
                timestamp: Utc::now(),
                details: std::collections::HashMap::new(),
                source_ip,
            })
            .await;
        }
    }

    /// Record command execution for an emergency access grant
    pub async fn record_command(&self, request_id: &str, command: &str, source_ip: Option<String>) {
        if let Some(grant) = self.active_grants.get(request_id) {
            self.audit(EmergencyAuditEntry {
                entry_id: Uuid::new_v4().to_string(),
                request_id: request_id.to_string(),
                event_type: EmergencyAuditEvent::CommandExecuted,
                actor_id: grant.request.requester_id.clone(),
                timestamp: Utc::now(),
                details: {
                    let mut map = std::collections::HashMap::new();
                    map.insert("command".to_string(), serde_json::json!(command));
                    map
                },
                source_ip,
            })
            .await;
        }
    }

    /// Record session end for an emergency access grant
    pub async fn record_session_end(&self, request_id: &str, source_ip: Option<String>) {
        if let Some(grant) = self.active_grants.get(request_id) {
            self.audit(EmergencyAuditEntry {
                entry_id: Uuid::new_v4().to_string(),
                request_id: request_id.to_string(),
                event_type: EmergencyAuditEvent::SessionEnded,
                actor_id: grant.request.requester_id.clone(),
                timestamp: Utc::now(),
                details: std::collections::HashMap::new(),
                source_ip,
            })
            .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Mock HTTP client for testing
    struct MockHttpClient {
        requests: Mutex<Vec<EmergencyAccessRequest>>,
        status_response: Mutex<Option<EmergencyStatusResponse>>,
        grant: Mutex<Option<EmergencyAccessGrant>>,
    }

    impl MockHttpClient {
        fn new() -> Self {
            Self {
                requests: Mutex::new(Vec::new()),
                status_response: Mutex::new(None),
                grant: Mutex::new(None),
            }
        }

        fn set_status_response(&self, response: EmergencyStatusResponse) {
            *self.status_response.lock().unwrap() = Some(response);
        }

        fn set_grant(&self, grant: EmergencyAccessGrant) {
            *self.grant.lock().unwrap() = Some(grant);
        }
    }

    #[async_trait::async_trait]
    impl EmergencyHttpClient for MockHttpClient {
        async fn submit_request(&self, request: &EmergencyAccessRequest) -> anyhow::Result<String> {
            let mut requests = self.requests.lock().unwrap();
            requests.push(request.clone());
            Ok(request.request_id.clone())
        }

        async fn check_status(&self, _request_id: &str) -> anyhow::Result<EmergencyStatusResponse> {
            let response = self.status_response.lock().unwrap();
            response
                .clone()
                .ok_or_else(|| anyhow::anyhow!("No mock response set"))
        }

        async fn validate_token(
            &self,
            _token: &str,
            _target_node_id: &str,
        ) -> anyhow::Result<Option<EmergencyAccessGrant>> {
            let grant = self.grant.lock().unwrap();
            Ok(grant.clone())
        }

        async fn revoke_access(
            &self,
            _request_id: &str,
            _reason: Option<&str>,
        ) -> anyhow::Result<()> {
            Ok(())
        }

        async fn submit_audit(&self, _entry: &EmergencyAuditEntry) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_request_builder() {
        let request = EmergencyAccessRequest::builder()
            .requester_id("user-123")
            .target_node_id("node-456")
            .reason("Critical incident INC-789")
            .severity(EmergencySeverity::Critical)
            .incident_id("INC-789")
            .duration_secs(7200)
            .allowed_command("systemctl restart app")
            .metadata("team", "platform")
            .build()
            .unwrap();

        assert_eq!(request.requester_id, "user-123");
        assert_eq!(request.target_node_id, "node-456");
        assert_eq!(request.reason, "Critical incident INC-789");
        assert_eq!(request.severity, EmergencySeverity::Critical);
        assert_eq!(request.incident_id, Some("INC-789".to_string()));
        assert_eq!(request.duration_secs, 7200);
        assert_eq!(request.allowed_commands, vec!["systemctl restart app"]);
        assert_eq!(request.metadata.get("team"), Some(&"platform".to_string()));
    }

    #[test]
    fn test_request_builder_missing_required() {
        let result = EmergencyAccessRequest::builder()
            .target_node_id("node-456")
            .reason("Test reason")
            .build();

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("requester_id"));
    }

    #[test]
    fn test_grant_validity() {
        let request = EmergencyAccessRequest::builder()
            .requester_id("user-123")
            .target_node_id("node-456")
            .reason("Test")
            .build()
            .unwrap();

        // Valid grant (expires in 1 hour)
        let valid_grant = EmergencyAccessGrant {
            grant_id: "grant-1".to_string(),
            request: request.clone(),
            approver_id: Some("admin-1".to_string()),
            approver_name: Some("Admin".to_string()),
            granted_at: Utc::now(),
            expires_at: Utc::now() + ChronoDuration::hours(1),
            access_token: "token-123".to_string(),
            auto_approved: false,
            approval_notes: None,
            restrictions: AccessRestrictions::default(),
        };

        assert!(valid_grant.is_valid());
        assert!(valid_grant.remaining_duration().is_some());

        // Expired grant
        let expired_grant = EmergencyAccessGrant {
            expires_at: Utc::now() - ChronoDuration::hours(1),
            ..valid_grant
        };

        assert!(!expired_grant.is_valid());
        assert!(expired_grant.remaining_duration().is_none());
    }

    #[test]
    fn test_status_display() {
        assert_eq!(EmergencyAccessStatus::Pending.to_string(), "pending");
        assert_eq!(EmergencyAccessStatus::Approved.to_string(), "approved");
        assert_eq!(
            EmergencyAccessStatus::Denied {
                reason: Some("Policy violation".to_string())
            }
            .to_string(),
            "denied: Policy violation"
        );
        assert_eq!(
            EmergencyAccessStatus::AwaitingApprovers {
                current: 1,
                required: 2
            }
            .to_string(),
            "awaiting_approvers (1/2)"
        );
    }

    #[tokio::test]
    async fn test_request_access() {
        let mock = Arc::new(MockHttpClient::new());
        let config = EmergencyAccessConfig {
            require_mfa: false,
            ..Default::default()
        };
        let manager = EmergencyAccessManager::with_http_client(config, mock.clone());

        let request = EmergencyAccessRequest::builder()
            .requester_id("user-123")
            .target_node_id("node-456")
            .reason("Test emergency access")
            .build()
            .unwrap();

        let request_id = manager.request_access(request).await.unwrap();
        assert!(!request_id.is_empty());

        let requests = mock.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].requester_id, "user-123");
    }

    #[tokio::test]
    async fn test_validate_token_caching() {
        let mock = Arc::new(MockHttpClient::new());
        let config = EmergencyAccessConfig {
            token_cache_ttl_secs: 60,
            require_mfa: false,
            ..Default::default()
        };

        let request = EmergencyAccessRequest::builder()
            .requester_id("user-123")
            .target_node_id("node-456")
            .reason("Test")
            .build()
            .unwrap();

        let grant = EmergencyAccessGrant {
            grant_id: "grant-1".to_string(),
            request,
            approver_id: Some("admin".to_string()),
            approver_name: None,
            granted_at: Utc::now(),
            expires_at: Utc::now() + ChronoDuration::hours(1),
            access_token: "test-token".to_string(),
            auto_approved: false,
            approval_notes: None,
            restrictions: AccessRestrictions::default(),
        };

        mock.set_grant(grant.clone());

        let manager = EmergencyAccessManager::with_http_client(config, mock);

        // First validation should hit the mock
        let result1 = manager
            .validate_token("test-token", "node-456")
            .await
            .unwrap();
        assert!(result1.is_some());

        // Second validation should use cache
        let result2 = manager
            .validate_token("test-token", "node-456")
            .await
            .unwrap();
        assert!(result2.is_some());
        assert_eq!(result1.unwrap().grant_id, result2.unwrap().grant_id);
    }

    #[test]
    fn test_severity_display() {
        assert_eq!(EmergencySeverity::Critical.to_string(), "critical");
        assert_eq!(EmergencySeverity::High.to_string(), "high");
        assert_eq!(EmergencySeverity::Medium.to_string(), "medium");
        assert_eq!(EmergencySeverity::Low.to_string(), "low");
    }

    #[test]
    fn test_default_config() {
        let config = EmergencyAccessConfig::default();
        assert_eq!(config.max_duration_secs, 14400);
        assert_eq!(config.default_duration_secs, 3600);
        assert!(!config.auto_approve_enabled);
        assert!(config.require_mfa);
        assert_eq!(config.min_approvers, 1);
    }
}
