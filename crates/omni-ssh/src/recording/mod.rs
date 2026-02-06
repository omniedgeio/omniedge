//! Session recording for audit and compliance
//!
//! This module provides session recording functionality for SSH sessions,
//! producing recordings in asciinema cast v2 format with OmniEdge extensions.
//!
//! ## Features
//!
//! - **Asciinema v2 format**: Compatible with asciinema player
//! - **Multiple writers**: File and cloud upload support
//! - **Integrity protection**: HMAC-signed chunks for tamper detection
//! - **Input/output separation**: Separate recording of terminal I/O
//!
//! ## Recording Format
//!
//! Recordings use the [asciinema cast format v2](https://github.com/asciinema/asciinema/blob/master/doc/asciicast-v2.md):
//! ```json
//! {"version": 2, "width": 80, "height": 24, "timestamp": 1234567890, ...}
//! [0.123456, "o", "Hello "]
//! [0.234567, "o", "World\n"]
//! [1.345678, "i", "ls -la\n"]
//! ```

use chrono::{DateTime, Utc};
use ring::digest::{Context, SHA256};
use ring::hmac;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::{debug, error, info, warn};

/// Recording configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingConfig {
    /// Enable session recording
    #[serde(default)]
    pub enabled: bool,
    /// Local recording directory (fallback)
    pub local_dir: Option<PathBuf>,
    /// Cloud recording upload URL
    pub cloud_url: Option<String>,
    /// Chunk size in bytes for upload
    #[serde(default = "default_chunk_size")]
    pub chunk_size: usize,
    /// Whether to record input (keystrokes)
    #[serde(default)]
    pub record_input: bool,
    /// HMAC key for integrity protection (base64 encoded)
    pub integrity_key: Option<String>,
}

fn default_chunk_size() -> usize {
    65536 // 64KB
}

impl Default for RecordingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            local_dir: None,
            cloud_url: None,
            chunk_size: default_chunk_size(),
            record_input: false,
            integrity_key: None,
        }
    }
}

/// Asciinema cast header (v2 format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CastHeader {
    /// Format version (always 2)
    pub version: u8,
    /// Terminal width
    pub width: u32,
    /// Terminal height
    pub height: u32,
    /// Start timestamp (Unix epoch)
    pub timestamp: i64,
    /// Recording duration (set on finalize)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    /// Command that was run
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Recording title
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Environment variables
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub env: HashMap<String, String>,

    // OmniEdge extensions
    /// SSH username
    #[serde(
        rename = "omniedge.ssh_user",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub ssh_user: Option<String>,
    /// Local system user
    #[serde(
        rename = "omniedge.local_user",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub local_user: Option<String>,
    /// Source node ID
    #[serde(
        rename = "omniedge.src_node_id",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub src_node_id: Option<String>,
    /// Connection ID
    #[serde(
        rename = "omniedge.connection_id",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub connection_id: Option<String>,
}

impl Default for CastHeader {
    fn default() -> Self {
        Self {
            version: 2,
            width: 80,
            height: 24,
            timestamp: Utc::now().timestamp(),
            duration: None,
            command: None,
            title: None,
            env: HashMap::new(),
            ssh_user: None,
            local_user: None,
            src_node_id: None,
            connection_id: None,
        }
    }
}

/// Single recording entry (asciinema format)
/// Format: [time, type, data]
/// - time: seconds since start
/// - type: "o" for output, "i" for input
/// - data: the text
#[derive(Serialize)]
struct CastEntry(f64, &'static str, String);

/// Session recorder
pub struct SessionRecorder {
    /// Session ID
    session_id: String,
    /// Start time (for elapsed calculation)
    start_time: std::time::Instant,
    /// Start timestamp (for header)
    start_timestamp: DateTime<Utc>,
    /// Terminal width
    width: u32,
    /// Terminal height
    height: u32,
    /// Recording writers
    writers: Vec<Box<dyn RecordingWriter>>,
    /// Whether header has been written
    header_written: bool,
    /// Total bytes recorded
    bytes_recorded: u64,
    /// Entry count
    entry_count: u64,
}

impl SessionRecorder {
    /// Create a new session recorder
    pub fn new(session_id: String, width: u32, height: u32) -> Self {
        Self {
            session_id,
            start_time: std::time::Instant::now(),
            start_timestamp: Utc::now(),
            width,
            height,
            writers: Vec::new(),
            header_written: false,
            bytes_recorded: 0,
            entry_count: 0,
        }
    }

    /// Add a recording writer
    pub fn add_writer(&mut self, writer: Box<dyn RecordingWriter>) {
        self.writers.push(writer);
    }

    /// Get session ID
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Get recording duration
    pub fn duration(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }

    /// Get bytes recorded
    pub fn bytes_recorded(&self) -> u64 {
        self.bytes_recorded
    }

    /// Get entry count
    pub fn entry_count(&self) -> u64 {
        self.entry_count
    }

    /// Create a default cast header
    pub fn create_header(&self) -> CastHeader {
        CastHeader {
            version: 2,
            width: self.width,
            height: self.height,
            timestamp: self.start_timestamp.timestamp(),
            duration: None,
            command: None,
            title: Some(format!("OmniEdge SSH Session {}", self.session_id)),
            env: HashMap::new(),
            ssh_user: None,
            local_user: None,
            src_node_id: None,
            connection_id: Some(self.session_id.clone()),
        }
    }

    /// Write cast header
    pub async fn write_header(&mut self, header: CastHeader) -> anyhow::Result<()> {
        let json = serde_json::to_string(&header)?;

        for writer in &mut self.writers {
            writer.write_line(&json).await?;
        }

        self.header_written = true;
        info!(
            "Recording started for session {} ({}x{})",
            self.session_id, self.width, self.height
        );

        Ok(())
    }

    /// Record terminal output
    pub async fn record_output(&mut self, data: &[u8]) -> anyhow::Result<()> {
        if !self.header_written {
            return Err(anyhow::anyhow!("Header not written"));
        }

        let elapsed = self.start_time.elapsed().as_secs_f64();
        let text = String::from_utf8_lossy(data);
        let entry = CastEntry(elapsed, "o", text.to_string());
        let json = serde_json::to_string(&entry)?;

        for writer in &mut self.writers {
            writer.write_line(&json).await?;
        }

        self.bytes_recorded += data.len() as u64;
        self.entry_count += 1;

        Ok(())
    }

    /// Record terminal input (keystrokes)
    pub async fn record_input(&mut self, data: &[u8]) -> anyhow::Result<()> {
        if !self.header_written {
            return Err(anyhow::anyhow!("Header not written"));
        }

        let elapsed = self.start_time.elapsed().as_secs_f64();
        let text = String::from_utf8_lossy(data);
        let entry = CastEntry(elapsed, "i", text.to_string());
        let json = serde_json::to_string(&entry)?;

        for writer in &mut self.writers {
            writer.write_line(&json).await?;
        }

        self.bytes_recorded += data.len() as u64;
        self.entry_count += 1;

        Ok(())
    }

    /// Record window resize event
    pub async fn record_resize(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        if !self.header_written {
            return Err(anyhow::anyhow!("Header not written"));
        }

        self.width = width;
        self.height = height;

        let elapsed = self.start_time.elapsed().as_secs_f64();
        // Use a custom event type for resize (asciinema extension)
        let resize_data = format!("{}x{}", width, height);
        let entry = CastEntry(elapsed, "r", resize_data);
        let json = serde_json::to_string(&entry)?;

        for writer in &mut self.writers {
            writer.write_line(&json).await?;
        }

        self.entry_count += 1;

        Ok(())
    }

    /// Mark header as written (for external header writing)
    pub fn mark_header_written(&mut self) {
        self.header_written = true;
    }

    /// Finalize recording
    pub async fn finalize(&mut self) -> anyhow::Result<()> {
        let duration = self.start_time.elapsed();

        for writer in &mut self.writers {
            writer.finalize().await?;
        }

        info!(
            "Recording finalized for session {} - {} entries, {} bytes, {:.2}s",
            self.session_id,
            self.entry_count,
            self.bytes_recorded,
            duration.as_secs_f64()
        );

        Ok(())
    }
}

/// Recording writer trait
#[async_trait::async_trait]
pub trait RecordingWriter: Send + Sync {
    /// Write a line to the recording
    async fn write_line(&mut self, line: &str) -> anyhow::Result<()>;
    /// Finalize the recording
    async fn finalize(&mut self) -> anyhow::Result<()>;
}

/// File-based recording writer
pub struct FileRecordingWriter {
    /// File path
    path: PathBuf,
    /// File handle
    file: Option<File>,
}

impl FileRecordingWriter {
    /// Create a new file recording writer
    pub async fn new(path: PathBuf) -> anyhow::Result<Self> {
        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let file = File::create(&path).await?;

        Ok(Self {
            path,
            file: Some(file),
        })
    }

    /// Get the file path
    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

#[async_trait::async_trait]
impl RecordingWriter for FileRecordingWriter {
    async fn write_line(&mut self, line: &str) -> anyhow::Result<()> {
        if let Some(ref mut file) = self.file {
            file.write_all(line.as_bytes()).await?;
            file.write_all(b"\n").await?;
        }
        Ok(())
    }

    async fn finalize(&mut self) -> anyhow::Result<()> {
        if let Some(ref mut file) = self.file {
            file.flush().await?;
            file.sync_all().await?;
        }
        info!("Recording saved to: {}", self.path.display());
        Ok(())
    }
}

/// Cloud upload recording writer
pub struct CloudRecordingWriter {
    /// HTTP client
    client: reqwest::Client,
    /// Upload URL
    upload_url: String,
    /// Session ID
    session_id: String,
    /// Buffer for batching
    buffer: Vec<String>,
    /// Lines per chunk
    chunk_size: usize,
    /// Current chunk number
    chunk_num: u32,
    /// API token for authentication
    api_token: Option<String>,
}

impl CloudRecordingWriter {
    /// Create a new cloud recording writer
    pub fn new(upload_url: String, session_id: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            upload_url,
            session_id,
            buffer: Vec::new(),
            chunk_size: 100, // Lines per chunk
            chunk_num: 0,
            api_token: None,
        }
    }

    /// Set chunk size (lines per upload)
    pub fn with_chunk_size(mut self, size: usize) -> Self {
        self.chunk_size = size;
        self
    }

    /// Set API token for authentication
    pub fn with_api_token(mut self, token: String) -> Self {
        self.api_token = Some(token);
        self
    }

    /// Flush buffered data to cloud
    async fn flush_buffer(&mut self) -> anyhow::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        let data = self.buffer.join("\n");
        self.chunk_num += 1;

        let mut request = self
            .client
            .post(&self.upload_url)
            .header("Content-Type", "application/x-ndjson")
            .header("X-Session-ID", &self.session_id)
            .header("X-Chunk-Number", self.chunk_num.to_string())
            .body(data);

        if let Some(ref token) = self.api_token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }

        match request.send().await {
            Ok(response) => {
                if !response.status().is_success() {
                    warn!(
                        "Cloud upload chunk {} failed: {}",
                        self.chunk_num,
                        response.status()
                    );
                } else {
                    debug!("Cloud upload chunk {} success", self.chunk_num);
                }
            }
            Err(e) => {
                warn!("Cloud upload chunk {} error: {}", self.chunk_num, e);
                // Don't fail the recording for upload errors
            }
        }

        self.buffer.clear();
        Ok(())
    }
}

#[async_trait::async_trait]
impl RecordingWriter for CloudRecordingWriter {
    async fn write_line(&mut self, line: &str) -> anyhow::Result<()> {
        self.buffer.push(line.to_string());

        if self.buffer.len() >= self.chunk_size {
            self.flush_buffer().await?;
        }

        Ok(())
    }

    async fn finalize(&mut self) -> anyhow::Result<()> {
        // Flush any remaining data
        self.flush_buffer().await?;

        // Send finalization signal
        let mut request = self
            .client
            .post(&self.upload_url)
            .header("Content-Type", "application/json")
            .header("X-Session-ID", &self.session_id)
            .header("X-Finalize", "true")
            .body("{}");

        if let Some(ref token) = self.api_token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }

        match request.send().await {
            Ok(response) => {
                if response.status().is_success() {
                    info!("Recording upload finalized for session {}", self.session_id);
                } else {
                    warn!(
                        "Recording finalization failed for session {}: {}",
                        self.session_id,
                        response.status()
                    );
                }
            }
            Err(e) => {
                warn!(
                    "Recording finalization error for session {}: {}",
                    self.session_id, e
                );
            }
        }

        Ok(())
    }
}

/// Signed recording chunk for tamper protection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedRecordingChunk {
    /// Chunk sequence number
    pub chunk_num: u32,
    /// Recording data (base64 encoded)
    pub data: String,
    /// SHA256 hash of data
    pub sha256: String,
    /// HMAC signature (using device key)
    pub signature: String,
    /// Timestamp when chunk was created
    pub timestamp: DateTime<Utc>,
    /// Previous chunk's hash (for chain verification)
    pub prev_hash: Option<String>,
}

/// Recording integrity manager for tamper protection
pub struct RecordingIntegrity {
    /// HMAC key
    hmac_key: hmac::Key,
    /// Current chunk number
    chunk_num: u32,
    /// Previous chunk hash
    prev_hash: Option<String>,
}

impl RecordingIntegrity {
    /// Create a new integrity manager with a key
    pub fn new(key: &[u8]) -> Self {
        Self {
            hmac_key: hmac::Key::new(hmac::HMAC_SHA256, key),
            chunk_num: 0,
            prev_hash: None,
        }
    }

    /// Create from base64-encoded key
    pub fn from_base64_key(key_b64: &str) -> anyhow::Result<Self> {
        let key = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, key_b64)?;
        Ok(Self::new(&key))
    }

    /// Sign a chunk of recording data
    pub fn sign_chunk(&mut self, data: &str) -> SignedRecordingChunk {
        // Calculate SHA256 of data
        let mut sha_context = Context::new(&SHA256);
        sha_context.update(data.as_bytes());
        let sha256 = sha_context.finish();
        let sha256_b64 =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, sha256.as_ref());

        // Create signature data: chunk_num + sha256 + prev_hash
        let mut sig_data = Vec::new();
        sig_data.extend_from_slice(&self.chunk_num.to_le_bytes());
        sig_data.extend_from_slice(sha256.as_ref());
        if let Some(ref prev) = self.prev_hash {
            sig_data.extend_from_slice(prev.as_bytes());
        }

        // Sign with HMAC
        let signature = hmac::sign(&self.hmac_key, &sig_data);
        let signature_b64 = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            signature.as_ref(),
        );

        // Encode data
        let data_b64 =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, data.as_bytes());

        let chunk = SignedRecordingChunk {
            chunk_num: self.chunk_num,
            data: data_b64,
            sha256: sha256_b64.clone(),
            signature: signature_b64,
            timestamp: Utc::now(),
            prev_hash: self.prev_hash.clone(),
        };

        // Update state
        self.chunk_num += 1;
        self.prev_hash = Some(sha256_b64);

        chunk
    }

    /// Verify a signed chunk
    pub fn verify_chunk(
        key: &[u8],
        chunk: &SignedRecordingChunk,
        expected_prev_hash: Option<&str>,
    ) -> anyhow::Result<String> {
        // Decode data
        let data = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &chunk.data)?;

        // Verify SHA256
        let mut sha_context = Context::new(&SHA256);
        sha_context.update(&data);
        let sha256 = sha_context.finish();
        let computed_sha256 =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, sha256.as_ref());

        if computed_sha256 != chunk.sha256 {
            return Err(anyhow::anyhow!("SHA256 mismatch"));
        }

        // Verify chain (prev_hash)
        if chunk.prev_hash != expected_prev_hash.map(|s| s.to_string()) {
            return Err(anyhow::anyhow!("Chain hash mismatch"));
        }

        // Verify HMAC signature
        let hmac_key = hmac::Key::new(hmac::HMAC_SHA256, key);

        let mut sig_data = Vec::new();
        sig_data.extend_from_slice(&chunk.chunk_num.to_le_bytes());
        sig_data.extend_from_slice(sha256.as_ref());
        if let Some(ref prev) = chunk.prev_hash {
            sig_data.extend_from_slice(prev.as_bytes());
        }

        let expected_sig =
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &chunk.signature)?;
        hmac::verify(&hmac_key, &sig_data, &expected_sig)
            .map_err(|_| anyhow::anyhow!("HMAC signature verification failed"))?;

        // Return the decoded data
        Ok(String::from_utf8(data)?)
    }
}

/// Integrity-protected recording writer wrapper
pub struct IntegrityRecordingWriter {
    /// Inner writer
    inner: Box<dyn RecordingWriter>,
    /// Integrity manager
    integrity: RecordingIntegrity,
    /// Buffer for current chunk
    buffer: Vec<String>,
    /// Lines per chunk
    chunk_size: usize,
}

impl IntegrityRecordingWriter {
    /// Create a new integrity-protected writer
    pub fn new(inner: Box<dyn RecordingWriter>, key: &[u8], chunk_size: usize) -> Self {
        Self {
            inner,
            integrity: RecordingIntegrity::new(key),
            buffer: Vec::new(),
            chunk_size,
        }
    }

    /// Flush current buffer as a signed chunk
    async fn flush_chunk(&mut self) -> anyhow::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        let data = self.buffer.join("\n");
        let chunk = self.integrity.sign_chunk(&data);
        let json = serde_json::to_string(&chunk)?;

        self.inner.write_line(&json).await?;
        self.buffer.clear();

        Ok(())
    }
}

#[async_trait::async_trait]
impl RecordingWriter for IntegrityRecordingWriter {
    async fn write_line(&mut self, line: &str) -> anyhow::Result<()> {
        self.buffer.push(line.to_string());

        if self.buffer.len() >= self.chunk_size {
            self.flush_chunk().await?;
        }

        Ok(())
    }

    async fn finalize(&mut self) -> anyhow::Result<()> {
        self.flush_chunk().await?;
        self.inner.finalize().await
    }
}

/// Create a session recorder from configuration
pub async fn create_recorder(
    config: &RecordingConfig,
    session_id: String,
    width: u32,
    height: u32,
) -> anyhow::Result<SessionRecorder> {
    let mut recorder = SessionRecorder::new(session_id.clone(), width, height);

    // Add file writer if configured
    if let Some(ref dir) = config.local_dir {
        let filename = format!("{}.cast", session_id);
        let path = dir.join(filename);
        let writer = FileRecordingWriter::new(path).await?;
        recorder.add_writer(Box::new(writer));
    }

    // Add cloud writer if configured
    if let Some(ref url) = config.cloud_url {
        let writer = CloudRecordingWriter::new(url.clone(), session_id);
        recorder.add_writer(Box::new(writer));
    }

    Ok(recorder)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cast_header_serialization() {
        let header = CastHeader {
            version: 2,
            width: 80,
            height: 24,
            timestamp: 1234567890,
            duration: Some(10.5),
            command: Some("bash".to_string()),
            title: Some("Test Session".to_string()),
            env: {
                let mut env = HashMap::new();
                env.insert("TERM".to_string(), "xterm-256color".to_string());
                env
            },
            ssh_user: Some("testuser".to_string()),
            local_user: Some("root".to_string()),
            src_node_id: Some("node123".to_string()),
            connection_id: Some("conn456".to_string()),
        };

        let json = serde_json::to_string(&header).unwrap();
        assert!(json.contains("\"version\":2"));
        assert!(json.contains("\"width\":80"));
        assert!(json.contains("\"omniedge.ssh_user\":\"testuser\""));
    }

    #[test]
    fn test_cast_entry_serialization() {
        let entry = CastEntry(1.234, "o", "Hello World".to_string());
        let json = serde_json::to_string(&entry).unwrap();
        assert_eq!(json, r#"[1.234,"o","Hello World"]"#);
    }

    #[test]
    fn test_integrity_sign_verify() {
        let key = b"test-key-32-bytes-long-xxxxxxxx";
        let mut integrity = RecordingIntegrity::new(key);

        // Sign first chunk
        let chunk1 = integrity.sign_chunk("test data 1");
        assert_eq!(chunk1.chunk_num, 0);
        assert!(chunk1.prev_hash.is_none());

        // Sign second chunk (should have prev_hash)
        let chunk2 = integrity.sign_chunk("test data 2");
        assert_eq!(chunk2.chunk_num, 1);
        assert!(chunk2.prev_hash.is_some());
        assert_eq!(chunk2.prev_hash.as_ref(), Some(&chunk1.sha256));

        // Verify first chunk
        let data1 = RecordingIntegrity::verify_chunk(key, &chunk1, None).unwrap();
        assert_eq!(data1, "test data 1");

        // Verify second chunk
        let data2 = RecordingIntegrity::verify_chunk(key, &chunk2, Some(&chunk1.sha256)).unwrap();
        assert_eq!(data2, "test data 2");
    }
}
