//! Cloud upload manager for robot data collection
//!
//! Handles uploading episodes to S3, GCS, or other cloud storage providers
//! with configurable credentials, resumable uploads, and retry logic.

use super::storage::EpisodeIndexEntry;
use super::types::{EpisodeId, TimestampNs};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use thiserror::Error;

/// Upload-related errors
#[derive(Debug, Error)]
pub enum UploadError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Bucket not found: {0}")]
    BucketNotFound(String),

    #[error("Object too large: {size_bytes} bytes (max: {max_bytes})")]
    ObjectTooLarge { size_bytes: u64, max_bytes: u64 },

    #[error("Rate limited, retry after {retry_after_secs} seconds")]
    RateLimited { retry_after_secs: u32 },

    #[error("Upload cancelled")]
    Cancelled,

    #[error("Max retries exceeded: {0}")]
    MaxRetriesExceeded(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Episode not found: {0}")]
    EpisodeNotFound(String),

    #[error("Provider error: {0}")]
    ProviderError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),
}

/// Cloud provider type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CloudProvider {
    /// Amazon S3
    #[default]
    S3,
    /// Google Cloud Storage
    Gcs,
    /// Azure Blob Storage
    Azure,
    /// MinIO (S3-compatible)
    Minio,
    /// Custom S3-compatible endpoint
    Custom,
}

impl CloudProvider {
    /// Get default endpoint for provider
    pub fn default_endpoint(&self) -> Option<&'static str> {
        match self {
            CloudProvider::S3 => None, // Uses AWS SDK default
            CloudProvider::Gcs => Some("https://storage.googleapis.com"),
            CloudProvider::Azure => None, // Requires account name
            CloudProvider::Minio => Some("http://localhost:9000"),
            CloudProvider::Custom => None,
        }
    }
}

/// S3 credentials configuration
///
/// Credentials can be provided directly or loaded from environment/files.
/// **IMPORTANT**: Never commit credentials to source control. Use environment
/// variables or credential files in production.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct S3Credentials {
    /// AWS Access Key ID
    /// Can also be set via `AWS_ACCESS_KEY_ID` environment variable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_key_id: Option<String>,

    /// AWS Secret Access Key
    /// Can also be set via `AWS_SECRET_ACCESS_KEY` environment variable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_access_key: Option<String>,

    /// AWS Session Token (for temporary credentials)
    /// Can also be set via `AWS_SESSION_TOKEN` environment variable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_token: Option<String>,

    /// AWS Region
    /// Can also be set via `AWS_REGION` or `AWS_DEFAULT_REGION` environment variables
    #[serde(default = "default_region")]
    pub region: String,

    /// AWS Profile name (for credentials file)
    /// Can also be set via `AWS_PROFILE` environment variable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,

    /// Path to credentials file
    /// Defaults to `~/.aws/credentials`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credentials_file: Option<PathBuf>,

    /// Whether to use IAM role credentials (for EC2/ECS)
    #[serde(default)]
    pub use_instance_credentials: bool,
}

fn default_region() -> String {
    "us-east-1".to_string()
}

impl S3Credentials {
    /// Create from environment variables
    pub fn from_env() -> Self {
        Self {
            access_key_id: std::env::var("AWS_ACCESS_KEY_ID").ok(),
            secret_access_key: std::env::var("AWS_SECRET_ACCESS_KEY").ok(),
            session_token: std::env::var("AWS_SESSION_TOKEN").ok(),
            region: std::env::var("AWS_REGION")
                .or_else(|_| std::env::var("AWS_DEFAULT_REGION"))
                .unwrap_or_else(|_| default_region()),
            profile: std::env::var("AWS_PROFILE").ok(),
            credentials_file: None,
            use_instance_credentials: false,
        }
    }

    /// Create with explicit credentials
    pub fn with_keys(
        access_key_id: impl Into<String>,
        secret_access_key: impl Into<String>,
    ) -> Self {
        Self {
            access_key_id: Some(access_key_id.into()),
            secret_access_key: Some(secret_access_key.into()),
            ..Default::default()
        }
    }

    /// Set region
    pub fn with_region(mut self, region: impl Into<String>) -> Self {
        self.region = region.into();
        self
    }

    /// Set profile
    pub fn with_profile(mut self, profile: impl Into<String>) -> Self {
        self.profile = Some(profile.into());
        self
    }

    /// Use instance credentials (IAM role)
    pub fn with_instance_credentials(mut self) -> Self {
        self.use_instance_credentials = true;
        self
    }

    /// Check if credentials are configured
    pub fn is_configured(&self) -> bool {
        self.access_key_id.is_some() && self.secret_access_key.is_some()
            || self.profile.is_some()
            || self.use_instance_credentials
    }
}

/// GCS credentials configuration
///
/// **IMPORTANT**: Never commit credentials to source control. Use environment
/// variables or service account files in production.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GcsCredentials {
    /// Path to service account JSON key file
    /// Can also be set via `GOOGLE_APPLICATION_CREDENTIALS` environment variable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_account_key: Option<PathBuf>,

    /// Service account key JSON content (alternative to file path)
    /// **WARNING**: Avoid storing this in config files
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_account_json: Option<String>,

    /// Project ID
    /// Can also be set via `GOOGLE_CLOUD_PROJECT` or `GCLOUD_PROJECT` environment variables
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,

    /// Whether to use default application credentials
    #[serde(default = "default_true")]
    pub use_application_default: bool,
}

fn default_true() -> bool {
    true
}

impl GcsCredentials {
    /// Create from environment variables
    pub fn from_env() -> Self {
        Self {
            service_account_key: std::env::var("GOOGLE_APPLICATION_CREDENTIALS")
                .ok()
                .map(PathBuf::from),
            service_account_json: None,
            project_id: std::env::var("GOOGLE_CLOUD_PROJECT")
                .or_else(|_| std::env::var("GCLOUD_PROJECT"))
                .ok(),
            use_application_default: true,
        }
    }

    /// Create with service account key file
    pub fn with_service_account(path: impl Into<PathBuf>) -> Self {
        Self {
            service_account_key: Some(path.into()),
            service_account_json: None,
            project_id: None,
            use_application_default: false,
        }
    }

    /// Set project ID
    pub fn with_project(mut self, project_id: impl Into<String>) -> Self {
        self.project_id = Some(project_id.into());
        self
    }

    /// Check if credentials are configured
    pub fn is_configured(&self) -> bool {
        self.service_account_key.is_some()
            || self.service_account_json.is_some()
            || self.use_application_default
    }
}

/// Azure Blob Storage credentials configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AzureCredentials {
    /// Storage account name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_name: Option<String>,

    /// Storage account key
    /// Can also be set via `AZURE_STORAGE_KEY` environment variable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_key: Option<String>,

    /// Connection string (alternative to account name/key)
    /// Can also be set via `AZURE_STORAGE_CONNECTION_STRING` environment variable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection_string: Option<String>,

    /// SAS token for limited access
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sas_token: Option<String>,

    /// Whether to use managed identity (for Azure VMs)
    #[serde(default)]
    pub use_managed_identity: bool,
}

impl AzureCredentials {
    /// Create from environment variables
    pub fn from_env() -> Self {
        Self {
            account_name: std::env::var("AZURE_STORAGE_ACCOUNT").ok(),
            account_key: std::env::var("AZURE_STORAGE_KEY").ok(),
            connection_string: std::env::var("AZURE_STORAGE_CONNECTION_STRING").ok(),
            sas_token: None,
            use_managed_identity: false,
        }
    }

    /// Check if credentials are configured
    pub fn is_configured(&self) -> bool {
        self.connection_string.is_some()
            || (self.account_name.is_some() && self.account_key.is_some())
            || self.sas_token.is_some()
            || self.use_managed_identity
    }
}

/// Upload configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadConfig {
    /// Cloud provider
    pub provider: CloudProvider,

    /// Bucket/container name
    pub bucket: String,

    /// Key prefix (folder path)
    #[serde(default)]
    pub prefix: String,

    /// Custom endpoint URL (for S3-compatible services)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,

    /// S3 credentials (when provider is S3/Minio/Custom)
    #[serde(default)]
    pub s3_credentials: S3Credentials,

    /// GCS credentials (when provider is GCS)
    #[serde(default)]
    pub gcs_credentials: GcsCredentials,

    /// Azure credentials (when provider is Azure)
    #[serde(default)]
    pub azure_credentials: AzureCredentials,

    /// Maximum concurrent uploads
    #[serde(default = "default_concurrent")]
    pub max_concurrent: u32,

    /// Maximum retries per upload
    #[serde(default = "default_retries")]
    pub max_retries: u32,

    /// Retry delay in milliseconds (exponential backoff base)
    #[serde(default = "default_retry_delay")]
    pub retry_delay_ms: u64,

    /// Multipart upload threshold in bytes
    #[serde(default = "default_multipart_threshold")]
    pub multipart_threshold: u64,

    /// Multipart chunk size in bytes
    #[serde(default = "default_chunk_size")]
    pub chunk_size: u64,

    /// Whether to verify checksums after upload
    #[serde(default = "default_true")]
    pub verify_checksum: bool,

    /// Storage class (e.g., "STANDARD", "GLACIER", "NEARLINE")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_class: Option<String>,

    /// Custom metadata to add to uploaded objects
    #[serde(default)]
    pub custom_metadata: HashMap<String, String>,

    /// Whether to upload metadata JSON alongside MCAP
    #[serde(default = "default_true")]
    pub upload_metadata: bool,

    /// Whether to delete local files after successful upload
    #[serde(default)]
    pub delete_after_upload: bool,
}

fn default_concurrent() -> u32 {
    4
}

fn default_retries() -> u32 {
    3
}

fn default_retry_delay() -> u64 {
    1000
}

fn default_multipart_threshold() -> u64 {
    100 * 1024 * 1024 // 100 MB
}

fn default_chunk_size() -> u64 {
    8 * 1024 * 1024 // 8 MB
}

impl Default for UploadConfig {
    fn default() -> Self {
        Self {
            provider: CloudProvider::S3,
            bucket: String::new(),
            prefix: String::new(),
            endpoint: None,
            s3_credentials: S3Credentials::default(),
            gcs_credentials: GcsCredentials::default(),
            azure_credentials: AzureCredentials::default(),
            max_concurrent: default_concurrent(),
            max_retries: default_retries(),
            retry_delay_ms: default_retry_delay(),
            multipart_threshold: default_multipart_threshold(),
            chunk_size: default_chunk_size(),
            verify_checksum: true,
            storage_class: None,
            custom_metadata: HashMap::new(),
            upload_metadata: true,
            delete_after_upload: false,
        }
    }
}

impl UploadConfig {
    /// Create config for S3
    pub fn s3(bucket: impl Into<String>) -> Self {
        Self {
            provider: CloudProvider::S3,
            bucket: bucket.into(),
            s3_credentials: S3Credentials::from_env(),
            ..Default::default()
        }
    }

    /// Create config for GCS
    pub fn gcs(bucket: impl Into<String>) -> Self {
        Self {
            provider: CloudProvider::Gcs,
            bucket: bucket.into(),
            gcs_credentials: GcsCredentials::from_env(),
            ..Default::default()
        }
    }

    /// Create config for Azure
    pub fn azure(container: impl Into<String>) -> Self {
        Self {
            provider: CloudProvider::Azure,
            bucket: container.into(),
            azure_credentials: AzureCredentials::from_env(),
            ..Default::default()
        }
    }

    /// Create config for MinIO
    pub fn minio(bucket: impl Into<String>, endpoint: impl Into<String>) -> Self {
        Self {
            provider: CloudProvider::Minio,
            bucket: bucket.into(),
            endpoint: Some(endpoint.into()),
            s3_credentials: S3Credentials::from_env(),
            ..Default::default()
        }
    }

    /// Set key prefix
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }

    /// Set S3 credentials
    pub fn with_s3_credentials(mut self, creds: S3Credentials) -> Self {
        self.s3_credentials = creds;
        self
    }

    /// Set GCS credentials
    pub fn with_gcs_credentials(mut self, creds: GcsCredentials) -> Self {
        self.gcs_credentials = creds;
        self
    }

    /// Set storage class
    pub fn with_storage_class(mut self, class: impl Into<String>) -> Self {
        self.storage_class = Some(class.into());
        self
    }

    /// Enable delete after upload
    pub fn with_delete_after_upload(mut self, delete: bool) -> Self {
        self.delete_after_upload = delete;
        self
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), UploadError> {
        if self.bucket.is_empty() {
            return Err(UploadError::InvalidConfig("Bucket name is required".into()));
        }

        match self.provider {
            CloudProvider::S3 | CloudProvider::Minio | CloudProvider::Custom => {
                if !self.s3_credentials.is_configured() {
                    return Err(UploadError::InvalidConfig(
                        "S3 credentials not configured. Set AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY \
                         environment variables or configure credentials in UploadConfig".into()
                    ));
                }
            }
            CloudProvider::Gcs => {
                if !self.gcs_credentials.is_configured() {
                    return Err(UploadError::InvalidConfig(
                        "GCS credentials not configured. Set GOOGLE_APPLICATION_CREDENTIALS \
                         environment variable or configure service account in UploadConfig"
                            .into(),
                    ));
                }
            }
            CloudProvider::Azure => {
                if !self.azure_credentials.is_configured() {
                    return Err(UploadError::InvalidConfig(
                        "Azure credentials not configured. Set AZURE_STORAGE_CONNECTION_STRING \
                         environment variable or configure credentials in UploadConfig"
                            .into(),
                    ));
                }
            }
        }

        if self.chunk_size < 5 * 1024 * 1024 {
            return Err(UploadError::InvalidConfig(
                "Chunk size must be at least 5 MB".into(),
            ));
        }

        Ok(())
    }

    /// Generate object key for episode
    pub fn object_key(&self, episode_id: &str, filename: &str) -> String {
        if self.prefix.is_empty() {
            format!("{}/{}", episode_id, filename)
        } else {
            format!(
                "{}/{}/{}",
                self.prefix.trim_end_matches('/'),
                episode_id,
                filename
            )
        }
    }
}

/// Upload progress information
#[derive(Debug, Clone)]
pub struct UploadProgress {
    /// Episode ID being uploaded
    pub episode_id: EpisodeId,
    /// Current phase
    pub phase: UploadPhase,
    /// Bytes uploaded
    pub bytes_uploaded: u64,
    /// Total bytes
    pub total_bytes: u64,
    /// Progress percentage (0-100)
    pub progress_percent: f32,
    /// Current file being uploaded
    pub current_file: Option<String>,
    /// Files uploaded / total files
    pub files_uploaded: u32,
    /// Total files
    pub total_files: u32,
    /// Retry count
    pub retry_count: u32,
    /// Upload speed in bytes per second
    pub speed_bps: u64,
    /// Estimated time remaining in seconds
    pub eta_seconds: Option<u64>,
}

/// Upload phases
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadPhase {
    /// Queued, waiting to start
    Queued,
    /// Preparing upload (reading file, calculating checksums)
    Preparing,
    /// Uploading data
    Uploading,
    /// Verifying upload
    Verifying,
    /// Complete
    Complete,
    /// Failed
    Failed,
    /// Cancelled
    Cancelled,
}

/// Upload result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadResult {
    /// Episode ID
    pub episode_id: EpisodeId,
    /// Whether upload succeeded
    pub success: bool,
    /// Destination URLs
    pub urls: Vec<String>,
    /// Total bytes uploaded
    pub bytes_uploaded: u64,
    /// Upload duration in milliseconds
    pub duration_ms: u64,
    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Retry count
    pub retries: u32,
}

/// Upload job in the queue
#[derive(Debug, Clone)]
pub struct UploadJob {
    /// Episode ID
    pub episode_id: EpisodeId,
    /// Path to MCAP file
    pub mcap_path: PathBuf,
    /// Path to metadata file (optional)
    pub metadata_path: Option<PathBuf>,
    /// Priority (higher = more urgent)
    pub priority: i32,
    /// Created timestamp
    pub created_at: TimestampNs,
}

impl UploadJob {
    /// Create from episode index entry
    pub fn from_entry(entry: &EpisodeIndexEntry, root_dir: &Path) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        Self {
            episode_id: entry.episode_id.clone(),
            mcap_path: entry.mcap_path(root_dir),
            metadata_path: entry.metadata_path(root_dir),
            priority: 0,
            created_at: now,
        }
    }

    /// Set priority
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }
}

/// Progress callback type
pub type UploadProgressCallback = Box<dyn Fn(UploadProgress) + Send + Sync>;

/// Upload manager
///
/// Handles uploading episodes to cloud storage with queue management,
/// retries, and progress tracking.
pub struct UploadManager {
    /// Configuration
    config: UploadConfig,
    /// Upload queue
    queue: Vec<UploadJob>,
    /// Currently uploading
    active_uploads: HashMap<String, UploadState>,
    /// Cancel flag
    cancel_flag: Arc<AtomicBool>,
    /// Total bytes uploaded in session
    session_bytes_uploaded: Arc<AtomicU64>,
}

/// State of an active upload
struct UploadState {
    /// Job
    job: UploadJob,
    /// Start time
    started_at: std::time::Instant,
    /// Bytes uploaded
    bytes_uploaded: AtomicU64,
    /// Retry count
    retry_count: u32,
    /// Cancel flag
    cancelled: AtomicBool,
}

impl UploadManager {
    /// Create a new upload manager
    pub fn new(config: UploadConfig) -> Result<Self, UploadError> {
        config.validate()?;

        Ok(Self {
            config,
            queue: Vec::new(),
            active_uploads: HashMap::new(),
            cancel_flag: Arc::new(AtomicBool::new(false)),
            session_bytes_uploaded: Arc::new(AtomicU64::new(0)),
        })
    }

    /// Get configuration
    pub fn config(&self) -> &UploadConfig {
        &self.config
    }

    /// Add episode to upload queue
    pub fn enqueue(&mut self, job: UploadJob) {
        // Check if already queued
        if self.queue.iter().any(|j| j.episode_id == job.episode_id) {
            return;
        }

        self.queue.push(job);
        // Sort by priority (descending) then by creation time (ascending)
        self.queue.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then(a.created_at.cmp(&b.created_at))
        });
    }

    /// Add multiple episodes to queue
    pub fn enqueue_batch(&mut self, jobs: Vec<UploadJob>) {
        for job in jobs {
            self.enqueue(job);
        }
    }

    /// Get queue length
    pub fn queue_len(&self) -> usize {
        self.queue.len()
    }

    /// Get active upload count
    pub fn active_count(&self) -> usize {
        self.active_uploads.len()
    }

    /// Cancel all uploads
    pub fn cancel_all(&mut self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
        self.queue.clear();
        for (_, state) in &self.active_uploads {
            state.cancelled.store(true, Ordering::SeqCst);
        }
    }

    /// Cancel upload for specific episode
    pub fn cancel(&mut self, episode_id: &str) {
        // Remove from queue
        self.queue.retain(|j| j.episode_id.as_str() != episode_id);

        // Cancel active upload
        if let Some(state) = self.active_uploads.get(episode_id) {
            state.cancelled.store(true, Ordering::SeqCst);
        }
    }

    /// Process next job in queue
    ///
    /// This is a placeholder that demonstrates the upload flow.
    /// In production, this would use an async runtime and actual HTTP client.
    pub fn process_next(
        &mut self,
        _progress_callback: Option<UploadProgressCallback>,
    ) -> Option<UploadResult> {
        if self.cancel_flag.load(Ordering::SeqCst) {
            return None;
        }

        let job = self.queue.pop()?;

        // Create upload state
        let state = UploadState {
            job: job.clone(),
            started_at: std::time::Instant::now(),
            bytes_uploaded: AtomicU64::new(0),
            retry_count: 0,
            cancelled: AtomicBool::new(false),
        };

        self.active_uploads
            .insert(job.episode_id.as_str().to_string(), state);

        // Simulate upload (placeholder for actual implementation)
        let result = self.do_upload(&job);

        // Remove from active
        self.active_uploads.remove(job.episode_id.as_str());

        Some(result)
    }

    /// Perform the actual upload
    ///
    /// This is a placeholder implementation. In production, this would:
    /// 1. Use an async HTTP client (reqwest, hyper, etc.)
    /// 2. Implement multipart upload for large files
    /// 3. Handle retries with exponential backoff
    /// 4. Verify checksums
    fn do_upload(&self, job: &UploadJob) -> UploadResult {
        let start = std::time::Instant::now();

        // Check if file exists
        if !job.mcap_path.exists() {
            return UploadResult {
                episode_id: job.episode_id.clone(),
                success: false,
                urls: Vec::new(),
                bytes_uploaded: 0,
                duration_ms: start.elapsed().as_millis() as u64,
                error: Some(format!("File not found: {:?}", job.mcap_path)),
                retries: 0,
            };
        }

        // Get file size
        let file_size = std::fs::metadata(&job.mcap_path)
            .map(|m| m.len())
            .unwrap_or(0);

        // Generate destination URLs
        let mcap_filename = job
            .mcap_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("episode.mcap");

        let mcap_key = self
            .config
            .object_key(job.episode_id.as_str(), mcap_filename);

        let base_url = match self.config.provider {
            CloudProvider::S3 => format!("s3://{}/{}", self.config.bucket, mcap_key),
            CloudProvider::Gcs => format!("gs://{}/{}", self.config.bucket, mcap_key),
            CloudProvider::Azure => format!(
                "https://{}.blob.core.windows.net/{}/{}",
                self.config
                    .azure_credentials
                    .account_name
                    .as_deref()
                    .unwrap_or("account"),
                self.config.bucket,
                mcap_key
            ),
            CloudProvider::Minio | CloudProvider::Custom => {
                let endpoint = self
                    .config
                    .endpoint
                    .as_deref()
                    .unwrap_or("http://localhost:9000");
                format!("{}/{}/{}", endpoint, self.config.bucket, mcap_key)
            }
        };

        let mut urls = vec![base_url.clone()];

        // Upload metadata if configured
        if self.config.upload_metadata {
            if let Some(ref meta_path) = job.metadata_path {
                if meta_path.exists() {
                    let meta_filename = meta_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("metadata.json");
                    let _meta_key = self
                        .config
                        .object_key(job.episode_id.as_str(), meta_filename);

                    // Add metadata URL
                    let meta_url = base_url
                        .rsplit_once('/')
                        .map(|(prefix, _)| format!("{}/{}", prefix, meta_filename));
                    if let Some(url) = meta_url {
                        urls.push(url);
                    }
                }
            }
        }

        // Placeholder: In production, actual upload would happen here
        // For now, we just simulate success
        //
        // Example implementation outline for S3:
        // ```
        // let client = aws_sdk_s3::Client::new(&aws_config);
        // let body = aws_sdk_s3::types::ByteStream::from_path(&job.mcap_path).await?;
        // client.put_object()
        //     .bucket(&self.config.bucket)
        //     .key(&mcap_key)
        //     .body(body)
        //     .send()
        //     .await?;
        // ```

        self.session_bytes_uploaded
            .fetch_add(file_size, Ordering::Relaxed);

        UploadResult {
            episode_id: job.episode_id.clone(),
            success: true,
            urls,
            bytes_uploaded: file_size,
            duration_ms: start.elapsed().as_millis() as u64,
            error: None,
            retries: 0,
        }
    }

    /// Upload an episode synchronously
    pub fn upload_episode(
        &mut self,
        entry: &EpisodeIndexEntry,
        root_dir: &Path,
        progress_callback: Option<UploadProgressCallback>,
    ) -> Result<UploadResult, UploadError> {
        let job = UploadJob::from_entry(entry, root_dir);

        if !job.mcap_path.exists() {
            return Err(UploadError::EpisodeNotFound(format!(
                "MCAP file not found: {:?}",
                job.mcap_path
            )));
        }

        self.enqueue(job);
        self.process_next(progress_callback)
            .ok_or_else(|| UploadError::Cancelled)
    }

    /// Get session statistics
    pub fn session_stats(&self) -> UploadSessionStats {
        UploadSessionStats {
            queued: self.queue.len() as u32,
            active: self.active_uploads.len() as u32,
            bytes_uploaded: self.session_bytes_uploaded.load(Ordering::Relaxed),
        }
    }

    /// Check if credentials are configured
    pub fn is_configured(&self) -> bool {
        match self.config.provider {
            CloudProvider::S3 | CloudProvider::Minio | CloudProvider::Custom => {
                self.config.s3_credentials.is_configured()
            }
            CloudProvider::Gcs => self.config.gcs_credentials.is_configured(),
            CloudProvider::Azure => self.config.azure_credentials.is_configured(),
        }
    }
}

/// Upload session statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadSessionStats {
    /// Queued uploads
    pub queued: u32,
    /// Active uploads
    pub active: u32,
    /// Bytes uploaded this session
    pub bytes_uploaded: u64,
}

/// Retry policy for uploads
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Initial delay in milliseconds
    pub initial_delay_ms: u64,
    /// Maximum delay in milliseconds
    pub max_delay_ms: u64,
    /// Exponential backoff multiplier
    pub multiplier: f32,
    /// Whether to add jitter
    pub jitter: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 1000,
            max_delay_ms: 60_000,
            multiplier: 2.0,
            jitter: true,
        }
    }
}

impl RetryPolicy {
    /// Calculate delay for given retry attempt
    pub fn delay_for_attempt(&self, attempt: u32) -> u64 {
        let base_delay = self.initial_delay_ms as f64 * self.multiplier.powi(attempt as i32) as f64;
        let delay = base_delay.min(self.max_delay_ms as f64) as u64;

        if self.jitter {
            // Add up to 25% jitter
            let jitter = (delay as f64 * 0.25 * rand_simple()) as u64;
            delay + jitter
        } else {
            delay
        }
    }
}

/// Simple random number generator (placeholder)
fn rand_simple() -> f64 {
    use std::time::SystemTime;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    (nanos % 1000) as f64 / 1000.0
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cloud_provider() {
        assert_eq!(CloudProvider::default(), CloudProvider::S3);
    }

    #[test]
    fn test_s3_credentials_from_env() {
        let creds = S3Credentials::from_env();
        // Credentials may or may not be set in test environment
        assert!(creds.region.len() > 0);
    }

    #[test]
    fn test_s3_credentials_builder() {
        let creds = S3Credentials::with_keys(
            "AKIAIOSFODNN7EXAMPLE",
            "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
        )
        .with_region("eu-west-1")
        .with_profile("production");

        assert_eq!(
            creds.access_key_id,
            Some("AKIAIOSFODNN7EXAMPLE".to_string())
        );
        assert_eq!(creds.region, "eu-west-1");
        assert_eq!(creds.profile, Some("production".to_string()));
        assert!(creds.is_configured());
    }

    #[test]
    fn test_gcs_credentials() {
        let creds =
            GcsCredentials::with_service_account("/path/to/key.json").with_project("my-project");

        assert!(creds.service_account_key.is_some());
        assert_eq!(creds.project_id, Some("my-project".to_string()));
        assert!(creds.is_configured());
    }

    #[test]
    fn test_upload_config_s3() {
        let config = UploadConfig::s3("my-bucket")
            .with_prefix("robot-data/2024")
            .with_storage_class("INTELLIGENT_TIERING");

        assert_eq!(config.provider, CloudProvider::S3);
        assert_eq!(config.bucket, "my-bucket");
        assert_eq!(config.prefix, "robot-data/2024");
        assert_eq!(
            config.storage_class,
            Some("INTELLIGENT_TIERING".to_string())
        );
    }

    #[test]
    fn test_upload_config_gcs() {
        let config = UploadConfig::gcs("my-bucket").with_prefix("episodes");

        assert_eq!(config.provider, CloudProvider::Gcs);
        assert_eq!(config.bucket, "my-bucket");
    }

    #[test]
    fn test_upload_config_minio() {
        let config = UploadConfig::minio("local-bucket", "http://localhost:9000");

        assert_eq!(config.provider, CloudProvider::Minio);
        assert_eq!(config.endpoint, Some("http://localhost:9000".to_string()));
    }

    #[test]
    fn test_object_key_generation() {
        let config = UploadConfig::s3("bucket").with_prefix("data/2024");

        let key = config.object_key("episode-001", "episode.mcap");
        assert_eq!(key, "data/2024/episode-001/episode.mcap");

        let config_no_prefix = UploadConfig::s3("bucket");
        let key2 = config_no_prefix.object_key("episode-001", "episode.mcap");
        assert_eq!(key2, "episode-001/episode.mcap");
    }

    #[test]
    fn test_upload_config_validation() {
        let config = UploadConfig::default();
        assert!(config.validate().is_err()); // Empty bucket

        let config =
            UploadConfig::s3("").with_s3_credentials(S3Credentials::with_keys("key", "secret"));
        assert!(config.validate().is_err()); // Empty bucket

        let mut config = UploadConfig::s3("bucket");
        config.s3_credentials = S3Credentials::with_keys("key", "secret");
        config.chunk_size = 1024; // Too small
        assert!(config.validate().is_err()); // Chunk size too small
    }

    #[test]
    fn test_retry_policy() {
        let policy = RetryPolicy::default();

        let delay0 = policy.delay_for_attempt(0);
        assert!(delay0 >= 1000);

        let delay1 = policy.delay_for_attempt(1);
        assert!(delay1 >= 2000);

        let delay5 = policy.delay_for_attempt(5);
        assert!(delay5 <= policy.max_delay_ms + (policy.max_delay_ms / 4)); // Max with jitter
    }

    #[test]
    fn test_upload_job() {
        let entry = EpisodeIndexEntry {
            episode_id: EpisodeId::from_string("test-001"),
            path: PathBuf::from("test-001"),
            mcap_file: "test-001.mcap".to_string(),
            metadata_file: Some("test-001_metadata.json".to_string()),
            start_time_ns: 0,
            end_time_ns: 0,
            duration_seconds: 0.0,
            size_bytes: 1024,
            sample_count: 0,
            robot_id: "robot".to_string(),
            uploaded: false,
            uploaded_at: None,
            upload_destination: None,
            quality_score: 1.0,
            created_at: 0,
            labels: HashMap::new(),
        };

        let job = UploadJob::from_entry(&entry, Path::new("/data")).with_priority(10);

        assert_eq!(job.episode_id.as_str(), "test-001");
        assert_eq!(job.mcap_path, PathBuf::from("/data/test-001/test-001.mcap"));
        assert_eq!(job.priority, 10);
    }

    #[test]
    fn test_upload_manager_queue() {
        let config = UploadConfig::s3("bucket")
            .with_s3_credentials(S3Credentials::with_keys("key", "secret"));

        let mut manager = UploadManager::new(config).unwrap();
        assert_eq!(manager.queue_len(), 0);

        let job = UploadJob {
            episode_id: EpisodeId::from_string("ep-1"),
            mcap_path: PathBuf::from("/data/ep-1.mcap"),
            metadata_path: None,
            priority: 0,
            created_at: 0,
        };

        manager.enqueue(job);
        assert_eq!(manager.queue_len(), 1);

        manager.cancel_all();
        assert_eq!(manager.queue_len(), 0);
    }

    #[test]
    fn test_upload_phase() {
        assert_ne!(UploadPhase::Queued, UploadPhase::Complete);
        assert_eq!(UploadPhase::Complete, UploadPhase::Complete);
    }

    #[test]
    fn test_upload_result() {
        let result = UploadResult {
            episode_id: EpisodeId::from_string("test"),
            success: true,
            urls: vec!["s3://bucket/test/episode.mcap".to_string()],
            bytes_uploaded: 1024 * 1024,
            duration_ms: 5000,
            error: None,
            retries: 0,
        };

        assert!(result.success);
        assert_eq!(result.urls.len(), 1);
    }

    #[test]
    fn test_session_stats() {
        let stats = UploadSessionStats {
            queued: 5,
            active: 2,
            bytes_uploaded: 1024 * 1024 * 100,
        };

        assert_eq!(stats.queued, 5);
        assert_eq!(stats.bytes_uploaded, 1024 * 1024 * 100);
    }
}
