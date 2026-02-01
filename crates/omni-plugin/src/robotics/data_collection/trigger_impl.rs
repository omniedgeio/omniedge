//! Concrete trigger implementations
//!
//! This module provides ready-to-use trigger implementations for common
//! robot data collection scenarios.

use super::triggers::{
    Trigger, TriggerContext, TriggerEvent, TriggerId, TriggerPriority, TriggerType,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Trigger that fires when teleoperation starts
///
/// Captures episodes during human demonstrations for imitation learning.
pub struct TeleopTrigger {
    id: TriggerId,
    priority: TriggerPriority,
    pre_roll_ns: u64,
    post_roll_ns: u64,
    /// Track previous state to detect transitions
    was_active: AtomicBool,
    enabled: AtomicBool,
}

impl TeleopTrigger {
    /// Create a new teleop trigger
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: TriggerId::new(id),
            priority: TriggerPriority::High,
            pre_roll_ns: 2_000_000_000,   // 2 seconds pre-roll
            post_roll_ns: 10_000_000_000, // 10 seconds post-roll (full demo)
            was_active: AtomicBool::new(false),
            enabled: AtomicBool::new(true),
        }
    }

    /// Set priority
    pub fn with_priority(mut self, priority: TriggerPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Set pre-roll duration
    pub fn with_pre_roll(mut self, duration_ns: u64) -> Self {
        self.pre_roll_ns = duration_ns;
        self
    }

    /// Set post-roll duration
    pub fn with_post_roll(mut self, duration_ns: u64) -> Self {
        self.post_roll_ns = duration_ns;
        self
    }
}

impl Trigger for TeleopTrigger {
    fn id(&self) -> &TriggerId {
        &self.id
    }

    fn trigger_type(&self) -> TriggerType {
        TriggerType::Teleop
    }

    fn priority(&self) -> TriggerPriority {
        self.priority
    }

    fn evaluate(&self, context: &TriggerContext) -> Option<TriggerEvent> {
        let was = self
            .was_active
            .swap(context.is_teleop_active, Ordering::SeqCst);

        // Fire on rising edge (teleop just started)
        if context.is_teleop_active && !was {
            Some(
                TriggerEvent::new(
                    self.id.clone(),
                    TriggerType::Teleop,
                    context.timestamp_ns,
                    "Teleoperation started",
                )
                .with_priority(self.priority)
                .with_pre_roll(self.pre_roll_ns)
                .with_post_roll(self.post_roll_ns)
                .with_metadata("trigger_edge", "rising"),
            )
        } else {
            None
        }
    }

    fn reset(&self) {
        self.was_active.store(false, Ordering::SeqCst);
    }

    fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
    }

    fn config_metadata(&self) -> HashMap<String, String> {
        let mut meta = HashMap::new();
        meta.insert("pre_roll_ns".into(), self.pre_roll_ns.to_string());
        meta.insert("post_roll_ns".into(), self.post_roll_ns.to_string());
        meta
    }
}

/// Trigger that fires on detected failures
///
/// Captures episodes when the robot experiences a failure, which are
/// valuable for learning recovery behaviors.
pub struct FailureTrigger {
    id: TriggerId,
    priority: TriggerPriority,
    pre_roll_ns: u64,
    post_roll_ns: u64,
    enabled: AtomicBool,
}

impl FailureTrigger {
    /// Create a new failure trigger
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: TriggerId::new(id),
            priority: TriggerPriority::Critical, // Failures are critical
            pre_roll_ns: 10_000_000_000,         // 10 seconds before failure
            post_roll_ns: 5_000_000_000,         // 5 seconds after
            enabled: AtomicBool::new(true),
        }
    }

    /// Set priority
    pub fn with_priority(mut self, priority: TriggerPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Set pre-roll duration
    pub fn with_pre_roll(mut self, duration_ns: u64) -> Self {
        self.pre_roll_ns = duration_ns;
        self
    }

    /// Set post-roll duration
    pub fn with_post_roll(mut self, duration_ns: u64) -> Self {
        self.post_roll_ns = duration_ns;
        self
    }
}

impl Trigger for FailureTrigger {
    fn id(&self) -> &TriggerId {
        &self.id
    }

    fn trigger_type(&self) -> TriggerType {
        TriggerType::Failure
    }

    fn priority(&self) -> TriggerPriority {
        self.priority
    }

    fn evaluate(&self, context: &TriggerContext) -> Option<TriggerEvent> {
        if context.failure_detected {
            let reason = context
                .failure_reason
                .clone()
                .unwrap_or_else(|| "Unknown failure".into());

            Some(
                TriggerEvent::new(
                    self.id.clone(),
                    TriggerType::Failure,
                    context.timestamp_ns,
                    &reason,
                )
                .with_priority(self.priority)
                .with_pre_roll(self.pre_roll_ns)
                .with_post_roll(self.post_roll_ns)
                .with_metadata("failure_reason", reason),
            )
        } else {
            None
        }
    }

    fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
    }

    fn config_metadata(&self) -> HashMap<String, String> {
        let mut meta = HashMap::new();
        meta.insert("pre_roll_ns".into(), self.pre_roll_ns.to_string());
        meta.insert("post_roll_ns".into(), self.post_roll_ns.to_string());
        meta
    }
}

/// Trigger that fires when anomaly score exceeds a threshold
///
/// Captures unusual situations that may be valuable for training.
pub struct AnomalyTrigger {
    id: TriggerId,
    threshold: f32,
    priority: TriggerPriority,
    pre_roll_ns: u64,
    post_roll_ns: u64,
    enabled: AtomicBool,
}

impl AnomalyTrigger {
    /// Create a new anomaly trigger
    pub fn new(id: impl Into<String>, threshold: f32) -> Self {
        Self {
            id: TriggerId::new(id),
            threshold: threshold.clamp(0.0, 1.0),
            priority: TriggerPriority::High,
            pre_roll_ns: 5_000_000_000,  // 5 seconds pre-roll
            post_roll_ns: 5_000_000_000, // 5 seconds post-roll
            enabled: AtomicBool::new(true),
        }
    }

    /// Set the threshold
    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Set priority
    pub fn with_priority(mut self, priority: TriggerPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Set pre-roll duration
    pub fn with_pre_roll(mut self, duration_ns: u64) -> Self {
        self.pre_roll_ns = duration_ns;
        self
    }

    /// Set post-roll duration
    pub fn with_post_roll(mut self, duration_ns: u64) -> Self {
        self.post_roll_ns = duration_ns;
        self
    }
}

impl Trigger for AnomalyTrigger {
    fn id(&self) -> &TriggerId {
        &self.id
    }

    fn trigger_type(&self) -> TriggerType {
        TriggerType::Anomaly
    }

    fn priority(&self) -> TriggerPriority {
        self.priority
    }

    fn evaluate(&self, context: &TriggerContext) -> Option<TriggerEvent> {
        if context.anomaly_score >= self.threshold {
            Some(
                TriggerEvent::new(
                    self.id.clone(),
                    TriggerType::Anomaly,
                    context.timestamp_ns,
                    format!(
                        "Anomaly detected: score {:.3} >= threshold {:.3}",
                        context.anomaly_score, self.threshold
                    ),
                )
                .with_priority(self.priority)
                .with_pre_roll(self.pre_roll_ns)
                .with_post_roll(self.post_roll_ns)
                .with_metadata("anomaly_score", context.anomaly_score.to_string())
                .with_metadata("threshold", self.threshold.to_string()),
            )
        } else {
            None
        }
    }

    fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
    }

    fn config_metadata(&self) -> HashMap<String, String> {
        let mut meta = HashMap::new();
        meta.insert("threshold".into(), self.threshold.to_string());
        meta.insert("pre_roll_ns".into(), self.pre_roll_ns.to_string());
        meta.insert("post_roll_ns".into(), self.post_roll_ns.to_string());
        meta
    }
}

/// Trigger that fires at regular intervals
///
/// Useful for background data collection to build diverse datasets.
pub struct PeriodicTrigger {
    id: TriggerId,
    interval_ns: u64,
    priority: TriggerPriority,
    pre_roll_ns: u64,
    post_roll_ns: u64,
    last_fire_ns: AtomicU64,
    enabled: AtomicBool,
}

impl PeriodicTrigger {
    /// Create a new periodic trigger
    ///
    /// # Arguments
    /// * `id` - Trigger identifier
    /// * `interval_ns` - Interval between triggers in nanoseconds
    pub fn new(id: impl Into<String>, interval_ns: u64) -> Self {
        Self {
            id: TriggerId::new(id),
            interval_ns,
            priority: TriggerPriority::Low, // Background collection
            pre_roll_ns: 5_000_000_000,     // 5 seconds
            post_roll_ns: 25_000_000_000,   // 25 seconds (30s total episode)
            // Use MAX as sentinel for "never fired"
            last_fire_ns: AtomicU64::new(u64::MAX),
            enabled: AtomicBool::new(true),
        }
    }

    /// Create with interval in seconds
    pub fn with_interval_secs(id: impl Into<String>, interval_secs: u64) -> Self {
        Self::new(id, interval_secs * 1_000_000_000)
    }

    /// Set priority
    pub fn with_priority(mut self, priority: TriggerPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Set pre-roll duration
    pub fn with_pre_roll(mut self, duration_ns: u64) -> Self {
        self.pre_roll_ns = duration_ns;
        self
    }

    /// Set post-roll duration
    pub fn with_post_roll(mut self, duration_ns: u64) -> Self {
        self.post_roll_ns = duration_ns;
        self
    }
}

impl Trigger for PeriodicTrigger {
    fn id(&self) -> &TriggerId {
        &self.id
    }

    fn trigger_type(&self) -> TriggerType {
        TriggerType::Periodic
    }

    fn priority(&self) -> TriggerPriority {
        self.priority
    }

    fn evaluate(&self, context: &TriggerContext) -> Option<TriggerEvent> {
        let last = self.last_fire_ns.load(Ordering::SeqCst);

        // Use MAX as sentinel for "never fired"
        let should_fire = if last == u64::MAX {
            // Never fired before - fire immediately
            true
        } else {
            context.timestamp_ns >= last + self.interval_ns
        };

        if should_fire {
            self.last_fire_ns
                .store(context.timestamp_ns, Ordering::SeqCst);

            Some(
                TriggerEvent::new(
                    self.id.clone(),
                    TriggerType::Periodic,
                    context.timestamp_ns,
                    "Periodic capture",
                )
                .with_priority(self.priority)
                .with_pre_roll(self.pre_roll_ns)
                .with_post_roll(self.post_roll_ns)
                .with_metadata("interval_ns", self.interval_ns.to_string()),
            )
        } else {
            None
        }
    }

    fn reset(&self) {
        self.last_fire_ns.store(u64::MAX, Ordering::SeqCst);
    }

    fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
    }

    fn config_metadata(&self) -> HashMap<String, String> {
        let mut meta = HashMap::new();
        meta.insert("interval_ns".into(), self.interval_ns.to_string());
        meta.insert("pre_roll_ns".into(), self.pre_roll_ns.to_string());
        meta.insert("post_roll_ns".into(), self.post_roll_ns.to_string());
        meta
    }
}

/// Trigger that fires on manual API request
///
/// Allows operators to manually trigger episode capture.
pub struct ManualTrigger {
    id: TriggerId,
    priority: TriggerPriority,
    pre_roll_ns: u64,
    post_roll_ns: u64,
    /// Pending trigger request
    pending: AtomicBool,
    pending_reason: parking_lot::RwLock<Option<String>>,
    enabled: AtomicBool,
}

impl ManualTrigger {
    /// Create a new manual trigger
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: TriggerId::new(id),
            priority: TriggerPriority::Normal,
            pre_roll_ns: 5_000_000_000,  // 5 seconds
            post_roll_ns: 5_000_000_000, // 5 seconds
            pending: AtomicBool::new(false),
            pending_reason: parking_lot::RwLock::new(None),
            enabled: AtomicBool::new(true),
        }
    }

    /// Set priority
    pub fn with_priority(mut self, priority: TriggerPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Set pre-roll duration
    pub fn with_pre_roll(mut self, duration_ns: u64) -> Self {
        self.pre_roll_ns = duration_ns;
        self
    }

    /// Set post-roll duration
    pub fn with_post_roll(mut self, duration_ns: u64) -> Self {
        self.post_roll_ns = duration_ns;
        self
    }

    /// Request a manual trigger
    pub fn request(&self, reason: impl Into<String>) {
        *self.pending_reason.write() = Some(reason.into());
        self.pending.store(true, Ordering::SeqCst);
    }

    /// Check if there's a pending request
    pub fn has_pending(&self) -> bool {
        self.pending.load(Ordering::SeqCst)
    }

    /// Cancel a pending request
    pub fn cancel(&self) {
        self.pending.store(false, Ordering::SeqCst);
        *self.pending_reason.write() = None;
    }
}

impl Trigger for ManualTrigger {
    fn id(&self) -> &TriggerId {
        &self.id
    }

    fn trigger_type(&self) -> TriggerType {
        TriggerType::Manual
    }

    fn priority(&self) -> TriggerPriority {
        self.priority
    }

    fn evaluate(&self, context: &TriggerContext) -> Option<TriggerEvent> {
        if self.pending.swap(false, Ordering::SeqCst) {
            let reason = self
                .pending_reason
                .write()
                .take()
                .unwrap_or_else(|| "Manual trigger".into());

            Some(
                TriggerEvent::new(
                    self.id.clone(),
                    TriggerType::Manual,
                    context.timestamp_ns,
                    &reason,
                )
                .with_priority(self.priority)
                .with_pre_roll(self.pre_roll_ns)
                .with_post_roll(self.post_roll_ns)
                .with_metadata("manual_reason", reason),
            )
        } else {
            None
        }
    }

    fn reset(&self) {
        self.pending.store(false, Ordering::SeqCst);
        *self.pending_reason.write() = None;
    }

    fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
    }
}

/// Trigger based on a custom signal threshold
///
/// Generic trigger that fires when a named signal crosses a threshold.
pub struct SignalThresholdTrigger {
    id: TriggerId,
    signal_name: String,
    threshold: f64,
    comparison: Comparison,
    priority: TriggerPriority,
    pre_roll_ns: u64,
    post_roll_ns: u64,
    enabled: AtomicBool,
}

/// Comparison type for signal threshold
#[derive(Debug, Clone, Copy)]
pub enum Comparison {
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
    Equal,
}

impl Comparison {
    fn compare(&self, value: f64, threshold: f64) -> bool {
        match self {
            Self::GreaterThan => value > threshold,
            Self::GreaterThanOrEqual => value >= threshold,
            Self::LessThan => value < threshold,
            Self::LessThanOrEqual => value <= threshold,
            Self::Equal => (value - threshold).abs() < f64::EPSILON,
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Self::GreaterThan => ">",
            Self::GreaterThanOrEqual => ">=",
            Self::LessThan => "<",
            Self::LessThanOrEqual => "<=",
            Self::Equal => "==",
        }
    }
}

impl SignalThresholdTrigger {
    /// Create a new signal threshold trigger
    pub fn new(
        id: impl Into<String>,
        signal_name: impl Into<String>,
        comparison: Comparison,
        threshold: f64,
    ) -> Self {
        Self {
            id: TriggerId::new(id),
            signal_name: signal_name.into(),
            threshold,
            comparison,
            priority: TriggerPriority::Normal,
            pre_roll_ns: 5_000_000_000,
            post_roll_ns: 5_000_000_000,
            enabled: AtomicBool::new(true),
        }
    }

    /// Set priority
    pub fn with_priority(mut self, priority: TriggerPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Set pre-roll duration
    pub fn with_pre_roll(mut self, duration_ns: u64) -> Self {
        self.pre_roll_ns = duration_ns;
        self
    }

    /// Set post-roll duration
    pub fn with_post_roll(mut self, duration_ns: u64) -> Self {
        self.post_roll_ns = duration_ns;
        self
    }
}

impl Trigger for SignalThresholdTrigger {
    fn id(&self) -> &TriggerId {
        &self.id
    }

    fn trigger_type(&self) -> TriggerType {
        TriggerType::Custom
    }

    fn priority(&self) -> TriggerPriority {
        self.priority
    }

    fn evaluate(&self, context: &TriggerContext) -> Option<TriggerEvent> {
        if let Some(&value) = context.signals.get(&self.signal_name) {
            if self.comparison.compare(value, self.threshold) {
                return Some(
                    TriggerEvent::new(
                        self.id.clone(),
                        TriggerType::Custom,
                        context.timestamp_ns,
                        format!(
                            "Signal '{}' {} {} (value: {})",
                            self.signal_name,
                            self.comparison.as_str(),
                            self.threshold,
                            value
                        ),
                    )
                    .with_priority(self.priority)
                    .with_pre_roll(self.pre_roll_ns)
                    .with_post_roll(self.post_roll_ns)
                    .with_metadata("signal_name", &self.signal_name)
                    .with_metadata("signal_value", value.to_string())
                    .with_metadata("threshold", self.threshold.to_string()),
                );
            }
        }
        None
    }

    fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
    }

    fn config_metadata(&self) -> HashMap<String, String> {
        let mut meta = HashMap::new();
        meta.insert("signal_name".into(), self.signal_name.clone());
        meta.insert("threshold".into(), self.threshold.to_string());
        meta.insert("comparison".into(), self.comparison.as_str().into());
        meta
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_teleop_trigger() {
        let trigger = TeleopTrigger::new("teleop");

        // Not active initially
        let ctx = TriggerContext::new(1000).with_teleop(false);
        assert!(trigger.evaluate(&ctx).is_none());

        // Transition to active - should fire
        let ctx = TriggerContext::new(2000).with_teleop(true);
        let event = trigger.evaluate(&ctx);
        assert!(event.is_some());
        let event = event.unwrap();
        assert_eq!(event.trigger_type, TriggerType::Teleop);
        assert_eq!(event.priority, TriggerPriority::High);

        // Still active - should not fire again
        let ctx = TriggerContext::new(3000).with_teleop(true);
        assert!(trigger.evaluate(&ctx).is_none());

        // Back to inactive
        let ctx = TriggerContext::new(4000).with_teleop(false);
        assert!(trigger.evaluate(&ctx).is_none());

        // Active again - should fire
        let ctx = TriggerContext::new(5000).with_teleop(true);
        assert!(trigger.evaluate(&ctx).is_some());
    }

    #[test]
    fn test_failure_trigger() {
        let trigger = FailureTrigger::new("failure");

        // No failure
        let ctx = TriggerContext::new(1000);
        assert!(trigger.evaluate(&ctx).is_none());

        // Failure detected
        let ctx = TriggerContext::new(2000).with_failure(true, Some("Motor overheated".into()));
        let event = trigger.evaluate(&ctx);
        assert!(event.is_some());
        let event = event.unwrap();
        assert_eq!(event.trigger_type, TriggerType::Failure);
        assert_eq!(event.priority, TriggerPriority::Critical);
        assert!(event.reason.contains("Motor overheated"));
    }

    #[test]
    fn test_anomaly_trigger() {
        let trigger = AnomalyTrigger::new("anomaly", 0.7);

        // Low anomaly score
        let ctx = TriggerContext::new(1000).with_anomaly(0.3);
        assert!(trigger.evaluate(&ctx).is_none());

        // At threshold
        let ctx = TriggerContext::new(2000).with_anomaly(0.7);
        let event = trigger.evaluate(&ctx);
        assert!(event.is_some());

        // Above threshold
        let ctx = TriggerContext::new(3000).with_anomaly(0.9);
        let event = trigger.evaluate(&ctx);
        assert!(event.is_some());
        assert_eq!(event.unwrap().priority, TriggerPriority::High);
    }

    #[test]
    fn test_periodic_trigger() {
        let trigger = PeriodicTrigger::new("periodic", 1_000_000); // 1ms interval

        // First evaluation - should fire
        let ctx = TriggerContext::new(0);
        let event = trigger.evaluate(&ctx);
        assert!(event.is_some());

        // Before interval - should not fire
        let ctx = TriggerContext::new(500_000);
        assert!(trigger.evaluate(&ctx).is_none());

        // After interval - should fire
        let ctx = TriggerContext::new(1_500_000);
        let event = trigger.evaluate(&ctx);
        assert!(event.is_some());
        assert_eq!(event.unwrap().priority, TriggerPriority::Low);
    }

    #[test]
    fn test_manual_trigger() {
        let trigger = ManualTrigger::new("manual");

        // No pending request
        let ctx = TriggerContext::new(1000);
        assert!(trigger.evaluate(&ctx).is_none());
        assert!(!trigger.has_pending());

        // Request trigger
        trigger.request("User requested capture");
        assert!(trigger.has_pending());

        // Evaluate - should fire and clear pending
        let ctx = TriggerContext::new(2000);
        let event = trigger.evaluate(&ctx);
        assert!(event.is_some());
        assert!(!trigger.has_pending());
        assert!(event.unwrap().reason.contains("User requested"));

        // Should not fire again
        let ctx = TriggerContext::new(3000);
        assert!(trigger.evaluate(&ctx).is_none());
    }

    #[test]
    fn test_signal_threshold_trigger() {
        let trigger =
            SignalThresholdTrigger::new("temp_high", "temperature", Comparison::GreaterThan, 75.0);

        // Signal not present
        let ctx = TriggerContext::new(1000);
        assert!(trigger.evaluate(&ctx).is_none());

        // Signal below threshold
        let ctx = TriggerContext::new(2000).with_signal("temperature", 50.0);
        assert!(trigger.evaluate(&ctx).is_none());

        // Signal at threshold (greater than, not equal)
        let ctx = TriggerContext::new(3000).with_signal("temperature", 75.0);
        assert!(trigger.evaluate(&ctx).is_none());

        // Signal above threshold
        let ctx = TriggerContext::new(4000).with_signal("temperature", 80.0);
        let event = trigger.evaluate(&ctx);
        assert!(event.is_some());
        assert_eq!(event.unwrap().trigger_type, TriggerType::Custom);
    }

    #[test]
    fn test_comparison_types() {
        assert!(Comparison::GreaterThan.compare(5.0, 3.0));
        assert!(!Comparison::GreaterThan.compare(3.0, 5.0));

        assert!(Comparison::GreaterThanOrEqual.compare(5.0, 5.0));
        assert!(Comparison::GreaterThanOrEqual.compare(6.0, 5.0));

        assert!(Comparison::LessThan.compare(3.0, 5.0));
        assert!(!Comparison::LessThan.compare(5.0, 3.0));

        assert!(Comparison::LessThanOrEqual.compare(5.0, 5.0));
        assert!(Comparison::LessThanOrEqual.compare(4.0, 5.0));

        assert!(Comparison::Equal.compare(5.0, 5.0));
        assert!(!Comparison::Equal.compare(5.0, 5.1));
    }

    #[test]
    fn test_trigger_enable_disable() {
        let trigger = TeleopTrigger::new("teleop");

        assert!(trigger.is_enabled());

        trigger.set_enabled(false);
        assert!(!trigger.is_enabled());

        trigger.set_enabled(true);
        assert!(trigger.is_enabled());
    }

    #[test]
    fn test_trigger_reset() {
        let trigger = TeleopTrigger::new("teleop");

        // Activate teleop
        let ctx = TriggerContext::new(1000).with_teleop(true);
        trigger.evaluate(&ctx);

        // Reset
        trigger.reset();

        // Should fire again on activation
        let ctx = TriggerContext::new(2000).with_teleop(true);
        assert!(trigger.evaluate(&ctx).is_some());
    }
}
