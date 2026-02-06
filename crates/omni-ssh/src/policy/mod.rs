//! Policy management with caching and offline support
//!
//! This module provides policy management for SSH access control,
//! including caching, background refresh, and offline resilience.

use crate::server::SshBackend;
use crate::types::SshPolicy;
use chrono::{DateTime, Utc};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Policy validity status
#[derive(Debug, Clone, PartialEq)]
pub enum PolicyValidity {
    /// Policy is fresh (within max_age)
    Fresh,
    /// Policy is stale but usable (within grace_period)
    Stale,
    /// Policy is expired (deny new connections, allow existing)
    Expired,
    /// Policy is invalid (deny all)
    Invalid,
}

/// Cached SSH policy with expiry handling
#[derive(Debug, Clone)]
pub struct PolicyCache {
    /// Cached policy
    pub policy: SshPolicy,
    /// When policy was fetched
    pub fetched_at: DateTime<Utc>,
    /// ETag for conditional requests
    pub etag: Option<String>,
    /// Maximum age before requiring refresh
    pub max_age: Duration,
    /// Grace period after max_age (allow with warning)
    pub grace_period: Duration,
    /// Hard expiry (deny all after this)
    pub hard_expiry: Duration,
}

impl PolicyCache {
    /// Create a new policy cache entry
    pub fn new(policy: SshPolicy, etag: Option<String>) -> Self {
        Self {
            policy,
            fetched_at: Utc::now(),
            etag,
            max_age: Duration::from_secs(60),       // 1 minute
            grace_period: Duration::from_secs(300), // 5 minutes grace
            hard_expiry: Duration::from_secs(3600), // 1 hour hard limit
        }
    }

    /// Create cache with custom timeouts
    pub fn with_timeouts(
        policy: SshPolicy,
        etag: Option<String>,
        max_age: Duration,
        grace_period: Duration,
        hard_expiry: Duration,
    ) -> Self {
        Self {
            policy,
            fetched_at: Utc::now(),
            etag,
            max_age,
            grace_period,
            hard_expiry,
        }
    }

    /// Check policy validity
    pub fn validity(&self) -> PolicyValidity {
        let age = (Utc::now() - self.fetched_at)
            .to_std()
            .unwrap_or(Duration::MAX);

        if age < self.max_age {
            PolicyValidity::Fresh
        } else if age < self.max_age + self.grace_period {
            PolicyValidity::Stale
        } else if age < self.hard_expiry {
            PolicyValidity::Expired
        } else {
            PolicyValidity::Invalid
        }
    }

    /// Check if cache should be refreshed
    pub fn should_refresh(&self) -> bool {
        matches!(
            self.validity(),
            PolicyValidity::Stale | PolicyValidity::Expired | PolicyValidity::Invalid
        )
    }

    /// Get age of cached policy
    pub fn age(&self) -> Duration {
        (Utc::now() - self.fetched_at)
            .to_std()
            .unwrap_or(Duration::ZERO)
    }
}

/// Policy manager with caching and background refresh
pub struct PolicyManager {
    cache: Arc<RwLock<Option<PolicyCache>>>,
    backend: Arc<dyn SshBackend>,
    cache_file: Option<PathBuf>,
    refresh_interval: Duration,
}

impl PolicyManager {
    /// Create a new policy manager
    pub fn new(backend: Arc<dyn SshBackend>, cache_file: Option<PathBuf>) -> Self {
        Self {
            cache: Arc::new(RwLock::new(None)),
            backend,
            cache_file,
            refresh_interval: Duration::from_secs(60),
        }
    }

    /// Create with custom refresh interval
    pub fn with_refresh_interval(
        backend: Arc<dyn SshBackend>,
        cache_file: Option<PathBuf>,
        refresh_interval: Duration,
    ) -> Self {
        Self {
            cache: Arc::new(RwLock::new(None)),
            backend,
            cache_file,
            refresh_interval,
        }
    }

    /// Get policy - from cache if valid, else fetch
    pub async fn get_policy(&self) -> anyhow::Result<(SshPolicy, PolicyValidity)> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(ref cached) = *cache {
                let validity = cached.validity();
                match validity {
                    PolicyValidity::Fresh => {
                        return Ok((cached.policy.clone(), validity));
                    }
                    PolicyValidity::Stale => {
                        tracing::warn!(age_secs = cached.age().as_secs(), "Using stale SSH policy");
                        // Try to refresh in background
                        self.trigger_background_refresh();
                        return Ok((cached.policy.clone(), validity));
                    }
                    PolicyValidity::Expired => {
                        tracing::error!("SSH policy expired, attempting refresh");
                        // Must refresh, but return cached if refresh fails
                    }
                    PolicyValidity::Invalid => {
                        tracing::error!("SSH policy invalid, must refresh");
                    }
                }
            }
        }

        // Need to refresh
        match self.refresh().await {
            Ok(policy) => Ok((policy, PolicyValidity::Fresh)),
            Err(e) => {
                // Try to use expired cache as fallback
                let cache = self.cache.read().await;
                if let Some(ref cached) = *cache {
                    if cached.validity() != PolicyValidity::Invalid {
                        tracing::warn!(
                            error = %e,
                            "Using expired policy due to refresh failure"
                        );
                        return Ok((cached.policy.clone(), cached.validity()));
                    }
                }

                // Try to load from disk cache
                if let Some(policy) = self.load_from_disk().await {
                    tracing::warn!("Using disk-cached policy due to refresh failure");
                    // Store in memory cache
                    let cache_entry = PolicyCache::new(policy.clone(), None);
                    {
                        let mut cache = self.cache.write().await;
                        *cache = Some(cache_entry);
                    }
                    return Ok((policy, PolicyValidity::Stale));
                }

                Err(e)
            }
        }
    }

    /// Force refresh policy from backend
    pub async fn refresh(&self) -> anyhow::Result<SshPolicy> {
        tracing::debug!("Refreshing SSH policy from backend");

        // Fetch from backend
        let policy = self.backend.get_ssh_policy().await?;

        // Update cache
        let cache_entry = PolicyCache::new(policy.clone(), None);
        {
            let mut cache = self.cache.write().await;
            *cache = Some(cache_entry);
        }

        // Persist to disk for offline resilience
        self.save_to_disk(&policy).await;

        tracing::info!(
            version = policy.version,
            rules = policy.rules.len(),
            "SSH policy refreshed"
        );

        Ok(policy)
    }

    /// Trigger background refresh without blocking
    fn trigger_background_refresh(&self) {
        let manager = self.clone();
        tokio::spawn(async move {
            if let Err(e) = manager.refresh().await {
                tracing::warn!(error = %e, "Background policy refresh failed");
            }
        });
    }

    /// Start background refresh task
    pub fn start_background_refresh(self: Arc<Self>) {
        let manager = self.clone();
        let interval = self.refresh_interval;

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                if let Err(e) = manager.refresh().await {
                    tracing::warn!(error = %e, "Scheduled policy refresh failed");
                }
            }
        });
    }

    /// Save policy to disk cache
    async fn save_to_disk(&self, policy: &SshPolicy) {
        if let Some(ref path) = self.cache_file {
            match serde_json::to_string_pretty(policy) {
                Ok(json) => {
                    if let Err(e) = tokio::fs::write(path, json).await {
                        tracing::warn!(
                            path = %path.display(),
                            error = %e,
                            "Failed to save policy to disk"
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to serialize policy");
                }
            }
        }
    }

    /// Load policy from disk cache
    async fn load_from_disk(&self) -> Option<SshPolicy> {
        if let Some(ref path) = self.cache_file {
            match tokio::fs::read_to_string(path).await {
                Ok(json) => match serde_json::from_str(&json) {
                    Ok(policy) => {
                        tracing::debug!(
                            path = %path.display(),
                            "Loaded policy from disk cache"
                        );
                        return Some(policy);
                    }
                    Err(e) => {
                        tracing::warn!(
                            path = %path.display(),
                            error = %e,
                            "Failed to parse disk-cached policy"
                        );
                    }
                },
                Err(e) => {
                    tracing::debug!(
                        path = %path.display(),
                        error = %e,
                        "No disk-cached policy found"
                    );
                }
            }
        }
        None
    }

    /// Get current cache status
    pub async fn cache_status(&self) -> Option<(PolicyValidity, Duration)> {
        let cache = self.cache.read().await;
        cache.as_ref().map(|c| (c.validity(), c.age()))
    }

    /// Clear the cache (for testing or forced refresh)
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        *cache = None;
    }

    /// Check if we have a valid policy
    pub async fn has_valid_policy(&self) -> bool {
        let cache = self.cache.read().await;
        cache
            .as_ref()
            .map(|c| c.validity() != PolicyValidity::Invalid)
            .unwrap_or(false)
    }
}

impl Clone for PolicyManager {
    fn clone(&self) -> Self {
        Self {
            cache: self.cache.clone(),
            backend: self.backend.clone(),
            cache_file: self.cache_file.clone(),
            refresh_interval: self.refresh_interval,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_validity_fresh() {
        let policy = SshPolicy::default();
        let cache = PolicyCache::new(policy, None);

        assert_eq!(cache.validity(), PolicyValidity::Fresh);
        assert!(!cache.should_refresh());
    }

    #[test]
    fn test_policy_cache_with_custom_timeouts() {
        let policy = SshPolicy::default();
        let cache = PolicyCache::with_timeouts(
            policy,
            None,
            Duration::from_secs(10),
            Duration::from_secs(20),
            Duration::from_secs(60),
        );

        assert_eq!(cache.max_age, Duration::from_secs(10));
        assert_eq!(cache.grace_period, Duration::from_secs(20));
        assert_eq!(cache.hard_expiry, Duration::from_secs(60));
    }
}
