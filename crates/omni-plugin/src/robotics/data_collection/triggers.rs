//! Trigger system for episode capture
//!
//! Triggers determine when to start and stop recording episodes.
//! Multiple triggers can be active simultaneously, and any trigger
//! firing will start episode capture.

use super::types::TimestampNs;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

/// Trigger-related errors
#[derive(Debug, Error)]
pub enum TriggerError {
    #[error("Trigger not found: {0}")]
    NotFound(String),

    #[error("Trigger already registered: {0}")]
    AlreadyRegistered(String),

    #[error("Invalid trigger configuration: {0}")]
    InvalidConfig(String),

    #[error("Trigger evaluation failed: {0}")]
    EvaluationFailed(String),
}

/// Unique identifier for a trigger
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TriggerId(String);

impl TriggerId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for TriggerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Type of trigger
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerType {
    /// Fires when teleoperation starts
    Teleop,
    /// Fires on detected failures
    Failure,
    /// Fires on high anomaly scores
    Anomaly,
    /// Fires at regular intervals
    Periodic,
    /// Fires on manual API request
    Manual,
    /// Custom trigger with user-defined logic
    Custom,
}

impl TriggerType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Teleop => "teleop",
            Self::Failure => "failure",
            Self::Anomaly => "anomaly",
            Self::Periodic => "periodic",
            Self::Manual => "manual",
            Self::Custom => "custom",
        }
    }
}

/// Priority level for triggers
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TriggerPriority {
    /// Low priority - background collection
    Low = 0,
    /// Normal priority - standard triggers
    Normal = 1,
    /// High priority - important events
    High = 2,
    /// Critical priority - must capture
    Critical = 3,
}

impl Default for TriggerPriority {
    fn default() -> Self {
        Self::Normal
    }
}

/// Event fired when a trigger activates
#[derive(Debug, Clone)]
pub struct TriggerEvent {
    /// ID of the trigger that fired
    pub trigger_id: TriggerId,
    /// Type of trigger
    pub trigger_type: TriggerType,
    /// Timestamp when the trigger fired
    pub timestamp_ns: TimestampNs,
    /// Priority of this trigger
    pub priority: TriggerPriority,
    /// Reason or description for the trigger
    pub reason: String,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
    /// Suggested pre-roll duration in nanoseconds
    pub pre_roll_ns: u64,
    /// Suggested post-roll duration in nanoseconds
    pub post_roll_ns: u64,
}

impl TriggerEvent {
    /// Create a new trigger event
    pub fn new(
        trigger_id: TriggerId,
        trigger_type: TriggerType,
        timestamp_ns: TimestampNs,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            trigger_id,
            trigger_type,
            timestamp_ns,
            priority: TriggerPriority::default(),
            reason: reason.into(),
            metadata: HashMap::new(),
            pre_roll_ns: 5_000_000_000,  // 5 seconds default
            post_roll_ns: 5_000_000_000, // 5 seconds default
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

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Get total episode duration suggestion
    pub fn suggested_duration_ns(&self) -> u64 {
        self.pre_roll_ns + self.post_roll_ns
    }
}

/// Context passed to triggers for evaluation
#[derive(Debug, Clone)]
pub struct TriggerContext {
    /// Current timestamp
    pub timestamp_ns: TimestampNs,
    /// Current teleoperation state
    pub is_teleop_active: bool,
    /// Current anomaly score (0.0 - 1.0)
    pub anomaly_score: f32,
    /// Whether a failure was detected
    pub failure_detected: bool,
    /// Failure description if detected
    pub failure_reason: Option<String>,
    /// Time since last trigger in nanoseconds
    pub time_since_last_trigger_ns: u64,
    /// Custom signals for trigger evaluation
    pub signals: HashMap<String, f64>,
}

impl Default for TriggerContext {
    fn default() -> Self {
        Self {
            timestamp_ns: 0,
            is_teleop_active: false,
            anomaly_score: 0.0,
            failure_detected: false,
            failure_reason: None,
            time_since_last_trigger_ns: u64::MAX,
            signals: HashMap::new(),
        }
    }
}

impl TriggerContext {
    /// Create a new context with timestamp
    pub fn new(timestamp_ns: TimestampNs) -> Self {
        Self {
            timestamp_ns,
            ..Default::default()
        }
    }

    /// Set teleop state
    pub fn with_teleop(mut self, active: bool) -> Self {
        self.is_teleop_active = active;
        self
    }

    /// Set anomaly score
    pub fn with_anomaly(mut self, score: f32) -> Self {
        self.anomaly_score = score.clamp(0.0, 1.0);
        self
    }

    /// Set failure state
    pub fn with_failure(mut self, detected: bool, reason: Option<String>) -> Self {
        self.failure_detected = detected;
        self.failure_reason = reason;
        self
    }

    /// Add a custom signal
    pub fn with_signal(mut self, name: impl Into<String>, value: f64) -> Self {
        self.signals.insert(name.into(), value);
        self
    }
}

/// Trait for trigger implementations
pub trait Trigger: Send + Sync {
    /// Get the trigger ID
    fn id(&self) -> &TriggerId;

    /// Get the trigger type
    fn trigger_type(&self) -> TriggerType;

    /// Get the trigger priority
    fn priority(&self) -> TriggerPriority {
        TriggerPriority::Normal
    }

    /// Evaluate whether this trigger should fire
    fn evaluate(&self, context: &TriggerContext) -> Option<TriggerEvent>;

    /// Reset the trigger state (e.g., after episode ends)
    fn reset(&self) {}

    /// Check if trigger is enabled
    fn is_enabled(&self) -> bool {
        true
    }

    /// Enable or disable the trigger
    fn set_enabled(&self, _enabled: bool) {}

    /// Get trigger configuration as metadata
    fn config_metadata(&self) -> HashMap<String, String> {
        HashMap::new()
    }
}

/// Manages multiple triggers and evaluates them
pub struct TriggerManager {
    /// Registered triggers
    triggers: HashMap<TriggerId, Arc<dyn Trigger>>,
    /// Whether manager is active
    active: bool,
    /// Last trigger time per trigger ID
    last_trigger_times: parking_lot::RwLock<HashMap<TriggerId, TimestampNs>>,
    /// Minimum time between triggers from same source (cooldown)
    cooldown_ns: u64,
    /// Global enable/disable
    enabled: std::sync::atomic::AtomicBool,
}

impl TriggerManager {
    /// Create a new trigger manager
    pub fn new() -> Self {
        Self {
            triggers: HashMap::new(),
            active: true,
            last_trigger_times: parking_lot::RwLock::new(HashMap::new()),
            cooldown_ns: 1_000_000_000, // 1 second default cooldown
            enabled: std::sync::atomic::AtomicBool::new(true),
        }
    }

    /// Set cooldown between triggers
    pub fn with_cooldown(mut self, cooldown_ns: u64) -> Self {
        self.cooldown_ns = cooldown_ns;
        self
    }

    /// Register a trigger
    pub fn register(&mut self, trigger: Arc<dyn Trigger>) -> Result<(), TriggerError> {
        let id = trigger.id().clone();
        if self.triggers.contains_key(&id) {
            return Err(TriggerError::AlreadyRegistered(id.to_string()));
        }
        self.triggers.insert(id, trigger);
        Ok(())
    }

    /// Unregister a trigger
    pub fn unregister(&mut self, id: &TriggerId) -> Result<(), TriggerError> {
        self.triggers
            .remove(id)
            .map(|_| ())
            .ok_or_else(|| TriggerError::NotFound(id.to_string()))
    }

    /// Get a trigger by ID
    pub fn get(&self, id: &TriggerId) -> Option<Arc<dyn Trigger>> {
        self.triggers.get(id).cloned()
    }

    /// List all registered trigger IDs
    pub fn list_triggers(&self) -> Vec<TriggerId> {
        self.triggers.keys().cloned().collect()
    }

    /// Enable the manager
    pub fn enable(&self) {
        self.enabled
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// Disable the manager
    pub fn disable(&self) {
        self.enabled
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }

    /// Check if manager is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Evaluate all triggers and return any that fired
    pub fn evaluate(&self, context: &TriggerContext) -> Vec<TriggerEvent> {
        if !self.is_enabled() {
            return Vec::new();
        }

        let mut events = Vec::new();
        let last_times = self.last_trigger_times.read();

        for (id, trigger) in &self.triggers {
            if !trigger.is_enabled() {
                continue;
            }

            // Check cooldown
            if let Some(&last_time) = last_times.get(id) {
                if context.timestamp_ns < last_time + self.cooldown_ns {
                    continue;
                }
            }

            // Evaluate the trigger
            if let Some(event) = trigger.evaluate(context) {
                events.push(event);
            }
        }

        // Sort by priority (highest first)
        events.sort_by(|a, b| b.priority.cmp(&a.priority));

        events
    }

    /// Record that triggers fired (for cooldown tracking)
    pub fn record_trigger_fired(&self, events: &[TriggerEvent]) {
        let mut last_times = self.last_trigger_times.write();
        for event in events {
            last_times.insert(event.trigger_id.clone(), event.timestamp_ns);
        }
    }

    /// Reset all triggers
    pub fn reset_all(&self) {
        for trigger in self.triggers.values() {
            trigger.reset();
        }
        self.last_trigger_times.write().clear();
    }

    /// Get statistics about triggers
    pub fn stats(&self) -> TriggerStats {
        let last_times = self.last_trigger_times.read();
        TriggerStats {
            total_triggers: self.triggers.len(),
            enabled_triggers: self.triggers.values().filter(|t| t.is_enabled()).count(),
            triggers_fired: last_times.len(),
        }
    }
}

impl Default for TriggerManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about trigger manager
#[derive(Debug, Clone)]
pub struct TriggerStats {
    pub total_triggers: usize,
    pub enabled_triggers: usize,
    pub triggers_fired: usize,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Simple test trigger that always fires
    struct AlwaysTrigger {
        id: TriggerId,
    }

    impl AlwaysTrigger {
        fn new(id: &str) -> Self {
            Self {
                id: TriggerId::new(id),
            }
        }
    }

    impl Trigger for AlwaysTrigger {
        fn id(&self) -> &TriggerId {
            &self.id
        }

        fn trigger_type(&self) -> TriggerType {
            TriggerType::Manual
        }

        fn evaluate(&self, context: &TriggerContext) -> Option<TriggerEvent> {
            Some(TriggerEvent::new(
                self.id.clone(),
                TriggerType::Manual,
                context.timestamp_ns,
                "Always fires",
            ))
        }
    }

    /// Test trigger that fires on condition
    struct ThresholdTrigger {
        id: TriggerId,
        threshold: f32,
    }

    impl ThresholdTrigger {
        fn new(id: &str, threshold: f32) -> Self {
            Self {
                id: TriggerId::new(id),
                threshold,
            }
        }
    }

    impl Trigger for ThresholdTrigger {
        fn id(&self) -> &TriggerId {
            &self.id
        }

        fn trigger_type(&self) -> TriggerType {
            TriggerType::Anomaly
        }

        fn evaluate(&self, context: &TriggerContext) -> Option<TriggerEvent> {
            if context.anomaly_score >= self.threshold {
                Some(
                    TriggerEvent::new(
                        self.id.clone(),
                        TriggerType::Anomaly,
                        context.timestamp_ns,
                        format!(
                            "Anomaly score {} >= {}",
                            context.anomaly_score, self.threshold
                        ),
                    )
                    .with_priority(TriggerPriority::High),
                )
            } else {
                None
            }
        }
    }

    #[test]
    fn test_trigger_event_creation() {
        let event = TriggerEvent::new(
            TriggerId::new("test"),
            TriggerType::Manual,
            1000,
            "Test event",
        )
        .with_priority(TriggerPriority::High)
        .with_pre_roll(10_000_000_000)
        .with_post_roll(5_000_000_000)
        .with_metadata("key", "value");

        assert_eq!(event.trigger_id.as_str(), "test");
        assert_eq!(event.priority, TriggerPriority::High);
        assert_eq!(event.pre_roll_ns, 10_000_000_000);
        assert_eq!(event.post_roll_ns, 5_000_000_000);
        assert_eq!(event.metadata.get("key"), Some(&"value".to_string()));
        assert_eq!(event.suggested_duration_ns(), 15_000_000_000);
    }

    #[test]
    fn test_trigger_context() {
        let context = TriggerContext::new(1000)
            .with_teleop(true)
            .with_anomaly(0.75)
            .with_failure(true, Some("Test failure".into()))
            .with_signal("temperature", 42.0);

        assert_eq!(context.timestamp_ns, 1000);
        assert!(context.is_teleop_active);
        assert_eq!(context.anomaly_score, 0.75);
        assert!(context.failure_detected);
        assert_eq!(context.failure_reason, Some("Test failure".to_string()));
        assert_eq!(context.signals.get("temperature"), Some(&42.0));
    }

    #[test]
    fn test_trigger_manager_registration() {
        let mut manager = TriggerManager::new();

        let trigger1 = Arc::new(AlwaysTrigger::new("trigger1"));
        let trigger2 = Arc::new(AlwaysTrigger::new("trigger2"));

        assert!(manager.register(trigger1.clone()).is_ok());
        assert!(manager.register(trigger2).is_ok());

        // Duplicate registration should fail
        assert!(matches!(
            manager.register(trigger1),
            Err(TriggerError::AlreadyRegistered(_))
        ));

        assert_eq!(manager.list_triggers().len(), 2);
    }

    #[test]
    fn test_trigger_manager_evaluate() {
        let mut manager = TriggerManager::new().with_cooldown(0); // No cooldown for test

        manager
            .register(Arc::new(AlwaysTrigger::new("always")))
            .unwrap();
        manager
            .register(Arc::new(ThresholdTrigger::new("threshold", 0.5)))
            .unwrap();

        // Low anomaly - only "always" fires
        let context = TriggerContext::new(1000).with_anomaly(0.3);
        let events = manager.evaluate(&context);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].trigger_id.as_str(), "always");

        // High anomaly - both fire
        let context = TriggerContext::new(2000).with_anomaly(0.8);
        let events = manager.evaluate(&context);
        assert_eq!(events.len(), 2);

        // Check priority ordering (High should come first)
        assert_eq!(events[0].priority, TriggerPriority::High);
    }

    #[test]
    fn test_trigger_cooldown() {
        let mut manager = TriggerManager::new().with_cooldown(1_000_000); // 1ms cooldown

        manager
            .register(Arc::new(AlwaysTrigger::new("always")))
            .unwrap();

        let context1 = TriggerContext::new(1000);
        let events = manager.evaluate(&context1);
        assert_eq!(events.len(), 1);

        // Record that trigger fired
        manager.record_trigger_fired(&events);

        // Evaluate again immediately - should be blocked by cooldown
        let context2 = TriggerContext::new(1500);
        let events = manager.evaluate(&context2);
        assert_eq!(events.len(), 0);

        // Evaluate after cooldown - should fire again
        let context3 = TriggerContext::new(2_000_000);
        let events = manager.evaluate(&context3);
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_trigger_manager_enable_disable() {
        let mut manager = TriggerManager::new();
        manager
            .register(Arc::new(AlwaysTrigger::new("always")))
            .unwrap();

        assert!(manager.is_enabled());

        let events = manager.evaluate(&TriggerContext::new(1000));
        assert_eq!(events.len(), 1);

        manager.disable();
        assert!(!manager.is_enabled());

        let events = manager.evaluate(&TriggerContext::new(2000));
        assert_eq!(events.len(), 0);

        manager.enable();
        let events = manager.evaluate(&TriggerContext::new(3000));
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_trigger_priority_ordering() {
        assert!(TriggerPriority::Critical > TriggerPriority::High);
        assert!(TriggerPriority::High > TriggerPriority::Normal);
        assert!(TriggerPriority::Normal > TriggerPriority::Low);
    }

    #[test]
    fn test_trigger_type_as_str() {
        assert_eq!(TriggerType::Teleop.as_str(), "teleop");
        assert_eq!(TriggerType::Failure.as_str(), "failure");
        assert_eq!(TriggerType::Anomaly.as_str(), "anomaly");
        assert_eq!(TriggerType::Periodic.as_str(), "periodic");
        assert_eq!(TriggerType::Manual.as_str(), "manual");
        assert_eq!(TriggerType::Custom.as_str(), "custom");
    }
}
