//! API types and handlers for robot data collection
//!
//! Defines request/response types for IPC and REST API integration.
//! These types are designed for use with Tauri commands or HTTP endpoints.

use super::metadata::EpisodeMetadata;
use super::packager::PackageResult;
use super::plugin::{ActiveEpisodeInfo, DataCollectionConfig, PluginState, PluginStats};
use super::storage::{EpisodeIndexEntry, StorageStats};
use super::triggers::TriggerType;
use super::types::{EpisodeId, TimestampNs};
use super::upload::UploadSessionStats;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Common Types
// ============================================================================

/// API response wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    /// Whether the request succeeded
    pub success: bool,
    /// Response data (if successful)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    /// Error message (if failed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Error code (if failed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    /// Timestamp of response
    pub timestamp: TimestampNs,
}

impl<T> ApiResponse<T> {
    /// Create a success response
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            error_code: None,
            timestamp: current_timestamp_ns(),
        }
    }

    /// Create an error response
    pub fn error(message: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message.into()),
            error_code: Some(code.into()),
            timestamp: current_timestamp_ns(),
        }
    }

    /// Create an error response from an error type
    pub fn from_error<E: std::fmt::Display>(err: E, code: impl Into<String>) -> Self {
        Self::error(err.to_string(), code)
    }
}

/// Pagination parameters
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PaginationParams {
    /// Page number (0-indexed)
    #[serde(default)]
    pub page: u32,
    /// Items per page
    #[serde(default = "default_page_size")]
    pub page_size: u32,
}

fn default_page_size() -> u32 {
    20
}

impl PaginationParams {
    /// Get offset for database query
    pub fn offset(&self) -> u32 {
        self.page * self.page_size
    }

    /// Get limit for database query
    pub fn limit(&self) -> u32 {
        self.page_size
    }
}

/// Paginated response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    /// Items in current page
    pub items: Vec<T>,
    /// Total number of items
    pub total: u32,
    /// Current page (0-indexed)
    pub page: u32,
    /// Items per page
    pub page_size: u32,
    /// Total number of pages
    pub total_pages: u32,
    /// Whether there are more pages
    pub has_more: bool,
}

impl<T> PaginatedResponse<T> {
    /// Create a new paginated response
    pub fn new(items: Vec<T>, total: u32, params: &PaginationParams) -> Self {
        let total_pages = total.div_ceil(params.page_size);
        Self {
            items,
            total,
            page: params.page,
            page_size: params.page_size,
            total_pages,
            has_more: params.page + 1 < total_pages,
        }
    }
}

// ============================================================================
// Status Endpoints
// ============================================================================

/// Plugin status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    /// Plugin name
    pub name: String,
    /// Plugin version
    pub version: String,
    /// Current state
    pub state: PluginState,
    /// Robot ID
    pub robot_id: String,
    /// Fleet ID (if configured)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fleet_id: Option<String>,
    /// Whether currently recording
    pub is_recording: bool,
    /// Current episode (if recording)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_episode: Option<ActiveEpisodeInfo>,
    /// Plugin statistics
    pub stats: PluginStats,
    /// Storage statistics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_stats: Option<StorageStats>,
    /// Upload statistics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upload_stats: Option<UploadSessionStats>,
    /// Uptime in seconds
    pub uptime_seconds: f64,
}

/// Health check response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    /// Overall health status
    pub status: HealthStatus,
    /// Component health details
    pub components: HashMap<String, ComponentHealth>,
    /// Timestamp
    pub timestamp: TimestampNs,
}

/// Health status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// All systems healthy
    Healthy,
    /// Some issues but functional
    Degraded,
    /// Critical issues
    Unhealthy,
}

/// Component health
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    /// Component status
    pub status: HealthStatus,
    /// Optional message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Last check timestamp
    pub last_check: TimestampNs,
}

// ============================================================================
// Recording Endpoints
// ============================================================================

/// Start recording request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartRecordingRequest {
    /// Reason for recording
    pub reason: String,
    /// Duration in seconds (optional, uses default if not specified)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<f32>,
    /// Pre-roll duration in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre_roll_seconds: Option<f32>,
    /// Post-roll duration in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_roll_seconds: Option<f32>,
    /// Labels to add to the episode
    #[serde(default)]
    pub labels: HashMap<String, String>,
}

impl Default for StartRecordingRequest {
    fn default() -> Self {
        Self {
            reason: "Manual recording".to_string(),
            duration_seconds: None,
            pre_roll_seconds: None,
            post_roll_seconds: None,
            labels: HashMap::new(),
        }
    }
}

/// Start recording response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartRecordingResponse {
    /// Episode ID
    pub episode_id: EpisodeId,
    /// Start timestamp
    pub start_time_ns: TimestampNs,
    /// Expected end timestamp
    pub expected_end_ns: TimestampNs,
    /// Message
    pub message: String,
}

/// Stop recording request
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StopRecordingRequest {
    /// Reason for stopping (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Whether to discard the episode instead of saving
    #[serde(default)]
    pub discard: bool,
}

/// Stop recording response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopRecordingResponse {
    /// Episode ID
    pub episode_id: EpisodeId,
    /// Whether episode was saved
    pub saved: bool,
    /// Package result (if saved)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<PackageResultSummary>,
    /// Message
    pub message: String,
}

/// Simplified package result for API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageResultSummary {
    /// Episode ID
    pub episode_id: String,
    /// Path to MCAP file
    pub mcap_path: String,
    /// Total samples
    pub total_samples: u64,
    /// Duration in seconds
    pub duration_seconds: f64,
    /// File size in bytes
    pub file_size_bytes: u64,
    /// Privacy detections
    pub privacy_detections: u64,
}

impl From<&PackageResult> for PackageResultSummary {
    fn from(result: &PackageResult) -> Self {
        Self {
            episode_id: result.episode_id.as_str().to_string(),
            mcap_path: result.mcap_path.to_string_lossy().to_string(),
            total_samples: result.total_samples,
            duration_seconds: result.duration_ns as f64 / 1_000_000_000.0,
            file_size_bytes: result.file_size_bytes,
            privacy_detections: result.privacy_detections,
        }
    }
}

// ============================================================================
// Episode Endpoints
// ============================================================================

/// List episodes request
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ListEpisodesRequest {
    /// Pagination
    #[serde(flatten)]
    pub pagination: PaginationParams,
    /// Filter by robot ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub robot_id: Option<String>,
    /// Filter by uploaded status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uploaded: Option<bool>,
    /// Filter by minimum quality score
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_quality: Option<f32>,
    /// Filter by label key=value
    #[serde(default)]
    pub labels: HashMap<String, String>,
    /// Filter by start time (after)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_after: Option<TimestampNs>,
    /// Filter by end time (before)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_before: Option<TimestampNs>,
    /// Sort field
    #[serde(default)]
    pub sort_by: EpisodeSortField,
    /// Sort direction
    #[serde(default)]
    pub sort_order: SortOrder,
}

/// Episode sort field
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EpisodeSortField {
    /// Sort by creation time
    #[default]
    CreatedAt,
    /// Sort by start time
    StartTime,
    /// Sort by duration
    Duration,
    /// Sort by size
    Size,
    /// Sort by quality
    Quality,
}

/// Sort order
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    /// Ascending
    Asc,
    /// Descending
    #[default]
    Desc,
}

/// Episode summary for list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodeSummary {
    /// Episode ID
    pub episode_id: String,
    /// Robot ID
    pub robot_id: String,
    /// Start timestamp
    pub start_time_ns: TimestampNs,
    /// Duration in seconds
    pub duration_seconds: f64,
    /// Sample count
    pub sample_count: u64,
    /// File size in bytes
    pub size_bytes: u64,
    /// Quality score
    pub quality_score: f32,
    /// Whether uploaded
    pub uploaded: bool,
    /// Upload destination (if uploaded)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upload_destination: Option<String>,
    /// Labels
    pub labels: HashMap<String, String>,
    /// Created at
    pub created_at: TimestampNs,
}

impl From<&EpisodeIndexEntry> for EpisodeSummary {
    fn from(entry: &EpisodeIndexEntry) -> Self {
        Self {
            episode_id: entry.episode_id.as_str().to_string(),
            robot_id: entry.robot_id.clone(),
            start_time_ns: entry.start_time_ns,
            duration_seconds: entry.duration_seconds,
            sample_count: entry.sample_count,
            size_bytes: entry.size_bytes,
            quality_score: entry.quality_score,
            uploaded: entry.uploaded,
            upload_destination: entry.upload_destination.clone(),
            labels: entry.labels.clone(),
            created_at: entry.created_at,
        }
    }
}

/// Get episode request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetEpisodeRequest {
    /// Episode ID
    pub episode_id: String,
    /// Whether to include full metadata
    #[serde(default)]
    pub include_metadata: bool,
}

/// Get episode response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetEpisodeResponse {
    /// Episode summary
    pub summary: EpisodeSummary,
    /// Full metadata (if requested)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<EpisodeMetadata>,
    /// Path to MCAP file
    pub mcap_path: String,
}

/// Delete episode request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteEpisodeRequest {
    /// Episode ID
    pub episode_id: String,
    /// Skip confirmation (dangerous)
    #[serde(default)]
    pub force: bool,
}

/// Delete episode response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteEpisodeResponse {
    /// Episode ID
    pub episode_id: String,
    /// Whether deletion succeeded
    pub deleted: bool,
    /// Message
    pub message: String,
}

// ============================================================================
// Upload Endpoints
// ============================================================================

/// Upload episode request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadEpisodeRequest {
    /// Episode ID
    pub episode_id: String,
    /// Priority (higher = more urgent)
    #[serde(default)]
    pub priority: i32,
}

/// Upload episode response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadEpisodeResponse {
    /// Episode ID
    pub episode_id: String,
    /// Whether queued successfully
    pub queued: bool,
    /// Position in queue (if queued)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue_position: Option<usize>,
    /// Message
    pub message: String,
}

/// Batch upload request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchUploadRequest {
    /// Episode IDs to upload
    pub episode_ids: Vec<String>,
    /// Filter: upload all pending
    #[serde(default)]
    pub upload_all_pending: bool,
}

/// Batch upload response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchUploadResponse {
    /// Episodes queued
    pub queued_count: u32,
    /// Episodes skipped (already uploaded/queued)
    pub skipped_count: u32,
    /// Episodes not found
    pub not_found: Vec<String>,
    /// Message
    pub message: String,
}

/// Upload status request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadStatusRequest {
    /// Episode ID (optional, get all if not specified)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub episode_id: Option<String>,
}

/// Upload status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadStatusResponse {
    /// Session statistics
    pub stats: UploadSessionStats,
    /// Active uploads
    pub active: Vec<UploadProgressInfo>,
    /// Queued uploads
    pub queued: Vec<String>,
}

/// Upload progress info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadProgressInfo {
    /// Episode ID
    pub episode_id: String,
    /// Progress percentage
    pub progress_percent: f32,
    /// Bytes uploaded
    pub bytes_uploaded: u64,
    /// Total bytes
    pub total_bytes: u64,
    /// Upload speed in bytes/sec
    pub speed_bps: u64,
    /// ETA in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eta_seconds: Option<u64>,
}

// ============================================================================
// Trigger Endpoints
// ============================================================================

/// List triggers request
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ListTriggersRequest {
    /// Filter by enabled status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Filter by type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger_type: Option<TriggerType>,
}

/// Trigger info (API response)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiTriggerInfo {
    /// Trigger ID
    pub id: String,
    /// Trigger type
    pub trigger_type: String,
    /// Whether enabled
    pub enabled: bool,
    /// Configuration metadata
    pub config: HashMap<String, String>,
}

/// Manual trigger request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualTriggerRequest {
    /// Reason for trigger
    pub reason: String,
    /// Priority
    #[serde(default)]
    pub priority: String,
    /// Additional metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

/// Manual trigger response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualTriggerResponse {
    /// Whether trigger fired
    pub triggered: bool,
    /// Episode ID (if recording started)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub episode_id: Option<String>,
    /// Message
    pub message: String,
}

// ============================================================================
// Stream Endpoints
// ============================================================================

/// List streams request
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ListStreamsRequest {
    /// Filter by enabled status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

/// Stream info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamInfo {
    /// Stream ID
    pub stream_id: String,
    /// Stream type
    pub stream_type: String,
    /// Target Hz
    pub target_hz: f32,
    /// Whether enabled
    pub enabled: bool,
    /// Buffer statistics
    pub buffer_stats: ApiBufferStats,
}

/// Buffer statistics (API response)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiBufferStats {
    /// Current sample count
    pub sample_count: u64,
    /// Buffer capacity
    pub capacity: usize,
    /// Buffer utilization (0-100)
    pub utilization_percent: f32,
    /// Oldest sample timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oldest_sample_ns: Option<TimestampNs>,
    /// Newest sample timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub newest_sample_ns: Option<TimestampNs>,
    /// Approximate data rate (Hz)
    pub actual_hz: f32,
}

// ============================================================================
// Configuration Endpoints
// ============================================================================

/// Get configuration response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetConfigResponse {
    /// Current configuration
    pub config: DataCollectionConfig,
    /// Configuration source
    pub source: String,
    /// Whether config is writable
    pub writable: bool,
}

/// Update configuration request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConfigRequest {
    /// Configuration updates (partial)
    pub updates: serde_json::Value,
    /// Whether to persist changes
    #[serde(default)]
    pub persist: bool,
    /// Whether to restart plugin to apply
    #[serde(default)]
    pub restart: bool,
}

/// Update configuration response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConfigResponse {
    /// Whether update succeeded
    pub success: bool,
    /// Fields updated
    pub updated_fields: Vec<String>,
    /// Whether restart is required
    pub restart_required: bool,
    /// Message
    pub message: String,
}

// ============================================================================
// Error Codes
// ============================================================================

/// Standard error codes
pub mod error_codes {
    /// Plugin not initialized
    pub const NOT_INITIALIZED: &str = "NOT_INITIALIZED";
    /// Plugin not running
    pub const NOT_RUNNING: &str = "NOT_RUNNING";
    /// Plugin already running
    pub const ALREADY_RUNNING: &str = "ALREADY_RUNNING";
    /// Episode not found
    pub const EPISODE_NOT_FOUND: &str = "EPISODE_NOT_FOUND";
    /// Episode in progress
    pub const EPISODE_IN_PROGRESS: &str = "EPISODE_IN_PROGRESS";
    /// No episode in progress
    pub const NO_EPISODE_IN_PROGRESS: &str = "NO_EPISODE_IN_PROGRESS";
    /// Invalid request
    pub const INVALID_REQUEST: &str = "INVALID_REQUEST";
    /// Storage error
    pub const STORAGE_ERROR: &str = "STORAGE_ERROR";
    /// Upload error
    pub const UPLOAD_ERROR: &str = "UPLOAD_ERROR";
    /// Configuration error
    pub const CONFIG_ERROR: &str = "CONFIG_ERROR";
    /// Internal error
    pub const INTERNAL_ERROR: &str = "INTERNAL_ERROR";
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Get current timestamp in nanoseconds
fn current_timestamp_ns() -> TimestampNs {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

// ============================================================================
// Command Handlers (Tauri-compatible signatures)
// ============================================================================

/// Command handler trait for Tauri integration
///
/// Implement this trait to create command handlers that can be
/// registered with Tauri's command system.
///
/// Example:
/// ```ignore
/// #[tauri::command]
/// async fn get_status(
///     state: State<'_, Arc<Mutex<DataCollectionPlugin>>>
/// ) -> Result<StatusResponse, String> {
///     let plugin = state.lock().await;
///     // ... generate response
/// }
/// ```
pub trait CommandHandler: Send + Sync {
    /// Get plugin status
    fn get_status(&self) -> ApiResponse<StatusResponse>;

    /// Get health status
    fn get_health(&self) -> ApiResponse<HealthResponse>;

    /// Start recording
    fn start_recording(
        &mut self,
        request: StartRecordingRequest,
    ) -> ApiResponse<StartRecordingResponse>;

    /// Stop recording
    fn stop_recording(
        &mut self,
        request: StopRecordingRequest,
    ) -> ApiResponse<StopRecordingResponse>;

    /// List episodes
    fn list_episodes(
        &self,
        request: ListEpisodesRequest,
    ) -> ApiResponse<PaginatedResponse<EpisodeSummary>>;

    /// Get episode
    fn get_episode(&self, request: GetEpisodeRequest) -> ApiResponse<GetEpisodeResponse>;

    /// Delete episode
    fn delete_episode(
        &mut self,
        request: DeleteEpisodeRequest,
    ) -> ApiResponse<DeleteEpisodeResponse>;

    /// Upload episode
    fn upload_episode(
        &mut self,
        request: UploadEpisodeRequest,
    ) -> ApiResponse<UploadEpisodeResponse>;

    /// Get upload status
    fn get_upload_status(&self, request: UploadStatusRequest) -> ApiResponse<UploadStatusResponse>;

    /// List streams
    fn list_streams(&self, request: ListStreamsRequest) -> ApiResponse<Vec<StreamInfo>>;

    /// Trigger manual recording
    fn trigger_manual(
        &mut self,
        request: ManualTriggerRequest,
    ) -> ApiResponse<ManualTriggerResponse>;
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_response_success() {
        let response: ApiResponse<String> = ApiResponse::success("Hello".to_string());
        assert!(response.success);
        assert_eq!(response.data, Some("Hello".to_string()));
        assert!(response.error.is_none());
    }

    #[test]
    fn test_api_response_error() {
        let response: ApiResponse<String> = ApiResponse::error("Something went wrong", "ERR_TEST");
        assert!(!response.success);
        assert!(response.data.is_none());
        assert_eq!(response.error, Some("Something went wrong".to_string()));
        assert_eq!(response.error_code, Some("ERR_TEST".to_string()));
    }

    #[test]
    fn test_pagination() {
        let params = PaginationParams {
            page: 2,
            page_size: 10,
        };
        assert_eq!(params.offset(), 20);
        assert_eq!(params.limit(), 10);
    }

    #[test]
    fn test_paginated_response() {
        let items = vec![1, 2, 3, 4, 5];
        let params = PaginationParams {
            page: 0,
            page_size: 5,
        };
        let response = PaginatedResponse::new(items, 12, &params);

        assert_eq!(response.total, 12);
        assert_eq!(response.total_pages, 3);
        assert!(response.has_more);
    }

    #[test]
    fn test_start_recording_request_default() {
        let request = StartRecordingRequest::default();
        assert_eq!(request.reason, "Manual recording");
        assert!(request.labels.is_empty());
    }

    #[test]
    fn test_health_status() {
        assert_ne!(HealthStatus::Healthy, HealthStatus::Unhealthy);
    }

    #[test]
    fn test_episode_sort_field() {
        assert_eq!(EpisodeSortField::default(), EpisodeSortField::CreatedAt);
    }

    #[test]
    fn test_sort_order() {
        assert_eq!(SortOrder::default(), SortOrder::Desc);
    }

    #[test]
    fn test_package_result_summary() {
        let result = PackageResult {
            episode_id: EpisodeId::from_string("test-001"),
            mcap_path: std::path::PathBuf::from("/data/test.mcap"),
            metadata_path: None,
            total_samples: 1000,
            samples_per_stream: HashMap::new(),
            duration_ns: 5_000_000_000,
            file_size_bytes: 1024 * 1024,
            privacy_detections: 3,
            packaging_time_ms: 500,
        };

        let summary = PackageResultSummary::from(&result);
        assert_eq!(summary.episode_id, "test-001");
        assert_eq!(summary.total_samples, 1000);
        assert_eq!(summary.duration_seconds, 5.0);
    }

    #[test]
    fn test_error_codes() {
        assert_eq!(error_codes::NOT_INITIALIZED, "NOT_INITIALIZED");
        assert_eq!(error_codes::EPISODE_NOT_FOUND, "EPISODE_NOT_FOUND");
    }
}
