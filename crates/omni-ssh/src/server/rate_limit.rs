//! Connection rate limiting for SSH server
//!
//! Provides protection against brute-force attacks and connection floods
//! by limiting connection rates per IP and globally.

use dashmap::DashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;

/// Connection rate limiter for SSH server
#[derive(Debug, Clone)]
pub struct ConnectionRateLimiter {
    /// Max connections per source IP per minute
    pub per_ip_per_minute: u32,
    /// Max failed auth attempts before temporary ban
    pub max_failed_auth: u32,
    /// Ban duration after max_failed_auth
    pub ban_duration: Duration,
    /// Global max concurrent connections
    pub max_concurrent: u32,

    // Internal state (wrapped in Arc for cloning)
    connection_counts: Arc<DashMap<IpAddr, ConnectionCount>>,
    failed_auth_counts: Arc<DashMap<IpAddr, FailedAuthCount>>,
    banned_ips: Arc<DashMap<IpAddr, Instant>>,
    concurrent_semaphore: Arc<Semaphore>,
}

#[derive(Debug)]
struct ConnectionCount {
    count: u32,
    window_start: Instant,
}

#[derive(Debug)]
struct FailedAuthCount {
    count: u32,
    first_failure: Instant,
}

/// Rate limit check result
#[derive(Debug, Clone, PartialEq)]
pub enum RateLimitResult {
    /// Connection is allowed
    Allowed,
    /// Connection is rate limited, try again later
    RateLimited {
        /// How long to wait before retrying
        retry_after: Duration,
    },
    /// IP is temporarily banned
    Banned {
        /// Time remaining on ban
        remaining: Duration,
    },
    /// Too many concurrent connections globally
    TooManyConnections,
}

impl ConnectionRateLimiter {
    /// Create a new rate limiter with specified limits
    pub fn new(
        per_ip_per_minute: u32,
        max_failed_auth: u32,
        ban_duration: Duration,
        max_concurrent: u32,
    ) -> Self {
        Self {
            per_ip_per_minute,
            max_failed_auth,
            ban_duration,
            max_concurrent,
            connection_counts: Arc::new(DashMap::new()),
            failed_auth_counts: Arc::new(DashMap::new()),
            banned_ips: Arc::new(DashMap::new()),
            concurrent_semaphore: Arc::new(Semaphore::new(max_concurrent as usize)),
        }
    }

    /// Check if connection from IP is allowed (doesn't consume permit)
    pub fn check_allowed(&self, ip: IpAddr) -> RateLimitResult {
        // 1. Check if IP is banned
        if let Some(ban_time) = self.banned_ips.get(&ip) {
            let elapsed = ban_time.elapsed();
            if elapsed < self.ban_duration {
                return RateLimitResult::Banned {
                    remaining: self.ban_duration - elapsed,
                };
            } else {
                // Ban expired, clean up
                drop(ban_time); // Release lock before removing
                self.banned_ips.remove(&ip);
                self.failed_auth_counts.remove(&ip);
            }
        }

        // 2. Check per-IP rate limit
        let now = Instant::now();
        let mut entry = self.connection_counts.entry(ip).or_insert(ConnectionCount {
            count: 0,
            window_start: now,
        });

        // Reset window if minute has passed
        if entry.window_start.elapsed() > Duration::from_secs(60) {
            entry.count = 0;
            entry.window_start = now;
        }

        if entry.count >= self.per_ip_per_minute {
            let retry_after = Duration::from_secs(60)
                .checked_sub(entry.window_start.elapsed())
                .unwrap_or(Duration::from_secs(1));
            return RateLimitResult::RateLimited { retry_after };
        }

        // 3. Check global concurrent limit
        if self.concurrent_semaphore.available_permits() == 0 {
            return RateLimitResult::TooManyConnections;
        }

        // Increment counter (connection is allowed)
        entry.count += 1;
        RateLimitResult::Allowed
    }

    /// Acquire concurrent connection permit
    /// Returns None if no permits available
    pub fn try_acquire_permit(&self) -> Option<tokio::sync::OwnedSemaphorePermit> {
        self.concurrent_semaphore.clone().try_acquire_owned().ok()
    }

    /// Acquire concurrent connection permit, waiting if necessary
    /// Returns None if the semaphore is closed (which shouldn't happen in normal operation)
    pub async fn acquire_permit(&self) -> Option<tokio::sync::OwnedSemaphorePermit> {
        self.concurrent_semaphore.clone().acquire_owned().await.ok()
    }

    /// Record failed authentication attempt
    pub fn record_failed_auth(&self, ip: IpAddr) {
        let now = Instant::now();
        let mut entry = self
            .failed_auth_counts
            .entry(ip)
            .or_insert(FailedAuthCount {
                count: 0,
                first_failure: now,
            });

        // Reset if it's been more than an hour since first failure
        if entry.first_failure.elapsed() > Duration::from_secs(3600) {
            entry.count = 0;
            entry.first_failure = now;
        }

        entry.count += 1;

        if entry.count >= self.max_failed_auth {
            tracing::warn!(
                ip = %ip,
                failed_attempts = entry.count,
                ban_duration_secs = self.ban_duration.as_secs(),
                "Banning IP due to failed auth attempts"
            );
            self.banned_ips.insert(ip, now);
        }
    }

    /// Record successful authentication (resets failed count)
    pub fn record_successful_auth(&self, ip: IpAddr) {
        self.failed_auth_counts.remove(&ip);
    }

    /// Check if an IP is currently banned
    pub fn is_banned(&self, ip: IpAddr) -> bool {
        if let Some(ban_time) = self.banned_ips.get(&ip) {
            if ban_time.elapsed() < self.ban_duration {
                return true;
            }
        }
        false
    }

    /// Get remaining ban time for an IP
    pub fn ban_remaining(&self, ip: IpAddr) -> Option<Duration> {
        self.banned_ips.get(&ip).and_then(|ban_time| {
            let elapsed = ban_time.elapsed();
            if elapsed < self.ban_duration {
                Some(self.ban_duration - elapsed)
            } else {
                None
            }
        })
    }

    /// Manually ban an IP
    pub fn ban_ip(&self, ip: IpAddr) {
        tracing::info!(ip = %ip, "Manually banning IP");
        self.banned_ips.insert(ip, Instant::now());
    }

    /// Manually unban an IP
    pub fn unban_ip(&self, ip: IpAddr) {
        tracing::info!(ip = %ip, "Manually unbanning IP");
        self.banned_ips.remove(&ip);
        self.failed_auth_counts.remove(&ip);
    }

    /// Clean up expired entries (call periodically)
    pub fn cleanup(&self) {
        // Remove expired bans
        self.banned_ips
            .retain(|_, ban_time| ban_time.elapsed() < self.ban_duration);

        // Remove old connection counts (older than 2 minutes)
        self.connection_counts
            .retain(|_, count| count.window_start.elapsed() < Duration::from_secs(120));

        // Remove old failed auth counts (older than 1 hour)
        self.failed_auth_counts
            .retain(|_, count| count.first_failure.elapsed() < Duration::from_secs(3600));
    }

    /// Get current statistics
    pub fn stats(&self) -> RateLimiterStats {
        RateLimiterStats {
            active_connections: self.max_concurrent as usize
                - self.concurrent_semaphore.available_permits(),
            banned_ips: self.banned_ips.len(),
            tracked_ips: self.connection_counts.len(),
        }
    }

    /// Get number of available connection permits
    pub fn available_permits(&self) -> usize {
        self.concurrent_semaphore.available_permits()
    }

    /// Get list of currently banned IPs
    pub fn banned_ip_list(&self) -> Vec<(IpAddr, Duration)> {
        self.banned_ips
            .iter()
            .filter_map(|entry| {
                let elapsed = entry.value().elapsed();
                if elapsed < self.ban_duration {
                    Some((*entry.key(), self.ban_duration - elapsed))
                } else {
                    None
                }
            })
            .collect()
    }
}

impl Default for ConnectionRateLimiter {
    fn default() -> Self {
        Self::new(
            10,                       // 10 connections per IP per minute
            5,                        // 5 failed auth attempts
            Duration::from_secs(900), // 15 minute ban
            100,                      // 100 concurrent connections
        )
    }
}

/// Rate limiter statistics
#[derive(Debug, Clone)]
pub struct RateLimiterStats {
    /// Number of active connections
    pub active_connections: usize,
    /// Number of banned IPs
    pub banned_ips: usize,
    /// Number of tracked IPs
    pub tracked_ips: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn test_rate_limit_basic() {
        let limiter = ConnectionRateLimiter::new(5, 3, Duration::from_secs(60), 100);
        let ip: IpAddr = Ipv4Addr::new(192, 168, 1, 1).into();

        // First 5 should be allowed
        for _ in 0..5 {
            assert!(matches!(
                limiter.check_allowed(ip),
                RateLimitResult::Allowed
            ));
        }

        // 6th should be rate limited
        assert!(matches!(
            limiter.check_allowed(ip),
            RateLimitResult::RateLimited { .. }
        ));
    }

    #[test]
    fn test_failed_auth_ban() {
        let limiter = ConnectionRateLimiter::new(10, 3, Duration::from_secs(60), 100);
        let ip: IpAddr = Ipv4Addr::new(192, 168, 1, 1).into();

        // Record 3 failed auth attempts
        for _ in 0..3 {
            limiter.record_failed_auth(ip);
        }

        // Should now be banned
        assert!(limiter.is_banned(ip));
        assert!(matches!(
            limiter.check_allowed(ip),
            RateLimitResult::Banned { .. }
        ));
    }

    #[test]
    fn test_successful_auth_clears_failed() {
        let limiter = ConnectionRateLimiter::new(10, 3, Duration::from_secs(60), 100);
        let ip: IpAddr = Ipv4Addr::new(192, 168, 1, 1).into();

        // Record 2 failed auth attempts
        limiter.record_failed_auth(ip);
        limiter.record_failed_auth(ip);

        // Successful auth should clear the count
        limiter.record_successful_auth(ip);

        // 2 more failures shouldn't trigger ban
        limiter.record_failed_auth(ip);
        limiter.record_failed_auth(ip);

        assert!(!limiter.is_banned(ip));
    }

    #[test]
    fn test_manual_ban_unban() {
        let limiter = ConnectionRateLimiter::new(10, 3, Duration::from_secs(60), 100);
        let ip: IpAddr = Ipv4Addr::new(192, 168, 1, 1).into();

        assert!(!limiter.is_banned(ip));

        limiter.ban_ip(ip);
        assert!(limiter.is_banned(ip));

        limiter.unban_ip(ip);
        assert!(!limiter.is_banned(ip));
    }

    #[test]
    fn test_different_ips_independent() {
        let limiter = ConnectionRateLimiter::new(5, 3, Duration::from_secs(60), 100);
        let ip1: IpAddr = Ipv4Addr::new(192, 168, 1, 1).into();
        let ip2: IpAddr = Ipv4Addr::new(192, 168, 1, 2).into();

        // Exhaust rate limit for ip1
        for _ in 0..5 {
            assert!(matches!(
                limiter.check_allowed(ip1),
                RateLimitResult::Allowed
            ));
        }
        assert!(matches!(
            limiter.check_allowed(ip1),
            RateLimitResult::RateLimited { .. }
        ));

        // ip2 should still be allowed
        assert!(matches!(
            limiter.check_allowed(ip2),
            RateLimitResult::Allowed
        ));
    }

    #[test]
    fn test_stats() {
        let limiter = ConnectionRateLimiter::new(10, 3, Duration::from_secs(60), 100);
        let ip: IpAddr = Ipv4Addr::new(192, 168, 1, 1).into();

        // Check initial stats
        let stats = limiter.stats();
        assert_eq!(stats.active_connections, 0);
        assert_eq!(stats.banned_ips, 0);

        // Ban an IP
        limiter.ban_ip(ip);
        let stats = limiter.stats();
        assert_eq!(stats.banned_ips, 1);
    }

    #[tokio::test]
    async fn test_concurrent_limit() {
        let limiter = ConnectionRateLimiter::new(100, 3, Duration::from_secs(60), 2);

        // Acquire 2 permits
        let permit1 = limiter.try_acquire_permit();
        let permit2 = limiter.try_acquire_permit();

        assert!(permit1.is_some());
        assert!(permit2.is_some());

        // Third should fail
        let permit3 = limiter.try_acquire_permit();
        assert!(permit3.is_none());

        // Check that check_allowed also reports too many connections
        let ip: IpAddr = Ipv4Addr::new(192, 168, 1, 1).into();
        assert!(matches!(
            limiter.check_allowed(ip),
            RateLimitResult::TooManyConnections
        ));

        // Release a permit
        drop(permit1);

        // Now should work
        let permit4 = limiter.try_acquire_permit();
        assert!(permit4.is_some());
    }
}
