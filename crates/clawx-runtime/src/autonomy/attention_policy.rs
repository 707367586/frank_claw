//! Attention Policy: notification filtering and feedback management.
//!
//! Decides whether a task run result should notify the user immediately,
//! be deferred to a digest, stored silently, or suppressed entirely.
//!
//! Aligned with autonomy-architecture.md §5.

use chrono::{DateTime, Utc};

use clawx_types::autonomy::*;

/// Configuration for quiet hours.
#[derive(Debug, Clone)]
pub struct QuietHoursConfig {
    /// Start hour (0-23).
    pub start_hour: u32,
    /// End hour (0-23).
    pub end_hour: u32,
}

impl Default for QuietHoursConfig {
    fn default() -> Self {
        Self {
            start_hour: 22,
            end_hour: 8,
        }
    }
}

/// Attention policy engine.
#[derive(Debug)]
pub struct AttentionPolicyEngine {
    pub quiet_hours: Option<QuietHoursConfig>,
    /// Cooldown period in seconds between same-task notifications.
    pub cooldown_secs: u64,
    /// Number of consecutive ignores before auto-pause.
    pub auto_pause_threshold: u32,
}

impl Default for AttentionPolicyEngine {
    fn default() -> Self {
        Self {
            quiet_hours: Some(QuietHoursConfig::default()),
            cooldown_secs: 3600,
            auto_pause_threshold: 3,
        }
    }
}

/// Context for attention evaluation.
#[derive(Debug)]
pub struct AttentionContext {
    pub trigger_kind: TriggerKind,
    pub run_status: RunStatus,
    pub consecutive_ignores: u32,
    pub last_notification_at: Option<DateTime<Utc>>,
    pub now: DateTime<Utc>,
}

impl AttentionPolicyEngine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Evaluate whether a notification should be sent.
    pub fn evaluate(&self, ctx: &AttentionContext) -> AttentionDecision {
        // Rule 1: mute_forever handled at Task level (archived), not here

        // Rule 2: Failed runs always notify (errors are important)
        if ctx.run_status == RunStatus::Failed {
            return AttentionDecision::SendNow;
        }

        // Rule 3: Auto-pause after consecutive ignores
        if ctx.consecutive_ignores >= self.auto_pause_threshold {
            return AttentionDecision::Suppress;
        }

        // Rule 4: Cooldown check
        if let Some(last) = ctx.last_notification_at {
            let elapsed = (ctx.now - last).num_seconds() as u64;
            if elapsed < self.cooldown_secs {
                return AttentionDecision::StoreOnly;
            }
        }

        // Rule 5: Quiet hours check
        if self.is_quiet_hours(ctx.now) {
            if ctx.run_status == RunStatus::Completed {
                return AttentionDecision::SendDigest;
            }
        }

        // Default: send now
        AttentionDecision::SendNow
    }

    /// Check if current time is within quiet hours.
    fn is_quiet_hours(&self, now: DateTime<Utc>) -> bool {
        let config = match &self.quiet_hours {
            Some(c) => c,
            None => return false,
        };

        let hour = now.format("%H").to_string().parse::<u32>().unwrap_or(0);

        if config.start_hour > config.end_hour {
            // Wraps around midnight: e.g., 22:00 - 08:00
            hour >= config.start_hour || hour < config.end_hour
        } else {
            // Same day: e.g., 01:00 - 06:00
            hour >= config.start_hour && hour < config.end_hour
        }
    }

    /// Process feedback and return any task-level action needed.
    pub fn process_feedback(&self, feedback: FeedbackKind) -> FeedbackAction {
        match feedback {
            FeedbackKind::Accepted => FeedbackAction::None,
            FeedbackKind::Ignored => FeedbackAction::IncrementIgnoreCount,
            FeedbackKind::Rejected => FeedbackAction::IncrementNegativeFeedback,
            FeedbackKind::MuteForever => FeedbackAction::ArchiveTask,
            FeedbackKind::ReduceFrequency => FeedbackAction::AdjustTriggerFrequency,
        }
    }
}

/// Action to take at the task level based on feedback.
#[derive(Debug, Clone, PartialEq)]
pub enum FeedbackAction {
    None,
    IncrementIgnoreCount,
    IncrementNegativeFeedback,
    ArchiveTask,
    AdjustTriggerFrequency,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn engine() -> AttentionPolicyEngine {
        AttentionPolicyEngine::default()
    }

    fn base_context() -> AttentionContext {
        // Use 14:00 UTC to avoid quiet hours (default 22:00-08:00)
        AttentionContext {
            trigger_kind: TriggerKind::Time,
            run_status: RunStatus::Completed,
            consecutive_ignores: 0,
            last_notification_at: None,
            now: Utc.with_ymd_and_hms(2026, 3, 20, 14, 0, 0).unwrap(),
        }
    }

    // -----------------------------------------------------------------------
    // Basic decisions
    // -----------------------------------------------------------------------

    #[test]
    fn normal_completed_sends_now() {
        let policy = engine();
        let ctx = base_context();
        assert_eq!(policy.evaluate(&ctx), AttentionDecision::SendNow);
    }

    #[test]
    fn failed_run_always_sends_now() {
        let policy = engine();
        let mut ctx = base_context();
        ctx.run_status = RunStatus::Failed;
        ctx.consecutive_ignores = 10; // Even with many ignores
        assert_eq!(policy.evaluate(&ctx), AttentionDecision::SendNow);
    }

    // -----------------------------------------------------------------------
    // Auto-pause after ignores
    // -----------------------------------------------------------------------

    #[test]
    fn auto_pause_after_three_ignores() {
        let policy = engine();
        let mut ctx = base_context();
        ctx.consecutive_ignores = 3;
        assert_eq!(policy.evaluate(&ctx), AttentionDecision::Suppress);
    }

    #[test]
    fn two_ignores_still_sends() {
        let policy = engine();
        let mut ctx = base_context();
        ctx.consecutive_ignores = 2;
        // Not yet at threshold
        assert_ne!(policy.evaluate(&ctx), AttentionDecision::Suppress);
    }

    // -----------------------------------------------------------------------
    // Cooldown
    // -----------------------------------------------------------------------

    #[test]
    fn within_cooldown_stores_only() {
        let policy = engine();
        let mut ctx = base_context();
        ctx.last_notification_at = Some(ctx.now - chrono::Duration::seconds(60));
        // 60 seconds < 3600 second cooldown
        assert_eq!(policy.evaluate(&ctx), AttentionDecision::StoreOnly);
    }

    #[test]
    fn past_cooldown_sends_now() {
        let policy = engine();
        let mut ctx = base_context();
        ctx.last_notification_at = Some(ctx.now - chrono::Duration::seconds(7200));
        // 7200 seconds > 3600 second cooldown
        assert_eq!(policy.evaluate(&ctx), AttentionDecision::SendNow);
    }

    // -----------------------------------------------------------------------
    // Quiet hours
    // -----------------------------------------------------------------------

    #[test]
    fn quiet_hours_digest() {
        let policy = engine();
        // 23:00 UTC is within default quiet hours (22:00-08:00)
        let mut ctx = base_context();
        ctx.now = Utc.with_ymd_and_hms(2026, 3, 20, 23, 0, 0).unwrap();
        assert_eq!(policy.evaluate(&ctx), AttentionDecision::SendDigest);
    }

    #[test]
    fn outside_quiet_hours_sends_now() {
        let policy = engine();
        // 14:00 UTC is outside quiet hours
        let mut ctx = base_context();
        ctx.now = Utc.with_ymd_and_hms(2026, 3, 20, 14, 0, 0).unwrap();
        assert_eq!(policy.evaluate(&ctx), AttentionDecision::SendNow);
    }

    #[test]
    fn no_quiet_hours_sends_now() {
        let mut policy = engine();
        policy.quiet_hours = None;
        let mut ctx = base_context();
        ctx.now = Utc.with_ymd_and_hms(2026, 3, 20, 23, 0, 0).unwrap();
        assert_eq!(policy.evaluate(&ctx), AttentionDecision::SendNow);
    }

    // -----------------------------------------------------------------------
    // Feedback processing
    // -----------------------------------------------------------------------

    #[test]
    fn feedback_accepted() {
        let policy = engine();
        assert_eq!(policy.process_feedback(FeedbackKind::Accepted), FeedbackAction::None);
    }

    #[test]
    fn feedback_ignored() {
        let policy = engine();
        assert_eq!(
            policy.process_feedback(FeedbackKind::Ignored),
            FeedbackAction::IncrementIgnoreCount
        );
    }

    #[test]
    fn feedback_rejected() {
        let policy = engine();
        assert_eq!(
            policy.process_feedback(FeedbackKind::Rejected),
            FeedbackAction::IncrementNegativeFeedback
        );
    }

    #[test]
    fn feedback_mute_forever() {
        let policy = engine();
        assert_eq!(
            policy.process_feedback(FeedbackKind::MuteForever),
            FeedbackAction::ArchiveTask
        );
    }

    #[test]
    fn feedback_reduce_frequency() {
        let policy = engine();
        assert_eq!(
            policy.process_feedback(FeedbackKind::ReduceFrequency),
            FeedbackAction::AdjustTriggerFrequency
        );
    }

    // -----------------------------------------------------------------------
    // Priority: failed > ignore-threshold > cooldown > quiet hours
    // -----------------------------------------------------------------------

    #[test]
    fn failed_overrides_cooldown() {
        let policy = engine();
        let mut ctx = base_context();
        ctx.run_status = RunStatus::Failed;
        ctx.last_notification_at = Some(ctx.now - chrono::Duration::seconds(60));
        // Failed takes priority over cooldown
        assert_eq!(policy.evaluate(&ctx), AttentionDecision::SendNow);
    }

    #[test]
    fn ignore_threshold_overrides_cooldown() {
        let policy = engine();
        let mut ctx = base_context();
        ctx.consecutive_ignores = 3;
        ctx.last_notification_at = None; // No cooldown issue
        // Auto-pause takes priority
        assert_eq!(policy.evaluate(&ctx), AttentionDecision::Suppress);
    }
}
