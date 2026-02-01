//! MCAP file format writer for robot data
//!
//! MCAP is a container format for heterogeneous timestamped data,
//! designed for robotics applications and compatible with Foxglove Studio.
//!
//! This module provides a writer implementation that creates MCAP files
//! from episode data.
//!
//! See: https://mcap.dev/

use super::types::{DataSample, StreamId, TimestampNs};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{self, Write};
use thiserror::Error;

/// MCAP-related errors
#[derive(Debug, Error)]
pub enum McapError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Schema not registered: {0}")]
    SchemaNotRegistered(String),

    #[error("Channel not registered: {0}")]
    ChannelNotRegistered(String),

    #[error("Invalid state: {0}")]
    InvalidState(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Compression error: {0}")]
    CompressionError(String),
}

/// MCAP magic bytes
const MCAP_MAGIC: &[u8] = &[0x89, b'M', b'C', b'A', b'P', 0x30, b'\r', b'\n'];

/// MCAP record opcodes
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum Opcode {
    Header = 0x01,
    Footer = 0x02,
    Schema = 0x03,
    Channel = 0x04,
    Message = 0x05,
    Chunk = 0x06,
    MessageIndex = 0x07,
    ChunkIndex = 0x08,
    Attachment = 0x09,
    AttachmentIndex = 0x0A,
    Statistics = 0x0B,
    Metadata = 0x0C,
    MetadataIndex = 0x0D,
    SummaryOffset = 0x0E,
    DataEnd = 0x0F,
}

/// Schema definition for a message type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    /// Unique schema ID
    pub id: u16,
    /// Schema name (e.g., "sensor_msgs/Image")
    pub name: String,
    /// Encoding (e.g., "ros2msg", "protobuf", "jsonschema")
    pub encoding: String,
    /// Schema data (depends on encoding)
    pub data: Vec<u8>,
}

impl Schema {
    /// Create a new schema
    pub fn new(id: u16, name: impl Into<String>, encoding: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            encoding: encoding.into(),
            data: Vec::new(),
        }
    }

    /// Set schema data
    pub fn with_data(mut self, data: Vec<u8>) -> Self {
        self.data = data;
        self
    }

    /// Create a JSON schema
    pub fn json(id: u16, name: impl Into<String>, json_schema: &str) -> Self {
        Self {
            id,
            name: name.into(),
            encoding: "jsonschema".into(),
            data: json_schema.as_bytes().to_vec(),
        }
    }
}

/// Channel definition (a named stream of messages)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    /// Unique channel ID
    pub id: u16,
    /// Schema ID (0 for schemaless)
    pub schema_id: u16,
    /// Topic name
    pub topic: String,
    /// Message encoding (e.g., "json", "protobuf", "cdr")
    pub message_encoding: String,
    /// Channel metadata
    pub metadata: HashMap<String, String>,
}

impl Channel {
    /// Create a new channel
    pub fn new(
        id: u16,
        schema_id: u16,
        topic: impl Into<String>,
        message_encoding: impl Into<String>,
    ) -> Self {
        Self {
            id,
            schema_id,
            topic: topic.into(),
            message_encoding: message_encoding.into(),
            metadata: HashMap::new(),
        }
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// MCAP writer configuration
#[derive(Debug, Clone)]
pub struct McapWriterConfig {
    /// Profile name
    pub profile: String,
    /// Library name
    pub library: String,
    /// Whether to use chunking
    pub use_chunking: bool,
    /// Chunk size in bytes (if chunking enabled)
    pub chunk_size: usize,
    /// Compression for chunks
    pub compression: McapCompression,
    /// Whether to create message index
    pub create_message_index: bool,
}

impl Default for McapWriterConfig {
    fn default() -> Self {
        Self {
            profile: "ros2".into(),
            library: "omniedge-robotics".into(),
            use_chunking: true,
            chunk_size: 4 * 1024 * 1024, // 4 MB chunks
            compression: McapCompression::Zstd,
            create_message_index: true,
        }
    }
}

/// Compression options for MCAP chunks
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McapCompression {
    /// No compression
    None,
    /// Zstandard compression
    Zstd,
    /// LZ4 compression
    Lz4,
}

impl McapCompression {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "",
            Self::Zstd => "zstd",
            Self::Lz4 => "lz4",
        }
    }
}

/// Statistics tracked during writing
#[derive(Debug, Clone, Default)]
pub struct McapStatistics {
    /// Number of messages written
    pub message_count: u64,
    /// Number of chunks written
    pub chunk_count: u32,
    /// Number of attachments
    pub attachment_count: u32,
    /// Total uncompressed size
    pub uncompressed_size: u64,
    /// Total compressed size
    pub compressed_size: u64,
    /// Message count per channel
    pub channel_message_counts: HashMap<u16, u64>,
    /// First message timestamp
    pub message_start_time: TimestampNs,
    /// Last message timestamp
    pub message_end_time: TimestampNs,
}

/// MCAP file writer
///
/// Writes MCAP format files compatible with Foxglove Studio.
pub struct McapWriter<W: Write> {
    /// Underlying writer
    writer: W,
    /// Configuration
    config: McapWriterConfig,
    /// Registered schemas
    schemas: HashMap<u16, Schema>,
    /// Registered channels
    channels: HashMap<u16, Channel>,
    /// Stream ID to channel ID mapping
    stream_to_channel: HashMap<StreamId, u16>,
    /// Next schema ID
    next_schema_id: u16,
    /// Next channel ID
    next_channel_id: u16,
    /// Current chunk buffer (if chunking)
    chunk_buffer: Vec<u8>,
    /// Message indices for current chunk
    message_indices: Vec<MessageIndexEntry>,
    /// Chunk indices
    chunk_indices: Vec<ChunkIndexEntry>,
    /// Statistics
    stats: McapStatistics,
    /// Current state
    state: WriterState,
    /// Bytes written to file
    bytes_written: u64,
}

/// Writer state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WriterState {
    /// Initial state, header not written
    Initial,
    /// Header written, can write data
    Writing,
    /// Finished, footer written
    Finished,
}

/// Message index entry
#[derive(Debug, Clone)]
struct MessageIndexEntry {
    timestamp: TimestampNs,
    offset: u64,
}

/// Chunk index entry
#[derive(Debug, Clone)]
struct ChunkIndexEntry {
    start_time: TimestampNs,
    end_time: TimestampNs,
    offset: u64,
    chunk_length: u64,
    message_index_offset: u64,
    message_index_length: u64,
    compression: String,
    compressed_size: u64,
    uncompressed_size: u64,
}

impl<W: Write> McapWriter<W> {
    /// Create a new MCAP writer
    pub fn new(writer: W, config: McapWriterConfig) -> Self {
        Self {
            writer,
            config,
            schemas: HashMap::new(),
            channels: HashMap::new(),
            stream_to_channel: HashMap::new(),
            next_schema_id: 1,
            next_channel_id: 1,
            chunk_buffer: Vec::new(),
            message_indices: Vec::new(),
            chunk_indices: Vec::new(),
            stats: McapStatistics::default(),
            state: WriterState::Initial,
            bytes_written: 0,
        }
    }

    /// Start writing (write header)
    pub fn start(&mut self) -> Result<(), McapError> {
        if self.state != WriterState::Initial {
            return Err(McapError::InvalidState("Already started".into()));
        }

        // Write magic
        self.write_bytes(MCAP_MAGIC)?;

        // Write header record
        self.write_header()?;

        self.state = WriterState::Writing;
        Ok(())
    }

    /// Register a schema
    pub fn register_schema(&mut self, schema: Schema) -> Result<u16, McapError> {
        let id = schema.id;
        self.schemas.insert(id, schema.clone());

        if self.state == WriterState::Writing {
            self.write_schema(&schema)?;
        }

        Ok(id)
    }

    /// Register a schema and return a new ID
    pub fn add_schema(
        &mut self,
        name: impl Into<String>,
        encoding: impl Into<String>,
        data: Vec<u8>,
    ) -> Result<u16, McapError> {
        let id = self.next_schema_id;
        self.next_schema_id += 1;

        let schema = Schema {
            id,
            name: name.into(),
            encoding: encoding.into(),
            data,
        };

        self.register_schema(schema)
    }

    /// Register a channel
    pub fn register_channel(&mut self, channel: Channel) -> Result<u16, McapError> {
        let id = channel.id;

        // Verify schema exists (unless schemaless)
        if channel.schema_id != 0 && !self.schemas.contains_key(&channel.schema_id) {
            return Err(McapError::SchemaNotRegistered(format!(
                "Schema {} not found",
                channel.schema_id
            )));
        }

        self.channels.insert(id, channel.clone());

        if self.state == WriterState::Writing {
            self.write_channel(&channel)?;
        }

        Ok(id)
    }

    /// Add a channel and return a new ID
    pub fn add_channel(
        &mut self,
        schema_id: u16,
        topic: impl Into<String>,
        message_encoding: impl Into<String>,
    ) -> Result<u16, McapError> {
        let id = self.next_channel_id;
        self.next_channel_id += 1;

        let channel = Channel::new(id, schema_id, topic, message_encoding);
        self.register_channel(channel)
    }

    /// Register a stream with a channel
    pub fn register_stream(
        &mut self,
        stream_id: StreamId,
        channel_id: u16,
    ) -> Result<(), McapError> {
        if !self.channels.contains_key(&channel_id) {
            return Err(McapError::ChannelNotRegistered(format!(
                "Channel {} not found",
                channel_id
            )));
        }

        self.stream_to_channel.insert(stream_id, channel_id);
        Ok(())
    }

    /// Write a message
    pub fn write_message(
        &mut self,
        channel_id: u16,
        timestamp: TimestampNs,
        data: &[u8],
    ) -> Result<(), McapError> {
        if self.state != WriterState::Writing {
            return Err(McapError::InvalidState(
                "Writer not in writing state".into(),
            ));
        }

        if !self.channels.contains_key(&channel_id) {
            return Err(McapError::ChannelNotRegistered(format!(
                "Channel {} not found",
                channel_id
            )));
        }

        // Update statistics
        self.stats.message_count += 1;
        *self
            .stats
            .channel_message_counts
            .entry(channel_id)
            .or_insert(0) += 1;

        if self.stats.message_start_time == 0 || timestamp < self.stats.message_start_time {
            self.stats.message_start_time = timestamp;
        }
        if timestamp > self.stats.message_end_time {
            self.stats.message_end_time = timestamp;
        }

        if self.config.use_chunking {
            // Write to chunk buffer
            let offset = self.chunk_buffer.len() as u64;
            self.write_message_to_buffer(channel_id, timestamp, data)?;

            if self.config.create_message_index {
                self.message_indices
                    .push(MessageIndexEntry { timestamp, offset });
            }

            // Flush chunk if it's large enough
            if self.chunk_buffer.len() >= self.config.chunk_size {
                self.flush_chunk()?;
            }
        } else {
            // Write directly
            self.write_message_record(channel_id, timestamp, data)?;
        }

        Ok(())
    }

    /// Write a data sample
    pub fn write_sample(&mut self, sample: &DataSample) -> Result<(), McapError> {
        let channel_id = self
            .stream_to_channel
            .get(&sample.stream_id)
            .copied()
            .ok_or_else(|| {
                McapError::ChannelNotRegistered(format!(
                    "Stream {} not registered",
                    sample.stream_id.as_str()
                ))
            })?;

        let data = sample.data.to_bytes();
        self.write_message(channel_id, sample.timestamp_ns, &data)
    }

    /// Add an attachment (e.g., URDF file, calibration data)
    pub fn add_attachment(
        &mut self,
        name: impl Into<String>,
        media_type: impl Into<String>,
        data: &[u8],
    ) -> Result<(), McapError> {
        if self.state != WriterState::Writing {
            return Err(McapError::InvalidState(
                "Writer not in writing state".into(),
            ));
        }

        // Flush any pending chunk first
        if self.config.use_chunking && !self.chunk_buffer.is_empty() {
            self.flush_chunk()?;
        }

        self.write_attachment_record(&name.into(), &media_type.into(), data)?;
        self.stats.attachment_count += 1;

        Ok(())
    }

    /// Add metadata
    pub fn add_metadata(
        &mut self,
        name: impl Into<String>,
        metadata: &HashMap<String, String>,
    ) -> Result<(), McapError> {
        if self.state != WriterState::Writing {
            return Err(McapError::InvalidState(
                "Writer not in writing state".into(),
            ));
        }

        self.write_metadata_record(&name.into(), metadata)?;
        Ok(())
    }

    /// Finish writing (write footer and close)
    pub fn finish(mut self) -> Result<McapStatistics, McapError> {
        if self.state != WriterState::Writing {
            return Err(McapError::InvalidState(
                "Writer not in writing state".into(),
            ));
        }

        // Flush any pending chunk
        if self.config.use_chunking && !self.chunk_buffer.is_empty() {
            self.flush_chunk()?;
        }

        // Write data end marker
        self.write_data_end()?;

        // Write summary section (statistics, indices)
        self.write_summary()?;

        // Write footer
        self.write_footer()?;

        // Write trailing magic
        self.write_bytes(MCAP_MAGIC)?;

        self.state = WriterState::Finished;
        Ok(self.stats)
    }

    /// Get current statistics
    pub fn stats(&self) -> &McapStatistics {
        &self.stats
    }

    // ========================================================================
    // Private methods
    // ========================================================================

    fn write_bytes(&mut self, data: &[u8]) -> Result<(), McapError> {
        self.writer.write_all(data)?;
        self.bytes_written += data.len() as u64;
        Ok(())
    }

    fn write_header(&mut self) -> Result<(), McapError> {
        let mut record = Vec::new();

        // Profile (prefixed string)
        write_prefixed_string(&mut record, &self.config.profile)?;
        // Library (prefixed string)
        write_prefixed_string(&mut record, &self.config.library)?;

        self.write_record(Opcode::Header, &record)
    }

    fn write_schema(&mut self, schema: &Schema) -> Result<(), McapError> {
        let mut record = Vec::new();

        // Schema ID (u16)
        record.extend_from_slice(&schema.id.to_le_bytes());
        // Name (prefixed string)
        write_prefixed_string(&mut record, &schema.name)?;
        // Encoding (prefixed string)
        write_prefixed_string(&mut record, &schema.encoding)?;
        // Data length (u32) + data
        record.extend_from_slice(&(schema.data.len() as u32).to_le_bytes());
        record.extend_from_slice(&schema.data);

        self.write_record(Opcode::Schema, &record)
    }

    fn write_channel(&mut self, channel: &Channel) -> Result<(), McapError> {
        let mut record = Vec::new();

        // Channel ID (u16)
        record.extend_from_slice(&channel.id.to_le_bytes());
        // Schema ID (u16)
        record.extend_from_slice(&channel.schema_id.to_le_bytes());
        // Topic (prefixed string)
        write_prefixed_string(&mut record, &channel.topic)?;
        // Message encoding (prefixed string)
        write_prefixed_string(&mut record, &channel.message_encoding)?;
        // Metadata (prefixed map)
        write_prefixed_map(&mut record, &channel.metadata)?;

        self.write_record(Opcode::Channel, &record)
    }

    fn write_message_record(
        &mut self,
        channel_id: u16,
        timestamp: TimestampNs,
        data: &[u8],
    ) -> Result<(), McapError> {
        let mut record = Vec::new();

        // Channel ID (u16)
        record.extend_from_slice(&channel_id.to_le_bytes());
        // Sequence (u32) - we use message count
        record.extend_from_slice(&(self.stats.message_count as u32).to_le_bytes());
        // Log time (u64)
        record.extend_from_slice(&timestamp.to_le_bytes());
        // Publish time (u64) - same as log time
        record.extend_from_slice(&timestamp.to_le_bytes());
        // Data
        record.extend_from_slice(data);

        self.write_record(Opcode::Message, &record)
    }

    fn write_message_to_buffer(
        &mut self,
        channel_id: u16,
        timestamp: TimestampNs,
        data: &[u8],
    ) -> Result<(), McapError> {
        // Write message record to chunk buffer
        let mut record = Vec::new();

        // Channel ID (u16)
        record.extend_from_slice(&channel_id.to_le_bytes());
        // Sequence (u32)
        record.extend_from_slice(&(self.stats.message_count as u32).to_le_bytes());
        // Log time (u64)
        record.extend_from_slice(&timestamp.to_le_bytes());
        // Publish time (u64)
        record.extend_from_slice(&timestamp.to_le_bytes());
        // Data
        record.extend_from_slice(data);

        // Write record header to buffer
        self.chunk_buffer.push(Opcode::Message as u8);
        self.chunk_buffer
            .extend_from_slice(&(record.len() as u64).to_le_bytes());
        self.chunk_buffer.extend_from_slice(&record);

        Ok(())
    }

    fn flush_chunk(&mut self) -> Result<(), McapError> {
        if self.chunk_buffer.is_empty() {
            return Ok(());
        }

        let uncompressed_size = self.chunk_buffer.len() as u64;
        let start_time = self
            .message_indices
            .first()
            .map(|e| e.timestamp)
            .unwrap_or(0);
        let end_time = self
            .message_indices
            .last()
            .map(|e| e.timestamp)
            .unwrap_or(0);

        // Compress chunk data
        let (compressed_data, compression_name) = match self.config.compression {
            McapCompression::None => (self.chunk_buffer.clone(), ""),
            McapCompression::Zstd | McapCompression::Lz4 => {
                // For now, use no compression (placeholder)
                // In production, use actual zstd/lz4 compression
                (self.chunk_buffer.clone(), "")
            }
        };

        let compressed_size = compressed_data.len() as u64;
        let chunk_offset = self.bytes_written;

        // Write chunk record
        let mut record = Vec::new();

        // Message start time (u64)
        record.extend_from_slice(&start_time.to_le_bytes());
        // Message end time (u64)
        record.extend_from_slice(&end_time.to_le_bytes());
        // Uncompressed size (u64)
        record.extend_from_slice(&uncompressed_size.to_le_bytes());
        // Uncompressed CRC (u32) - placeholder
        record.extend_from_slice(&0u32.to_le_bytes());
        // Compression (prefixed string)
        write_prefixed_string(&mut record, compression_name)?;
        // Compressed size (u64)
        record.extend_from_slice(&compressed_size.to_le_bytes());
        // Compressed data
        record.extend_from_slice(&compressed_data);

        self.write_record(Opcode::Chunk, &record)?;

        let chunk_length = self.bytes_written - chunk_offset;

        // Write message indices
        let message_index_offset = self.bytes_written;
        if self.config.create_message_index {
            self.write_message_indices()?;
        }
        let message_index_length = self.bytes_written - message_index_offset;

        // Record chunk index
        self.chunk_indices.push(ChunkIndexEntry {
            start_time,
            end_time,
            offset: chunk_offset,
            chunk_length,
            message_index_offset,
            message_index_length,
            compression: compression_name.into(),
            compressed_size,
            uncompressed_size,
        });

        // Update stats
        self.stats.chunk_count += 1;
        self.stats.uncompressed_size += uncompressed_size;
        self.stats.compressed_size += compressed_size;

        // Clear buffers
        self.chunk_buffer.clear();
        self.message_indices.clear();

        Ok(())
    }

    fn write_message_indices(&mut self) -> Result<(), McapError> {
        // Group indices by channel
        // For simplicity, we'll skip detailed message indices in this implementation
        // In production, this would write proper MessageIndex records
        Ok(())
    }

    fn write_attachment_record(
        &mut self,
        name: &str,
        media_type: &str,
        data: &[u8],
    ) -> Result<(), McapError> {
        let mut record = Vec::new();

        // Log time (u64)
        record.extend_from_slice(&0u64.to_le_bytes());
        // Create time (u64)
        record.extend_from_slice(&0u64.to_le_bytes());
        // Name (prefixed string)
        write_prefixed_string(&mut record, name)?;
        // Media type (prefixed string)
        write_prefixed_string(&mut record, media_type)?;
        // Data size (u64)
        record.extend_from_slice(&(data.len() as u64).to_le_bytes());
        // Data
        record.extend_from_slice(data);
        // CRC (u32) - placeholder
        record.extend_from_slice(&0u32.to_le_bytes());

        self.write_record(Opcode::Attachment, &record)
    }

    fn write_metadata_record(
        &mut self,
        name: &str,
        metadata: &HashMap<String, String>,
    ) -> Result<(), McapError> {
        let mut record = Vec::new();

        // Name (prefixed string)
        write_prefixed_string(&mut record, name)?;
        // Metadata (prefixed map)
        write_prefixed_map(&mut record, metadata)?;

        self.write_record(Opcode::Metadata, &record)
    }

    fn write_data_end(&mut self) -> Result<(), McapError> {
        let mut record = Vec::new();
        // Data section CRC (u32) - placeholder
        record.extend_from_slice(&0u32.to_le_bytes());
        self.write_record(Opcode::DataEnd, &record)
    }

    fn write_summary(&mut self) -> Result<(), McapError> {
        // Write statistics
        self.write_statistics()?;

        // Write chunk indices
        for index in &self.chunk_indices.clone() {
            self.write_chunk_index(index)?;
        }

        Ok(())
    }

    fn write_statistics(&mut self) -> Result<(), McapError> {
        let mut record = Vec::new();

        // Message count (u64)
        record.extend_from_slice(&self.stats.message_count.to_le_bytes());
        // Schema count (u16)
        record.extend_from_slice(&(self.schemas.len() as u16).to_le_bytes());
        // Channel count (u32)
        record.extend_from_slice(&(self.channels.len() as u32).to_le_bytes());
        // Attachment count (u32)
        record.extend_from_slice(&self.stats.attachment_count.to_le_bytes());
        // Metadata count (u32) - placeholder
        record.extend_from_slice(&0u32.to_le_bytes());
        // Chunk count (u32)
        record.extend_from_slice(&self.stats.chunk_count.to_le_bytes());
        // Message start time (u64)
        record.extend_from_slice(&self.stats.message_start_time.to_le_bytes());
        // Message end time (u64)
        record.extend_from_slice(&self.stats.message_end_time.to_le_bytes());
        // Channel message counts (prefixed array)
        record.extend_from_slice(&(self.stats.channel_message_counts.len() as u32).to_le_bytes());
        for (channel_id, count) in &self.stats.channel_message_counts {
            record.extend_from_slice(&channel_id.to_le_bytes());
            record.extend_from_slice(&count.to_le_bytes());
        }

        self.write_record(Opcode::Statistics, &record)
    }

    fn write_chunk_index(&mut self, index: &ChunkIndexEntry) -> Result<(), McapError> {
        let mut record = Vec::new();

        // Message start time (u64)
        record.extend_from_slice(&index.start_time.to_le_bytes());
        // Message end time (u64)
        record.extend_from_slice(&index.end_time.to_le_bytes());
        // Chunk start offset (u64)
        record.extend_from_slice(&index.offset.to_le_bytes());
        // Chunk length (u64)
        record.extend_from_slice(&index.chunk_length.to_le_bytes());
        // Message index offsets (empty for now)
        record.extend_from_slice(&0u32.to_le_bytes());
        // Message index length (u64)
        record.extend_from_slice(&index.message_index_length.to_le_bytes());
        // Compression (prefixed string)
        write_prefixed_string(&mut record, &index.compression)?;
        // Compressed size (u64)
        record.extend_from_slice(&index.compressed_size.to_le_bytes());
        // Uncompressed size (u64)
        record.extend_from_slice(&index.uncompressed_size.to_le_bytes());

        self.write_record(Opcode::ChunkIndex, &record)
    }

    fn write_footer(&mut self) -> Result<(), McapError> {
        let mut record = Vec::new();

        // Summary start (u64) - offset where summary begins
        record.extend_from_slice(&0u64.to_le_bytes());
        // Summary offset start (u64)
        record.extend_from_slice(&0u64.to_le_bytes());
        // Summary CRC (u32)
        record.extend_from_slice(&0u32.to_le_bytes());

        self.write_record(Opcode::Footer, &record)
    }

    fn write_record(&mut self, opcode: Opcode, data: &[u8]) -> Result<(), McapError> {
        // Opcode (1 byte)
        self.write_bytes(&[opcode as u8])?;
        // Record length (8 bytes, little-endian)
        self.write_bytes(&(data.len() as u64).to_le_bytes())?;
        // Record data
        self.write_bytes(data)?;
        Ok(())
    }
}

// ============================================================================
// Helper functions
// ============================================================================

fn write_prefixed_string<W: Write>(writer: &mut W, s: &str) -> Result<(), McapError> {
    let bytes = s.as_bytes();
    writer.write_all(&(bytes.len() as u32).to_le_bytes())?;
    writer.write_all(bytes)?;
    Ok(())
}

fn write_prefixed_map<W: Write>(
    writer: &mut W,
    map: &HashMap<String, String>,
) -> Result<(), McapError> {
    // Calculate total size
    let mut size: u32 = 0;
    for (k, v) in map {
        size += 4 + k.len() as u32 + 4 + v.len() as u32;
    }

    writer.write_all(&size.to_le_bytes())?;

    for (k, v) in map {
        write_prefixed_string(writer, k)?;
        write_prefixed_string(writer, v)?;
    }

    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcap_writer_creation() {
        let buffer = Vec::new();
        let config = McapWriterConfig::default();
        let writer = McapWriter::new(buffer, config);

        assert_eq!(writer.state, WriterState::Initial);
    }

    #[test]
    fn test_mcap_writer_start() {
        let buffer = Vec::new();
        let config = McapWriterConfig::default();
        let mut writer = McapWriter::new(buffer, config);

        writer.start().unwrap();
        assert_eq!(writer.state, WriterState::Writing);
    }

    #[test]
    fn test_mcap_writer_schema() {
        let buffer = Vec::new();
        let config = McapWriterConfig::default();
        let mut writer = McapWriter::new(buffer, config);

        writer.start().unwrap();

        let schema_id = writer
            .add_schema("test/Message", "jsonschema", b"{}".to_vec())
            .unwrap();

        assert_eq!(schema_id, 1);
        assert!(writer.schemas.contains_key(&1));
    }

    #[test]
    fn test_mcap_writer_channel() {
        let buffer = Vec::new();
        let config = McapWriterConfig::default();
        let mut writer = McapWriter::new(buffer, config);

        writer.start().unwrap();

        let schema_id = writer
            .add_schema("test/Message", "jsonschema", b"{}".to_vec())
            .unwrap();

        let channel_id = writer
            .add_channel(schema_id, "/test/topic", "json")
            .unwrap();

        assert_eq!(channel_id, 1);
        assert!(writer.channels.contains_key(&1));
    }

    #[test]
    fn test_mcap_writer_message() {
        let buffer = Vec::new();
        let config = McapWriterConfig::default();
        let mut writer = McapWriter::new(buffer, config);

        writer.start().unwrap();

        let schema_id = writer
            .add_schema("test/Message", "jsonschema", b"{}".to_vec())
            .unwrap();

        let channel_id = writer
            .add_channel(schema_id, "/test/topic", "json")
            .unwrap();

        writer
            .write_message(channel_id, 1000000000, b"test data")
            .unwrap();

        assert_eq!(writer.stats.message_count, 1);
    }

    #[test]
    fn test_mcap_writer_finish() {
        let buffer = Vec::new();
        let config = McapWriterConfig::default();
        let mut writer = McapWriter::new(buffer, config);

        writer.start().unwrap();

        let schema_id = writer
            .add_schema("test/Message", "jsonschema", b"{}".to_vec())
            .unwrap();

        let channel_id = writer
            .add_channel(schema_id, "/test/topic", "json")
            .unwrap();

        writer
            .write_message(channel_id, 1000000000, b"test data")
            .unwrap();

        let stats = writer.finish().unwrap();

        assert_eq!(stats.message_count, 1);
    }

    #[test]
    fn test_schema_creation() {
        let schema = Schema::new(1, "sensor_msgs/Image", "ros2msg");
        assert_eq!(schema.id, 1);
        assert_eq!(schema.name, "sensor_msgs/Image");
        assert_eq!(schema.encoding, "ros2msg");
    }

    #[test]
    fn test_channel_creation() {
        let channel =
            Channel::new(1, 1, "/camera/image", "cdr").with_metadata("callerid", "omniedge");

        assert_eq!(channel.id, 1);
        assert_eq!(channel.topic, "/camera/image");
        assert!(channel.metadata.contains_key("callerid"));
    }

    #[test]
    fn test_mcap_compression_enum() {
        assert_eq!(McapCompression::None.as_str(), "");
        assert_eq!(McapCompression::Zstd.as_str(), "zstd");
        assert_eq!(McapCompression::Lz4.as_str(), "lz4");
    }
}
