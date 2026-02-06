//! Connection health monitoring
//!
//! Provides real-time health monitoring for SSH connections with
//! automatic disconnect on health degradation.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::watch;

/// Health status of a connection
#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    /// Connection is healthy
    Healthy,
    /// Connection is degraded but functional
    Degraded {
        /// Reason for degradation
        reason: String,
    },
    /// Connection is unhealthy
    Unhealthy {
        /// Reason for unhealthy status
        reason: String,
    },
    /// Connection is dead
    Dead {
        /// Reason for death
        reason: String,
    },
}

impl HealthStatus {
    /// Check if the connection should be terminated
    pub fn should_terminate(&self) -> bool {
        matches!(self, HealthStatus::Dead { .. })
    }

    /// Check if the connection is usable
    pub fn is_usable(&self) -> bool {
        matches!(self, HealthStatus::Healthy | HealthStatus::Degraded { .. })
    }
}

/// Health thresholds for connection monitoring
#[derive(Debug, Clone)]
pub struct HealthThreshold {
    /// Maximum RTT before degraded (milliseconds)
    pub max_rtt_degraded_ms: u64,
    /// Maximum RTT before unhealthy (milliseconds)
    pub max_rtt_unhealthy_ms: u64,
    /// Maximum packet loss percentage before degraded
    pub max_loss_degraded_pct: f32,
    /// Maximum packet loss percentage before unhealthy
    pub max_loss_unhealthy_pct: f32,
    /// Maximum idle time before keepalive (seconds)
    pub keepalive_interval_secs: u64,
    /// Maximum missed keepalives before dead
    pub max_missed_keepalives: u32,
    /// Timeout for SSH operations (seconds)
    pub operation_timeout_secs: u64,
}

impl Default for HealthThreshold {
    fn default() -> Self {
        Self {
            max_rtt_degraded_ms: 500,     // 500ms
            max_rtt_unhealthy_ms: 2000,   // 2 seconds
            max_loss_degraded_pct: 5.0,   // 5%
            max_loss_unhealthy_pct: 20.0, // 20%
            keepalive_interval_secs: 30,  // 30 seconds
            max_missed_keepalives: 3,     // 3 missed
            operation_timeout_secs: 60,   // 1 minute
        }
    }
}

/// Connection health tracking
#[derive(Debug)]
pub struct ConnectionHealth {
    /// Connection ID
    pub conn_id: String,
    /// Current health status
    status: Arc<watch::Sender<HealthStatus>>,
    /// Last activity timestamp
    last_activity: Arc<AtomicU64>,
    /// Last keepalive sent timestamp  
    last_keepalive_sent: Arc<AtomicU64>,
    /// Last keepalive received timestamp
    last_keepalive_received: Arc<AtomicU64>,
    /// Missed keepalive count
    missed_keepalives: Arc<AtomicU64>,
    /// Total bytes received
    bytes_received: Arc<AtomicU64>,
    /// Total bytes sent
    bytes_sent: Arc<AtomicU64>,
    /// Whether monitoring is active
    active: Arc<AtomicBool>,
    /// Health thresholds
    thresholds: HealthThreshold,
    /// Start time
    start_time: Instant,
}

impl ConnectionHealth {
    /// Create new connection health tracker
    pub fn new(conn_id: String, thresholds: HealthThreshold) -> Self {
        let (tx, _rx) = watch::channel(HealthStatus::Healthy);
        let now = Instant::now().elapsed().as_secs();

        Self {
            conn_id,
            status: Arc::new(tx),
            last_activity: Arc::new(AtomicU64::new(now)),
            last_keepalive_sent: Arc::new(AtomicU64::new(0)),
            last_keepalive_received: Arc::new(AtomicU64::new(0)),
            missed_keepalives: Arc::new(AtomicU64::new(0)),
            bytes_received: Arc::new(AtomicU64::new(0)),
            bytes_sent: Arc::new(AtomicU64::new(0)),
            active: Arc::new(AtomicBool::new(true)),
            thresholds,
            start_time: Instant::now(),
        }
    }

    /// Subscribe to health status changes
    pub fn subscribe(&self) -> watch::Receiver<HealthStatus> {
        self.status.subscribe()
    }

    /// Get current health status
    pub fn status(&self) -> HealthStatus {
        self.status.borrow().clone()
    }

    /// Record activity (data received/sent)
    pub fn record_activity(&self) {
        self.last_activity
            .store(self.start_time.elapsed().as_secs(), Ordering::Relaxed);
    }

    /// Record bytes received
    pub fn record_received(&self, bytes: u64) {
        self.bytes_received.fetch_add(bytes, Ordering::Relaxed);
        self.record_activity();
    }

    /// Record bytes sent
    pub fn record_sent(&self, bytes: u64) {
        self.bytes_sent.fetch_add(bytes, Ordering::Relaxed);
        self.record_activity();
    }

    /// Record keepalive sent
    pub fn record_keepalive_sent(&self) {
        self.last_keepalive_sent
            .store(self.start_time.elapsed().as_secs(), Ordering::Relaxed);
    }

    /// Record keepalive response received
    pub fn record_keepalive_received(&self) {
        self.last_keepalive_received
            .store(self.start_time.elapsed().as_secs(), Ordering::Relaxed);
        self.missed_keepalives.store(0, Ordering::Relaxed);
        self.record_activity();
    }

    /// Record missed keepalive
    pub fn record_missed_keepalive(&self) {
        let missed = self.missed_keepalives.fetch_add(1, Ordering::Relaxed) + 1;

        if missed >= self.thresholds.max_missed_keepalives as u64 {
            self.update_status(HealthStatus::Dead {
                reason: format!("Missed {} keepalives", missed),
            });
        } else {
            self.update_status(HealthStatus::Unhealthy {
                reason: format!(
                    "Missed {} of {} keepalives",
                    missed, self.thresholds.max_missed_keepalives
                ),
            });
        }
    }

    /// Update RTT measurement
    pub fn update_rtt(&self, rtt_ms: u64) {
        let status = if rtt_ms > self.thresholds.max_rtt_unhealthy_ms {
            HealthStatus::Unhealthy {
                reason: format!("High RTT: {}ms", rtt_ms),
            }
        } else if rtt_ms > self.thresholds.max_rtt_degraded_ms {
            HealthStatus::Degraded {
                reason: format!("Elevated RTT: {}ms", rtt_ms),
            }
        } else {
            HealthStatus::Healthy
        };

        self.update_status(status);
    }

    /// Update packet loss measurement
    pub fn update_packet_loss(&self, loss_pct: f32) {
        let status = if loss_pct > self.thresholds.max_loss_unhealthy_pct {
            HealthStatus::Unhealthy {
                reason: format!("High packet loss: {:.1}%", loss_pct),
            }
        } else if loss_pct > self.thresholds.max_loss_degraded_pct {
            HealthStatus::Degraded {
                reason: format!("Elevated packet loss: {:.1}%", loss_pct),
            }
        } else {
            HealthStatus::Healthy
        };

        self.update_status(status);
    }

    /// Mark connection as dead
    pub fn mark_dead(&self, reason: &str) {
        self.update_status(HealthStatus::Dead {
            reason: reason.to_string(),
        });
    }

    /// Update health status
    fn update_status(&self, status: HealthStatus) {
        if self.active.load(Ordering::Relaxed) {
            // Use send_replace to ensure the value is always updated
            self.status.send_replace(status);
        }
    }

    /// Check if needs keepalive
    pub fn needs_keepalive(&self) -> bool {
        let last_activity = self.last_activity.load(Ordering::Relaxed);
        let elapsed = self.start_time.elapsed().as_secs() - last_activity;
        elapsed >= self.thresholds.keepalive_interval_secs
    }

    /// Get connection statistics
    pub fn stats(&self) -> ConnectionStats {
        ConnectionStats {
            conn_id: self.conn_id.clone(),
            uptime: self.start_time.elapsed(),
            bytes_received: self.bytes_received.load(Ordering::Relaxed),
            bytes_sent: self.bytes_sent.load(Ordering::Relaxed),
            missed_keepalives: self.missed_keepalives.load(Ordering::Relaxed) as u32,
            status: self.status(),
        }
    }

    /// Stop monitoring (connection closing)
    pub fn stop(&self) {
        self.active.store(false, Ordering::Relaxed);
    }
}

/// Connection statistics
#[derive(Debug, Clone)]
pub struct ConnectionStats {
    /// Connection ID
    pub conn_id: String,
    /// Connection uptime
    pub uptime: Duration,
    /// Total bytes received
    pub bytes_received: u64,
    /// Total bytes sent
    pub bytes_sent: u64,
    /// Number of missed keepalives
    pub missed_keepalives: u32,
    /// Current status
    pub status: HealthStatus,
}

/// Health monitor for multiple connections
pub struct HealthMonitor {
    /// Connections being monitored
    connections: dashmap::DashMap<String, Arc<ConnectionHealth>>,
    /// Default thresholds
    default_thresholds: HealthThreshold,
}

impl HealthMonitor {
    /// Create a new health monitor
    pub fn new(thresholds: HealthThreshold) -> Self {
        Self {
            connections: dashmap::DashMap::new(),
            default_thresholds: thresholds,
        }
    }

    /// Start monitoring a connection
    pub fn monitor(&self, conn_id: String) -> Arc<ConnectionHealth> {
        let health = Arc::new(ConnectionHealth::new(
            conn_id.clone(),
            self.default_thresholds.clone(),
        ));
        self.connections.insert(conn_id, health.clone());
        health
    }

    /// Start monitoring with custom thresholds
    pub fn monitor_with_thresholds(
        &self,
        conn_id: String,
        thresholds: HealthThreshold,
    ) -> Arc<ConnectionHealth> {
        let health = Arc::new(ConnectionHealth::new(conn_id.clone(), thresholds));
        self.connections.insert(conn_id, health.clone());
        health
    }

    /// Stop monitoring a connection
    pub fn stop_monitoring(&self, conn_id: &str) {
        if let Some((_, health)) = self.connections.remove(conn_id) {
            health.stop();
        }
    }

    /// Get health for a connection
    pub fn get(&self, conn_id: &str) -> Option<Arc<ConnectionHealth>> {
        self.connections.get(conn_id).map(|r| r.value().clone())
    }

    /// Get all unhealthy connections
    pub fn unhealthy_connections(&self) -> Vec<String> {
        self.connections
            .iter()
            .filter(|entry| {
                matches!(
                    entry.value().status(),
                    HealthStatus::Unhealthy { .. } | HealthStatus::Dead { .. }
                )
            })
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Get all connections needing keepalive
    pub fn needs_keepalive(&self) -> Vec<String> {
        self.connections
            .iter()
            .filter(|entry| entry.value().needs_keepalive())
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Get overall stats
    pub fn overall_stats(&self) -> MonitorStats {
        let mut healthy = 0;
        let mut degraded = 0;
        let mut unhealthy = 0;
        let mut dead = 0;

        for entry in self.connections.iter() {
            match entry.value().status() {
                HealthStatus::Healthy => healthy += 1,
                HealthStatus::Degraded { .. } => degraded += 1,
                HealthStatus::Unhealthy { .. } => unhealthy += 1,
                HealthStatus::Dead { .. } => dead += 1,
            }
        }

        MonitorStats {
            total: self.connections.len(),
            healthy,
            degraded,
            unhealthy,
            dead,
        }
    }
}

impl Default for HealthMonitor {
    fn default() -> Self {
        Self::new(HealthThreshold::default())
    }
}

/// Monitor-wide statistics
#[derive(Debug, Clone)]
pub struct MonitorStats {
    /// Total monitored connections
    pub total: usize,
    /// Healthy connections
    pub healthy: usize,
    /// Degraded connections
    pub degraded: usize,
    /// Unhealthy connections
    pub unhealthy: usize,
    /// Dead connections
    pub dead: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_usable() {
        assert!(HealthStatus::Healthy.is_usable());
        assert!(HealthStatus::Degraded {
            reason: "test".to_string()
        }
        .is_usable());
        assert!(!HealthStatus::Unhealthy {
            reason: "test".to_string()
        }
        .is_usable());
        assert!(!HealthStatus::Dead {
            reason: "test".to_string()
        }
        .is_usable());
    }

    #[test]
    fn test_health_status_terminate() {
        assert!(!HealthStatus::Healthy.should_terminate());
        assert!(!HealthStatus::Degraded {
            reason: "test".to_string()
        }
        .should_terminate());
        assert!(!HealthStatus::Unhealthy {
            reason: "test".to_string()
        }
        .should_terminate());
        assert!(HealthStatus::Dead {
            reason: "test".to_string()
        }
        .should_terminate());
    }

    #[test]
    fn test_connection_health_basic() {
        let health = ConnectionHealth::new("test".to_string(), HealthThreshold::default());

        assert_eq!(health.status(), HealthStatus::Healthy);
        assert_eq!(health.conn_id, "test");
    }

    #[test]
    fn test_connection_health_rtt_thresholds() {
        let thresholds = HealthThreshold {
            max_rtt_degraded_ms: 100,
            max_rtt_unhealthy_ms: 500,
            ..Default::default()
        };
        let health = ConnectionHealth::new("test".to_string(), thresholds);

        // Normal RTT
        health.update_rtt(50);
        assert_eq!(health.status(), HealthStatus::Healthy);

        // Degraded RTT
        health.update_rtt(200);
        assert!(matches!(health.status(), HealthStatus::Degraded { .. }));

        // Unhealthy RTT
        health.update_rtt(1000);
        assert!(matches!(health.status(), HealthStatus::Unhealthy { .. }));
    }

    #[test]
    fn test_connection_stats() {
        let health = ConnectionHealth::new("test".to_string(), HealthThreshold::default());

        health.record_received(1000);
        health.record_sent(500);

        let stats = health.stats();
        assert_eq!(stats.bytes_received, 1000);
        assert_eq!(stats.bytes_sent, 500);
        assert_eq!(stats.conn_id, "test");
    }

    #[test]
    fn test_health_monitor() {
        let monitor = HealthMonitor::default();

        let health1 = monitor.monitor("conn1".to_string());
        let health2 = monitor.monitor("conn2".to_string());

        assert_eq!(monitor.overall_stats().total, 2);
        assert_eq!(monitor.overall_stats().healthy, 2);

        health1.mark_dead("test");
        assert_eq!(monitor.overall_stats().dead, 1);

        let unhealthy = monitor.unhealthy_connections();
        assert_eq!(unhealthy.len(), 1);
        assert_eq!(unhealthy[0], "conn1");
    }
}
