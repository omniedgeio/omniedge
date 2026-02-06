//! OmniEdge identity-based authentication

use crate::server::{PeerIdentity, SshBackend};
use crate::types::*;
use std::net::IpAddr;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Authenticator using OmniEdge identity
pub struct OmniEdgeAuthenticator {
    backend: Arc<dyn SshBackend>,
}

impl OmniEdgeAuthenticator {
    /// Create a new authenticator
    pub fn new(backend: Arc<dyn SshBackend>) -> Self {
        Self { backend }
    }

    /// Authenticate incoming connection using OmniEdge identity
    pub async fn authenticate(&self, src_ip: IpAddr, ssh_user: &str) -> anyhow::Result<AuthResult> {
        // 1. Verify connection comes from VPN tunnel
        if !self.backend.is_omniedge_ip(src_ip) {
            warn!("Auth rejected: {} is not an OmniEdge IP", src_ip);
            return Ok(AuthResult::Reject {
                message: "Connection not from OmniEdge network".to_string(),
            });
        }

        // 2. Look up peer identity via OmniEdge backend
        let peer = match self.backend.who_is(src_ip).await? {
            Some(p) => p,
            None => {
                warn!("Auth rejected: Unknown peer at {}", src_ip);
                return Ok(AuthResult::Reject {
                    message: "Unknown peer".to_string(),
                });
            }
        };

        debug!(
            "Authenticating SSH user '{}' from {} (node: {}, user: {})",
            ssh_user, src_ip, peer.node.name, peer.user.email
        );

        // 3. Evaluate SSH policy
        let policy = self.backend.get_ssh_policy().await?;
        let (action, matched_rule) = self.evaluate_policy(&policy, &peer, ssh_user)?;

        // 4. Check for HoldAndDelegate (interactive authorization)
        if let Some(url) = &action.hold_and_delegate {
            info!(
                "SSH connection requires interactive approval: {} -> {}",
                peer.user.email, ssh_user
            );
            return Ok(AuthResult::HoldAndDelegate { url: url.clone() });
        }

        // 5. Check time restrictions
        if let Some(restrictions) = &action.time_restrictions {
            if !self.check_time_restrictions(restrictions) {
                if restrictions.override_with_approval {
                    if let Some(url) = &action.hold_and_delegate {
                        return Ok(AuthResult::HoldAndDelegate { url: url.clone() });
                    }
                }
                return Ok(AuthResult::Reject {
                    message: "Access not allowed at this time".to_string(),
                });
            }
        }

        // 6. Check accept/reject
        if action.reject {
            return Ok(AuthResult::Reject {
                message: action
                    .message
                    .unwrap_or_else(|| "Access denied".to_string()),
            });
        }

        if !action.accept {
            return Ok(AuthResult::Reject {
                message: "No matching rule".to_string(),
            });
        }

        // 7. Map SSH user to local user
        let local_user = self.resolve_local_user(&matched_rule, ssh_user)?;

        info!(
            "SSH access granted: {}@{} -> {} (local: {})",
            peer.user.email, peer.node.name, ssh_user, local_user
        );

        Ok(AuthResult::Accept { local_user, action })
    }

    /// Evaluate policy rules against connection
    fn evaluate_policy(
        &self,
        policy: &SshPolicy,
        peer: &PeerIdentity,
        ssh_user: &str,
    ) -> anyhow::Result<(SshAction, Option<SshRule>)> {
        for rule in &policy.rules {
            // Check if rule is expired
            if let Some(expires) = &rule.expires {
                if *expires < chrono::Utc::now() {
                    continue;
                }
            }

            // Check if any principal matches
            if self.principal_matches(&rule.principals, peer) {
                // Check if SSH user is allowed
                if rule.ssh_users.contains_key(ssh_user) || rule.ssh_users.contains_key("*") {
                    debug!(
                        "Rule '{}' matched for user '{}' from {}",
                        rule.id, ssh_user, peer.node.name
                    );
                    return Ok((rule.action.clone(), Some(rule.clone())));
                }
            }
        }

        // No matching rule - default deny
        debug!(
            "No matching SSH policy rule for user '{}' from {}",
            ssh_user, peer.node.name
        );
        Ok((
            SshAction {
                accept: false,
                reject: true,
                message: Some("No matching SSH policy rule".to_string()),
                ..Default::default()
            },
            None,
        ))
    }

    /// Check if any principal matches the peer
    fn principal_matches(&self, principals: &[SshPrincipal], peer: &PeerIdentity) -> bool {
        for p in principals {
            if p.any {
                return true;
            }
            if let Some(node_id) = &p.node_id {
                if *node_id == peer.node.id {
                    return true;
                }
            }
            if let Some(node_ip) = &p.node_ip {
                if *node_ip == peer.node.virtual_ip {
                    return true;
                }
            }
            if let Some(email) = &p.user_email {
                if self.email_matches(email, &peer.user.email) {
                    return true;
                }
            }
            if let Some(network_id) = &p.network_id {
                if *network_id == peer.node.network_id {
                    return true;
                }
            }
            if let Some(tag) = &p.tag {
                if peer.node.tags.contains(tag) {
                    return true;
                }
            }
        }
        false
    }

    /// Check if email pattern matches
    fn email_matches(&self, pattern: &str, email: &str) -> bool {
        if pattern == "*" {
            return true;
        }
        if pattern.starts_with('*') && pattern.len() > 1 {
            // Wildcard suffix match: *@domain.com
            let suffix = &pattern[1..];
            return email.ends_with(suffix);
        }
        pattern == email
    }

    /// Resolve SSH user to local system user
    fn resolve_local_user(&self, rule: &Option<SshRule>, ssh_user: &str) -> anyhow::Result<String> {
        if let Some(rule) = rule {
            // Check exact match first
            if let Some(local) = rule.ssh_users.get(ssh_user) {
                if local == "=" {
                    return Ok(ssh_user.to_string());
                }
                return Ok(local.clone());
            }
            // Check wildcard
            if let Some(local) = rule.ssh_users.get("*") {
                if local == "=" {
                    return Ok(ssh_user.to_string());
                }
                return Ok(local.clone());
            }
        }
        Err(anyhow::anyhow!("No user mapping found for '{}'", ssh_user))
    }

    /// Check time restrictions
    fn check_time_restrictions(&self, restrictions: &TimeRestrictions) -> bool {
        use chrono::{Datelike, Timelike, Utc};

        let now = Utc::now();

        // Check allowed days
        if let Some(allowed_days) = &restrictions.allowed_days {
            let day_name = match now.weekday() {
                chrono::Weekday::Mon => "monday",
                chrono::Weekday::Tue => "tuesday",
                chrono::Weekday::Wed => "wednesday",
                chrono::Weekday::Thu => "thursday",
                chrono::Weekday::Fri => "friday",
                chrono::Weekday::Sat => "saturday",
                chrono::Weekday::Sun => "sunday",
            };

            if !allowed_days.iter().any(|d| d.to_lowercase() == day_name) {
                return false;
            }
        }

        // Check allowed hours
        if let Some(hours) = &restrictions.allowed_hours {
            let current_hour = now.hour();
            let current_minute = now.minute();
            let current_time = current_hour * 100 + current_minute;

            // Parse start and end times (HH:MM format)
            let start_time = Self::parse_time(&hours.start).unwrap_or(0);
            let end_time = Self::parse_time(&hours.end).unwrap_or(2400);

            if start_time <= end_time {
                // Normal range (e.g., 09:00 - 17:00)
                if current_time < start_time || current_time > end_time {
                    return false;
                }
            } else {
                // Overnight range (e.g., 22:00 - 06:00)
                if current_time < start_time && current_time > end_time {
                    return false;
                }
            }
        }

        true
    }

    /// Parse time string (HH:MM) to integer (HHMM)
    fn parse_time(time: &str) -> Option<u32> {
        let parts: Vec<&str> = time.split(':').collect();
        if parts.len() != 2 {
            return None;
        }
        let hour: u32 = parts[0].parse().ok()?;
        let minute: u32 = parts[1].parse().ok()?;
        Some(hour * 100 + minute)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_matches() {
        let auth = OmniEdgeAuthenticator {
            backend: Arc::new(MockBackend),
        };

        assert!(auth.email_matches("*", "user@example.com"));
        assert!(auth.email_matches("user@example.com", "user@example.com"));
        assert!(auth.email_matches("*@example.com", "user@example.com"));
        assert!(!auth.email_matches("*@other.com", "user@example.com"));
        assert!(!auth.email_matches("other@example.com", "user@example.com"));
    }

    // Mock backend for testing
    struct MockBackend;

    #[async_trait::async_trait]
    impl SshBackend for MockBackend {
        async fn get_host_keys(&self) -> anyhow::Result<Vec<russh_keys::key::KeyPair>> {
            Ok(vec![])
        }
        fn ssh_enabled(&self) -> bool {
            true
        }
        async fn who_is(&self, _addr: IpAddr) -> anyhow::Result<Option<PeerIdentity>> {
            Ok(None)
        }
        async fn get_ssh_policy(&self) -> anyhow::Result<SshPolicy> {
            Ok(SshPolicy::default())
        }
        async fn on_ssh_event(&self, _event: super::super::SshEvent) {}
        fn is_omniedge_ip(&self, _addr: IpAddr) -> bool {
            true
        }
        fn device_id(&self) -> &str {
            "test-device"
        }
        fn network_id(&self) -> &str {
            "test-network"
        }
    }
}
