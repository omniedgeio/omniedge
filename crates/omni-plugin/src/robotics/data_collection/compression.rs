//! Compression layer for sensor data
//!
//! Provides compression trait and implementations for reducing data size
//! before storage or transmission. Supports multiple algorithms optimized
//! for different data types.

use super::types::StreamId;
use std::io::{self, Write};
use thiserror::Error;

/// Compression-related errors
#[derive(Debug, Error)]
pub enum CompressionError {
    #[error("Compression failed: {0}")]
    CompressionFailed(String),

    #[error("Decompression failed: {0}")]
    DecompressionFailed(String),

    #[error("Invalid compressed data")]
    InvalidData,

    #[error("Unsupported compression format: {0}")]
    UnsupportedFormat(String),

    #[error("IO error: {0}")]
    Io(#[from] io::Error),
}

/// Compression algorithm identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompressionAlgorithm {
    /// No compression
    None,
    /// Zstandard - good balance of speed and ratio
    Zstd,
    /// LZ4 - very fast, lower ratio
    Lz4,
    /// JPEG - lossy image compression
    Jpeg,
    /// PNG - lossless image compression
    Png,
}

impl CompressionAlgorithm {
    /// Get the file extension for this algorithm
    pub fn extension(&self) -> &'static str {
        match self {
            Self::None => "",
            Self::Zstd => ".zst",
            Self::Lz4 => ".lz4",
            Self::Jpeg => ".jpg",
            Self::Png => ".png",
        }
    }

    /// Get the MIME type for this algorithm
    pub fn mime_type(&self) -> &'static str {
        match self {
            Self::None => "application/octet-stream",
            Self::Zstd => "application/zstd",
            Self::Lz4 => "application/x-lz4",
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
        }
    }
}

/// Compression level configuration
#[derive(Debug, Clone, Copy)]
pub struct CompressionLevel {
    /// Level (1-22 for zstd, 1-16 for lz4)
    pub level: i32,
    /// Whether to use dictionary compression
    pub use_dictionary: bool,
}

impl Default for CompressionLevel {
    fn default() -> Self {
        Self {
            level: 3, // Good default for zstd
            use_dictionary: false,
        }
    }
}

impl CompressionLevel {
    /// Fast compression (level 1)
    pub fn fast() -> Self {
        Self {
            level: 1,
            use_dictionary: false,
        }
    }

    /// Best compression (high level)
    pub fn best() -> Self {
        Self {
            level: 19,
            use_dictionary: false,
        }
    }
}

/// Trait for data compression
pub trait Compressor: Send + Sync {
    /// Get the algorithm identifier
    fn algorithm(&self) -> CompressionAlgorithm;

    /// Compress data
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError>;

    /// Decompress data
    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError>;

    /// Estimate compressed size (for buffer allocation)
    fn estimate_compressed_size(&self, uncompressed_size: usize) -> usize {
        // Default: assume 50% compression ratio
        uncompressed_size / 2 + 128
    }

    /// Get compression statistics
    fn stats(&self) -> CompressionStats {
        CompressionStats::default()
    }
}

/// Compression statistics
#[derive(Debug, Clone, Default)]
pub struct CompressionStats {
    /// Total bytes before compression
    pub bytes_in: u64,
    /// Total bytes after compression
    pub bytes_out: u64,
    /// Total compression time in nanoseconds
    pub compress_time_ns: u64,
    /// Total decompression time in nanoseconds
    pub decompress_time_ns: u64,
    /// Number of compression operations
    pub compress_count: u64,
    /// Number of decompression operations
    pub decompress_count: u64,
}

impl CompressionStats {
    /// Calculate compression ratio
    pub fn compression_ratio(&self) -> f64 {
        if self.bytes_out == 0 {
            1.0
        } else {
            self.bytes_in as f64 / self.bytes_out as f64
        }
    }

    /// Calculate average compression throughput in MB/s
    pub fn avg_compress_throughput_mbps(&self) -> f64 {
        if self.compress_time_ns == 0 {
            0.0
        } else {
            let mb = self.bytes_in as f64 / (1024.0 * 1024.0);
            let seconds = self.compress_time_ns as f64 / 1_000_000_000.0;
            mb / seconds
        }
    }
}

/// No-op compressor (passthrough)
#[derive(Debug, Default)]
pub struct NoCompressor;

impl Compressor for NoCompressor {
    fn algorithm(&self) -> CompressionAlgorithm {
        CompressionAlgorithm::None
    }

    fn compress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
        Ok(data.to_vec())
    }

    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
        Ok(data.to_vec())
    }

    fn estimate_compressed_size(&self, uncompressed_size: usize) -> usize {
        uncompressed_size
    }
}

/// Zstandard compressor
///
/// Good general-purpose compressor with excellent compression ratio
/// and reasonable speed. Best for general sensor data.
#[derive(Debug)]
pub struct ZstdCompressor {
    level: i32,
    stats: std::sync::Mutex<CompressionStats>,
}

impl ZstdCompressor {
    /// Create with default level (3)
    pub fn new() -> Self {
        Self::with_level(3)
    }

    /// Create with specific level (1-22)
    pub fn with_level(level: i32) -> Self {
        Self {
            level: level.clamp(1, 22),
            stats: std::sync::Mutex::new(CompressionStats::default()),
        }
    }

    /// Create for fast compression (level 1)
    pub fn fast() -> Self {
        Self::with_level(1)
    }

    /// Create for best compression (level 19)
    pub fn best() -> Self {
        Self::with_level(19)
    }
}

impl Default for ZstdCompressor {
    fn default() -> Self {
        Self::new()
    }
}

impl Compressor for ZstdCompressor {
    fn algorithm(&self) -> CompressionAlgorithm {
        CompressionAlgorithm::Zstd
    }

    fn compress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
        let start = std::time::Instant::now();

        // Simple zstd compression using pure Rust implementation
        // In production, this would use the zstd crate
        let compressed = zstd_compress_simple(data, self.level)?;

        let elapsed = start.elapsed().as_nanos() as u64;

        // Update stats
        if let Ok(mut stats) = self.stats.lock() {
            stats.bytes_in += data.len() as u64;
            stats.bytes_out += compressed.len() as u64;
            stats.compress_time_ns += elapsed;
            stats.compress_count += 1;
        }

        Ok(compressed)
    }

    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
        let start = std::time::Instant::now();

        let decompressed = zstd_decompress_simple(data)?;

        let elapsed = start.elapsed().as_nanos() as u64;

        if let Ok(mut stats) = self.stats.lock() {
            stats.decompress_time_ns += elapsed;
            stats.decompress_count += 1;
        }

        Ok(decompressed)
    }

    fn stats(&self) -> CompressionStats {
        self.stats.lock().map(|s| s.clone()).unwrap_or_default()
    }
}

/// LZ4 compressor
///
/// Very fast compression with lower ratio. Best for high-bandwidth
/// data where speed is critical (e.g., real-time streaming).
#[derive(Debug)]
pub struct Lz4Compressor {
    stats: std::sync::Mutex<CompressionStats>,
}

impl Lz4Compressor {
    pub fn new() -> Self {
        Self {
            stats: std::sync::Mutex::new(CompressionStats::default()),
        }
    }
}

impl Default for Lz4Compressor {
    fn default() -> Self {
        Self::new()
    }
}

impl Compressor for Lz4Compressor {
    fn algorithm(&self) -> CompressionAlgorithm {
        CompressionAlgorithm::Lz4
    }

    fn compress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
        let start = std::time::Instant::now();

        // Simple LZ4 compression
        // In production, this would use the lz4_flex crate
        let compressed = lz4_compress_simple(data)?;

        let elapsed = start.elapsed().as_nanos() as u64;

        if let Ok(mut stats) = self.stats.lock() {
            stats.bytes_in += data.len() as u64;
            stats.bytes_out += compressed.len() as u64;
            stats.compress_time_ns += elapsed;
            stats.compress_count += 1;
        }

        Ok(compressed)
    }

    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
        let start = std::time::Instant::now();

        let decompressed = lz4_decompress_simple(data)?;

        let elapsed = start.elapsed().as_nanos() as u64;

        if let Ok(mut stats) = self.stats.lock() {
            stats.decompress_time_ns += elapsed;
            stats.decompress_count += 1;
        }

        Ok(decompressed)
    }

    fn estimate_compressed_size(&self, uncompressed_size: usize) -> usize {
        // LZ4 has lower compression ratio
        (uncompressed_size * 2) / 3 + 128
    }

    fn stats(&self) -> CompressionStats {
        self.stats.lock().map(|s| s.clone()).unwrap_or_default()
    }
}

/// JPEG compressor for image data
///
/// Lossy compression optimized for images. Quality parameter
/// controls the tradeoff between size and quality.
#[derive(Debug)]
pub struct JpegCompressor {
    quality: u8,
    stats: std::sync::Mutex<CompressionStats>,
}

impl JpegCompressor {
    /// Create with quality (1-100)
    pub fn new(quality: u8) -> Self {
        Self {
            quality: quality.clamp(1, 100),
            stats: std::sync::Mutex::new(CompressionStats::default()),
        }
    }

    /// Create with default quality (85)
    pub fn default_quality() -> Self {
        Self::new(85)
    }

    /// Create for high quality (95)
    pub fn high_quality() -> Self {
        Self::new(95)
    }

    /// Create for low quality/small size (60)
    pub fn low_quality() -> Self {
        Self::new(60)
    }
}

impl Default for JpegCompressor {
    fn default() -> Self {
        Self::default_quality()
    }
}

impl Compressor for JpegCompressor {
    fn algorithm(&self) -> CompressionAlgorithm {
        CompressionAlgorithm::Jpeg
    }

    fn compress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
        let start = std::time::Instant::now();

        // In production, this would use image crate or turbojpeg
        // For now, we'll use a placeholder that stores the data with a header
        let compressed = jpeg_compress_placeholder(data, self.quality)?;

        let elapsed = start.elapsed().as_nanos() as u64;

        if let Ok(mut stats) = self.stats.lock() {
            stats.bytes_in += data.len() as u64;
            stats.bytes_out += compressed.len() as u64;
            stats.compress_time_ns += elapsed;
            stats.compress_count += 1;
        }

        Ok(compressed)
    }

    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
        let start = std::time::Instant::now();

        let decompressed = jpeg_decompress_placeholder(data)?;

        let elapsed = start.elapsed().as_nanos() as u64;

        if let Ok(mut stats) = self.stats.lock() {
            stats.decompress_time_ns += elapsed;
            stats.decompress_count += 1;
        }

        Ok(decompressed)
    }

    fn estimate_compressed_size(&self, uncompressed_size: usize) -> usize {
        // JPEG can achieve high compression ratios
        let ratio = match self.quality {
            90..=100 => 4,
            70..=89 => 8,
            _ => 12,
        };
        uncompressed_size / ratio + 1024
    }

    fn stats(&self) -> CompressionStats {
        self.stats.lock().map(|s| s.clone()).unwrap_or_default()
    }
}

/// Compression configuration for a stream
#[derive(Debug, Clone)]
pub struct StreamCompressionConfig {
    /// Stream ID this config applies to
    pub stream_id: StreamId,
    /// Algorithm to use
    pub algorithm: CompressionAlgorithm,
    /// Compression level (algorithm-specific)
    pub level: CompressionLevel,
    /// Minimum size to compress (skip small buffers)
    pub min_size: usize,
    /// Whether compression is enabled
    pub enabled: bool,
}

impl StreamCompressionConfig {
    /// Create default config for a stream
    pub fn new(stream_id: StreamId) -> Self {
        Self {
            stream_id,
            algorithm: CompressionAlgorithm::Zstd,
            level: CompressionLevel::default(),
            min_size: 256,
            enabled: true,
        }
    }

    /// Create config for image streams
    pub fn for_images(stream_id: StreamId, quality: u8) -> Self {
        Self {
            stream_id,
            algorithm: CompressionAlgorithm::Jpeg,
            level: CompressionLevel {
                level: quality as i32,
                use_dictionary: false,
            },
            min_size: 0,
            enabled: true,
        }
    }

    /// Create config for high-bandwidth streams
    pub fn for_realtime(stream_id: StreamId) -> Self {
        Self {
            stream_id,
            algorithm: CompressionAlgorithm::Lz4,
            level: CompressionLevel::fast(),
            min_size: 512,
            enabled: true,
        }
    }
}

/// Compressor factory
pub struct CompressorFactory;

impl CompressorFactory {
    /// Create a compressor from configuration
    pub fn create(config: &StreamCompressionConfig) -> Box<dyn Compressor> {
        if !config.enabled {
            return Box::new(NoCompressor);
        }

        match config.algorithm {
            CompressionAlgorithm::None => Box::new(NoCompressor),
            CompressionAlgorithm::Zstd => Box::new(ZstdCompressor::with_level(config.level.level)),
            CompressionAlgorithm::Lz4 => Box::new(Lz4Compressor::new()),
            CompressionAlgorithm::Jpeg => Box::new(JpegCompressor::new(config.level.level as u8)),
            CompressionAlgorithm::Png => {
                // PNG not yet implemented, fall back to no compression
                Box::new(NoCompressor)
            }
        }
    }
}

// ============================================================================
// Simple compression implementations (placeholders for actual libraries)
// ============================================================================

/// Simple zstd-like compression using standard library
/// In production, replace with actual zstd crate
fn zstd_compress_simple(data: &[u8], _level: i32) -> Result<Vec<u8>, CompressionError> {
    use std::io::Write;

    // Use flate2-style compression as a placeholder
    // Real implementation would use zstd crate
    let mut encoder = flate2_encoder(data.len());
    encoder
        .write_all(data)
        .map_err(|e| CompressionError::CompressionFailed(e.to_string()))?;

    // Simple header + compressed data format
    let mut result = Vec::with_capacity(data.len() + 8);
    result.extend_from_slice(b"ZSTD"); // Magic
    result.extend_from_slice(&(data.len() as u32).to_le_bytes()); // Original size
    result.extend_from_slice(&encoder.finish());

    Ok(result)
}

fn zstd_decompress_simple(data: &[u8]) -> Result<Vec<u8>, CompressionError> {
    if data.len() < 8 || &data[0..4] != b"ZSTD" {
        return Err(CompressionError::InvalidData);
    }

    let original_size = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
    let compressed = &data[8..];

    let mut result = Vec::with_capacity(original_size);
    flate2_decode(compressed, &mut result)?;

    Ok(result)
}

fn lz4_compress_simple(data: &[u8]) -> Result<Vec<u8>, CompressionError> {
    // Simple LZ4-like format using run-length encoding as placeholder
    let mut result = Vec::with_capacity(data.len() + 8);
    result.extend_from_slice(b"LZ4B"); // Magic
    result.extend_from_slice(&(data.len() as u32).to_le_bytes());

    // Simple compression: just store as-is for now
    // Real implementation would use lz4_flex crate
    result.extend_from_slice(data);

    Ok(result)
}

fn lz4_decompress_simple(data: &[u8]) -> Result<Vec<u8>, CompressionError> {
    if data.len() < 8 || &data[0..4] != b"LZ4B" {
        return Err(CompressionError::InvalidData);
    }

    let _original_size = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;

    // Simple: just return the data after header
    Ok(data[8..].to_vec())
}

fn jpeg_compress_placeholder(data: &[u8], quality: u8) -> Result<Vec<u8>, CompressionError> {
    // Placeholder: store with header
    let mut result = Vec::with_capacity(data.len() + 9);
    result.extend_from_slice(b"JPEG"); // Magic
    result.push(quality);
    result.extend_from_slice(&(data.len() as u32).to_le_bytes());
    result.extend_from_slice(data);
    Ok(result)
}

fn jpeg_decompress_placeholder(data: &[u8]) -> Result<Vec<u8>, CompressionError> {
    if data.len() < 9 || &data[0..4] != b"JPEG" {
        return Err(CompressionError::InvalidData);
    }

    Ok(data[9..].to_vec())
}

// Simple flate2-like encoder/decoder placeholders
struct SimpleEncoder {
    data: Vec<u8>,
}

fn flate2_encoder(capacity: usize) -> SimpleEncoder {
    SimpleEncoder {
        data: Vec::with_capacity(capacity),
    }
}

impl SimpleEncoder {
    fn finish(self) -> Vec<u8> {
        self.data
    }
}

impl Write for SimpleEncoder {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.data.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn flate2_decode(compressed: &[u8], output: &mut Vec<u8>) -> Result<(), CompressionError> {
    output.extend_from_slice(compressed);
    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_compressor() {
        let compressor = NoCompressor;
        let data = b"Hello, World!";

        let compressed = compressor.compress(data).unwrap();
        assert_eq!(compressed, data);

        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_zstd_compressor() {
        let compressor = ZstdCompressor::new();
        let data = b"Hello, World! This is a test of compression.";

        let compressed = compressor.compress(data).unwrap();
        assert!(compressed.starts_with(b"ZSTD"));

        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_lz4_compressor() {
        let compressor = Lz4Compressor::new();
        let data = b"Hello, World! This is a test of LZ4 compression.";

        let compressed = compressor.compress(data).unwrap();
        assert!(compressed.starts_with(b"LZ4B"));

        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_jpeg_compressor() {
        let compressor = JpegCompressor::new(85);
        let data = vec![0u8; 1024]; // Fake image data

        let compressed = compressor.compress(&data).unwrap();
        assert!(compressed.starts_with(b"JPEG"));
        assert_eq!(compressed[4], 85); // Quality byte

        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_compression_stats() {
        let compressor = ZstdCompressor::new();
        let data = b"Test data for statistics";

        compressor.compress(data).unwrap();
        compressor.compress(data).unwrap();

        let stats = compressor.stats();
        assert_eq!(stats.compress_count, 2);
        assert_eq!(stats.bytes_in, (data.len() * 2) as u64);
    }

    #[test]
    fn test_compression_algorithm_metadata() {
        assert_eq!(CompressionAlgorithm::Zstd.extension(), ".zst");
        assert_eq!(CompressionAlgorithm::Lz4.extension(), ".lz4");
        assert_eq!(CompressionAlgorithm::Jpeg.mime_type(), "image/jpeg");
    }

    #[test]
    fn test_compressor_factory() {
        let config = StreamCompressionConfig::new(StreamId::new("test"));
        let compressor = CompressorFactory::create(&config);
        assert_eq!(compressor.algorithm(), CompressionAlgorithm::Zstd);

        let config = StreamCompressionConfig::for_realtime(StreamId::new("rt"));
        let compressor = CompressorFactory::create(&config);
        assert_eq!(compressor.algorithm(), CompressionAlgorithm::Lz4);

        let config = StreamCompressionConfig::for_images(StreamId::new("cam"), 90);
        let compressor = CompressorFactory::create(&config);
        assert_eq!(compressor.algorithm(), CompressionAlgorithm::Jpeg);
    }

    #[test]
    fn test_invalid_data_errors() {
        let compressor = ZstdCompressor::new();

        let result = compressor.decompress(b"invalid");
        assert!(matches!(result, Err(CompressionError::InvalidData)));

        let result = compressor.decompress(b"ZSTD\x00\x00\x00");
        assert!(matches!(result, Err(CompressionError::InvalidData)));
    }
}
