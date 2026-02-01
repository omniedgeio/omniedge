//! Ring buffer implementation for sensor data
//!
//! Provides high-performance circular buffers for storing sensor data
//! with configurable capacity and time-based eviction.

use super::types::{DataSample, StreamId, TimestampNs};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Thread-safe ring buffer for sensor data
pub struct RingBuffer {
    /// Stream identifier
    stream_id: StreamId,
    /// Storage slots
    storage: Vec<parking_lot::RwLock<Option<DataSample>>>,
    /// Write position (wraps around)
    write_pos: AtomicU64,
    /// Total samples written (for sequence numbers)
    total_written: AtomicU64,
    /// Buffer capacity
    capacity: usize,
    /// Maximum age in nanoseconds
    max_age_ns: u64,
    /// Statistics
    stats: BufferStats,
}

/// Buffer statistics (atomically updated)
#[derive(Debug, Default)]
pub struct BufferStats {
    /// Total samples pushed
    pub samples_pushed: AtomicU64,
    /// Samples dropped due to buffer full
    pub samples_dropped: AtomicU64,
    /// Samples evicted due to age
    pub samples_evicted: AtomicU64,
    /// Total bytes written
    pub bytes_written: AtomicU64,
}

impl BufferStats {
    /// Get a snapshot of current stats
    pub fn snapshot(&self) -> BufferStatsSnapshot {
        BufferStatsSnapshot {
            samples_pushed: self.samples_pushed.load(Ordering::Relaxed),
            samples_dropped: self.samples_dropped.load(Ordering::Relaxed),
            samples_evicted: self.samples_evicted.load(Ordering::Relaxed),
            bytes_written: self.bytes_written.load(Ordering::Relaxed),
        }
    }
}

/// Snapshot of buffer statistics
#[derive(Debug, Clone, Default)]
pub struct BufferStatsSnapshot {
    pub samples_pushed: u64,
    pub samples_dropped: u64,
    pub samples_evicted: u64,
    pub bytes_written: u64,
}

impl RingBuffer {
    /// Create a new ring buffer
    ///
    /// # Arguments
    /// * `stream_id` - Stream identifier
    /// * `capacity` - Number of samples to buffer
    /// * `max_age_seconds` - Maximum age of samples in seconds
    pub fn new(stream_id: StreamId, capacity: usize, max_age_seconds: f32) -> Self {
        let storage = (0..capacity)
            .map(|_| parking_lot::RwLock::new(None))
            .collect();

        Self {
            stream_id,
            storage,
            write_pos: AtomicU64::new(0),
            total_written: AtomicU64::new(0),
            capacity,
            max_age_ns: (max_age_seconds * 1_000_000_000.0) as u64,
            stats: BufferStats::default(),
        }
    }

    /// Get stream ID
    pub fn stream_id(&self) -> &StreamId {
        &self.stream_id
    }

    /// Get buffer capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Push a sample into the buffer
    pub fn push(&self, mut sample: DataSample) {
        // Assign sequence number
        let seq = self.total_written.fetch_add(1, Ordering::SeqCst);
        sample.sequence = seq;

        // Calculate slot
        let pos = self.write_pos.fetch_add(1, Ordering::SeqCst);
        let slot = (pos as usize) % self.capacity;

        // Update stats
        let size = sample.size_bytes() as u64;
        self.stats.bytes_written.fetch_add(size, Ordering::Relaxed);
        self.stats.samples_pushed.fetch_add(1, Ordering::Relaxed);

        // Write to slot
        let mut guard = self.storage[slot].write();
        *guard = Some(sample);
    }

    /// Get samples in time range [start_ns, end_ns]
    pub fn get_range(&self, start_ns: TimestampNs, end_ns: TimestampNs) -> Vec<DataSample> {
        let mut results = Vec::new();

        for slot in &self.storage {
            let guard = slot.read();
            if let Some(sample) = guard.as_ref() {
                if sample.timestamp_ns >= start_ns && sample.timestamp_ns <= end_ns {
                    results.push(sample.clone());
                }
            }
        }

        // Sort by timestamp
        results.sort_by_key(|s| s.timestamp_ns);
        results
    }

    /// Get all samples newer than the given timestamp
    pub fn get_since(&self, since_ns: TimestampNs) -> Vec<DataSample> {
        self.get_range(since_ns, u64::MAX)
    }

    /// Get the N most recent samples
    pub fn get_latest(&self, count: usize) -> Vec<DataSample> {
        let mut results = Vec::with_capacity(count.min(self.capacity));

        // Collect all valid samples
        for slot in &self.storage {
            let guard = slot.read();
            if let Some(sample) = guard.as_ref() {
                results.push(sample.clone());
            }
        }

        // Sort by timestamp descending
        results.sort_by(|a, b| b.timestamp_ns.cmp(&a.timestamp_ns));

        // Take the most recent
        results.truncate(count);

        // Reverse to get chronological order
        results.reverse();
        results
    }

    /// Clear the buffer
    pub fn clear(&self) {
        for slot in &self.storage {
            let mut guard = slot.write();
            *guard = None;
        }
        self.write_pos.store(0, Ordering::SeqCst);
    }

    /// Get current buffer utilization (0.0 - 1.0)
    pub fn utilization(&self) -> f32 {
        let total = self.total_written.load(Ordering::Relaxed);
        let used = total.min(self.capacity as u64);
        used as f32 / self.capacity as f32
    }

    /// Get the time range covered by buffered data
    pub fn time_range(&self) -> Option<(TimestampNs, TimestampNs)> {
        let mut min_ts = u64::MAX;
        let mut max_ts = 0u64;
        let mut found_any = false;

        for slot in &self.storage {
            let guard = slot.read();
            if let Some(sample) = guard.as_ref() {
                min_ts = min_ts.min(sample.timestamp_ns);
                max_ts = max_ts.max(sample.timestamp_ns);
                found_any = true;
            }
        }

        if found_any {
            Some((min_ts, max_ts))
        } else {
            None
        }
    }

    /// Get buffer statistics
    pub fn stats(&self) -> BufferStatsSnapshot {
        self.stats.snapshot()
    }

    /// Evict samples older than max_age relative to current_time_ns
    pub fn evict_old(&self, current_time_ns: TimestampNs) -> usize {
        let cutoff = current_time_ns.saturating_sub(self.max_age_ns);
        let mut evicted = 0;

        for slot in &self.storage {
            let mut guard = slot.write();
            if let Some(sample) = guard.as_ref() {
                if sample.timestamp_ns < cutoff {
                    *guard = None;
                    evicted += 1;
                }
            }
        }

        self.stats
            .samples_evicted
            .fetch_add(evicted as u64, Ordering::Relaxed);
        evicted
    }

    /// Count current samples in buffer
    pub fn len(&self) -> usize {
        self.storage.iter().filter(|s| s.read().is_some()).count()
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// Safety: RingBuffer uses internal synchronization
unsafe impl Send for RingBuffer {}
unsafe impl Sync for RingBuffer {}

/// Multi-stream buffer manager
pub struct BufferManager {
    /// Buffers by stream ID
    buffers: parking_lot::RwLock<HashMap<StreamId, Arc<RingBuffer>>>,
    /// Total memory limit in bytes
    total_memory_limit: u64,
    /// Current memory used (approximate)
    current_memory_used: AtomicU64,
    /// Default buffer settings
    default_capacity: usize,
    default_max_age_seconds: f32,
}

impl BufferManager {
    /// Create a new buffer manager
    pub fn new(memory_limit_bytes: u64) -> Self {
        Self {
            buffers: parking_lot::RwLock::new(HashMap::new()),
            total_memory_limit: memory_limit_bytes,
            current_memory_used: AtomicU64::new(0),
            default_capacity: 1000,
            default_max_age_seconds: 60.0,
        }
    }

    /// Set default buffer settings
    pub fn with_defaults(mut self, capacity: usize, max_age_seconds: f32) -> Self {
        self.default_capacity = capacity;
        self.default_max_age_seconds = max_age_seconds;
        self
    }

    /// Register a new stream buffer
    pub fn register_stream(
        &self,
        stream_id: StreamId,
        capacity: Option<usize>,
        max_age_seconds: Option<f32>,
    ) -> Arc<RingBuffer> {
        let capacity = capacity.unwrap_or(self.default_capacity);
        let max_age = max_age_seconds.unwrap_or(self.default_max_age_seconds);

        let buffer = Arc::new(RingBuffer::new(stream_id.clone(), capacity, max_age));

        let mut buffers = self.buffers.write();
        buffers.insert(stream_id, buffer.clone());

        buffer
    }

    /// Get buffer for a stream
    pub fn get_buffer(&self, stream_id: &StreamId) -> Option<Arc<RingBuffer>> {
        let buffers = self.buffers.read();
        buffers.get(stream_id).cloned()
    }

    /// Get or create buffer for a stream
    pub fn get_or_create(&self, stream_id: StreamId) -> Arc<RingBuffer> {
        // First try read-only
        {
            let buffers = self.buffers.read();
            if let Some(buffer) = buffers.get(&stream_id) {
                return buffer.clone();
            }
        }

        // Need to create
        self.register_stream(stream_id, None, None)
    }

    /// Push a sample to its stream buffer
    pub fn push(&self, sample: DataSample) {
        let buffer = self.get_or_create(sample.stream_id.clone());
        buffer.push(sample);
    }

    /// Extract episode data from all buffers
    pub fn extract_episode(
        &self,
        start_ns: TimestampNs,
        end_ns: TimestampNs,
    ) -> HashMap<StreamId, Vec<DataSample>> {
        let buffers = self.buffers.read();
        let mut result = HashMap::new();

        for (stream_id, buffer) in buffers.iter() {
            let samples = buffer.get_range(start_ns, end_ns);
            if !samples.is_empty() {
                result.insert(stream_id.clone(), samples);
            }
        }

        result
    }

    /// Get all stream IDs
    pub fn stream_ids(&self) -> Vec<StreamId> {
        let buffers = self.buffers.read();
        buffers.keys().cloned().collect()
    }

    /// Get total number of streams
    pub fn stream_count(&self) -> usize {
        self.buffers.read().len()
    }

    /// Clear all buffers
    pub fn clear_all(&self) {
        let buffers = self.buffers.read();
        for buffer in buffers.values() {
            buffer.clear();
        }
    }

    /// Evict old data from all buffers
    pub fn evict_old_all(&self, current_time_ns: TimestampNs) -> usize {
        let buffers = self.buffers.read();
        let mut total_evicted = 0;

        for buffer in buffers.values() {
            total_evicted += buffer.evict_old(current_time_ns);
        }

        total_evicted
    }

    /// Get combined statistics
    pub fn stats(&self) -> BufferManagerStats {
        let buffers = self.buffers.read();
        let mut stats = BufferManagerStats::default();

        for (stream_id, buffer) in buffers.iter() {
            let buffer_stats = buffer.stats();
            stats.total_samples_pushed += buffer_stats.samples_pushed;
            stats.total_bytes_written += buffer_stats.bytes_written;
            stats.stream_stats.insert(stream_id.clone(), buffer_stats);
        }

        stats.stream_count = buffers.len();
        stats
    }
}

/// Combined buffer manager statistics
#[derive(Debug, Clone, Default)]
pub struct BufferManagerStats {
    /// Number of streams
    pub stream_count: usize,
    /// Total samples pushed across all streams
    pub total_samples_pushed: u64,
    /// Total bytes written across all streams
    pub total_bytes_written: u64,
    /// Per-stream statistics
    pub stream_stats: HashMap<StreamId, BufferStatsSnapshot>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::robotics::data_collection::types::SampleData;

    fn make_sample(stream_id: &str, timestamp_ns: u64, data: Vec<u8>) -> DataSample {
        DataSample::new(
            StreamId::new(stream_id),
            timestamp_ns,
            SampleData::Binary(data),
        )
    }

    #[test]
    fn test_ring_buffer_creation() {
        let buffer = RingBuffer::new(StreamId::new("test"), 100, 60.0);
        assert_eq!(buffer.capacity(), 100);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_push_and_get() {
        let buffer = RingBuffer::new(StreamId::new("test"), 100, 60.0);

        buffer.push(make_sample("test", 1_000_000_000, vec![1, 2, 3]));
        buffer.push(make_sample("test", 2_000_000_000, vec![4, 5, 6]));
        buffer.push(make_sample("test", 3_000_000_000, vec![7, 8, 9]));

        assert_eq!(buffer.len(), 3);

        let samples = buffer.get_range(1_000_000_000, 3_000_000_000);
        assert_eq!(samples.len(), 3);
        assert_eq!(samples[0].timestamp_ns, 1_000_000_000);
        assert_eq!(samples[2].timestamp_ns, 3_000_000_000);
    }

    #[test]
    fn test_get_latest() {
        let buffer = RingBuffer::new(StreamId::new("test"), 100, 60.0);

        for i in 0..10 {
            buffer.push(make_sample("test", i * 1_000_000_000, vec![i as u8]));
        }

        let latest = buffer.get_latest(3);
        assert_eq!(latest.len(), 3);
        assert_eq!(latest[0].timestamp_ns, 7_000_000_000);
        assert_eq!(latest[2].timestamp_ns, 9_000_000_000);
    }

    #[test]
    fn test_wrap_around() {
        let buffer = RingBuffer::new(StreamId::new("test"), 5, 60.0);

        // Push more than capacity
        for i in 0..10 {
            buffer.push(make_sample("test", i * 1_000_000_000, vec![i as u8]));
        }

        // Should only have last 5
        assert_eq!(buffer.len(), 5);

        let samples = buffer.get_latest(10);
        assert_eq!(samples.len(), 5);
    }

    #[test]
    fn test_time_range() {
        let buffer = RingBuffer::new(StreamId::new("test"), 100, 60.0);

        assert!(buffer.time_range().is_none());

        buffer.push(make_sample("test", 1_000_000_000, vec![]));
        buffer.push(make_sample("test", 5_000_000_000, vec![]));
        buffer.push(make_sample("test", 3_000_000_000, vec![]));

        let (min, max) = buffer.time_range().unwrap();
        assert_eq!(min, 1_000_000_000);
        assert_eq!(max, 5_000_000_000);
    }

    #[test]
    fn test_evict_old() {
        let buffer = RingBuffer::new(StreamId::new("test"), 100, 1.0); // 1 second max age

        buffer.push(make_sample("test", 1_000_000_000, vec![]));
        buffer.push(make_sample("test", 2_000_000_000, vec![]));
        buffer.push(make_sample("test", 3_000_000_000, vec![]));

        // Evict samples older than 2.5 seconds relative to time 3.5s
        let evicted = buffer.evict_old(3_500_000_000);
        assert_eq!(evicted, 2); // Should evict first two

        assert_eq!(buffer.len(), 1);
    }

    #[test]
    fn test_buffer_manager() {
        let manager = BufferManager::new(1_000_000_000); // 1GB limit

        let buffer1 = manager.register_stream(StreamId::new("stream1"), Some(100), None);
        let buffer2 = manager.register_stream(StreamId::new("stream2"), Some(100), None);

        assert_eq!(manager.stream_count(), 2);

        buffer1.push(make_sample("stream1", 1_000_000_000, vec![1, 2, 3]));
        buffer2.push(make_sample("stream2", 1_000_000_000, vec![4, 5, 6]));

        let episode = manager.extract_episode(0, 2_000_000_000);
        assert_eq!(episode.len(), 2);
    }

    #[test]
    fn test_buffer_manager_push() {
        let manager = BufferManager::new(1_000_000_000);

        manager.push(make_sample("auto_stream", 1_000_000_000, vec![1, 2, 3]));
        manager.push(make_sample("auto_stream", 2_000_000_000, vec![4, 5, 6]));

        assert_eq!(manager.stream_count(), 1);

        let buffer = manager.get_buffer(&StreamId::new("auto_stream")).unwrap();
        assert_eq!(buffer.len(), 2);
    }

    #[test]
    fn test_sequence_numbers() {
        let buffer = RingBuffer::new(StreamId::new("test"), 100, 60.0);

        buffer.push(make_sample("test", 1_000_000_000, vec![]));
        buffer.push(make_sample("test", 2_000_000_000, vec![]));
        buffer.push(make_sample("test", 3_000_000_000, vec![]));

        let samples = buffer.get_latest(3);
        assert_eq!(samples[0].sequence, 0);
        assert_eq!(samples[1].sequence, 1);
        assert_eq!(samples[2].sequence, 2);
    }
}
