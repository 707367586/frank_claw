//! GCRA (Generic Cell Rate Algorithm) rate limiter.
//!
//! Provides token-bucket-style rate limiting with configurable rate and burst.
//! Each key (e.g., agent_id) has its own limiter state.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Configuration for a GCRA rate limiter.
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Time between allowed requests (emission interval).
    pub emission_interval: Duration,
    /// Maximum burst size (how many requests can be made at once).
    pub burst_size: u32,
}

impl RateLimitConfig {
    /// Create a rate limit config from requests-per-second and burst size.
    pub fn new(requests_per_second: f64, burst_size: u32) -> Self {
        let emission_interval = Duration::from_secs_f64(1.0 / requests_per_second);
        Self {
            emission_interval,
            burst_size,
        }
    }
}

/// Result of a rate limit check.
#[derive(Debug, Clone, PartialEq)]
pub enum RateLimitResult {
    /// Request is allowed.
    Allowed,
    /// Request is denied. `retry_after` indicates when the next request will be allowed.
    Denied { retry_after: Duration },
}

/// Per-key GCRA state.
struct GcraState {
    /// Theoretical Arrival Time — when the next cell can arrive without violating the rate.
    tat: Instant,
}

/// A thread-safe GCRA rate limiter.
pub struct GcraRateLimiter {
    config: RateLimitConfig,
    states: Mutex<HashMap<String, GcraState>>,
}

impl GcraRateLimiter {
    /// Create a new rate limiter with the given configuration.
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            states: Mutex::new(HashMap::new()),
        }
    }

    /// Check if a request for the given key is allowed.
    ///
    /// Returns `Allowed` if the request can proceed, or `Denied` with a retry-after duration.
    pub fn check(&self, key: &str) -> RateLimitResult {
        self.check_at(key, Instant::now())
    }

    /// Check at a specific time (for testing).
    fn check_at(&self, key: &str, now: Instant) -> RateLimitResult {
        let mut states = self.states.lock().unwrap();
        let emission_interval = self.config.emission_interval;
        let burst_tolerance = emission_interval * self.config.burst_size;

        let state = states.entry(key.to_string()).or_insert(GcraState { tat: now });

        // If TAT is in the past, reset to now
        let tat = if state.tat < now { now } else { state.tat };

        // New TAT if we allow this request
        let new_tat = tat + emission_interval;

        // Check if the new TAT exceeds the burst window
        let allow_at = new_tat - burst_tolerance - emission_interval;

        if allow_at <= now {
            // Allowed — update TAT
            state.tat = new_tat;
            RateLimitResult::Allowed
        } else {
            // Denied — too many requests
            let retry_after = allow_at - now;
            RateLimitResult::Denied { retry_after }
        }
    }

    /// Remove state for a key (e.g., when an agent is deleted).
    pub fn remove(&self, key: &str) {
        self.states.lock().unwrap().remove(key);
    }

    /// Get the number of tracked keys.
    pub fn tracked_keys(&self) -> usize {
        self.states.lock().unwrap().len()
    }
}

impl std::fmt::Debug for GcraRateLimiter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GcraRateLimiter")
            .field("config", &self.config)
            .field("tracked_keys", &self.tracked_keys())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_limiter(rps: f64, burst: u32) -> GcraRateLimiter {
        GcraRateLimiter::new(RateLimitConfig::new(rps, burst))
    }

    #[test]
    fn first_request_is_always_allowed() {
        let limiter = make_limiter(10.0, 1);
        assert_eq!(limiter.check("agent-1"), RateLimitResult::Allowed);
    }

    #[test]
    fn burst_requests_allowed_up_to_burst_size() {
        let limiter = make_limiter(1.0, 3); // 1 rps, burst of 3
        let now = Instant::now();

        // First request + burst of 3 = 4 total allowed
        assert_eq!(limiter.check_at("a", now), RateLimitResult::Allowed);
        assert_eq!(limiter.check_at("a", now), RateLimitResult::Allowed);
        assert_eq!(limiter.check_at("a", now), RateLimitResult::Allowed);
        assert_eq!(limiter.check_at("a", now), RateLimitResult::Allowed);

        // 5th request should be denied
        let result = limiter.check_at("a", now);
        assert!(matches!(result, RateLimitResult::Denied { .. }));
    }

    #[test]
    fn denied_includes_retry_after() {
        let limiter = make_limiter(1.0, 0); // 1 rps, no burst
        let now = Instant::now();

        assert_eq!(limiter.check_at("a", now), RateLimitResult::Allowed);

        // Immediate second request should be denied
        let result = limiter.check_at("a", now);
        match result {
            RateLimitResult::Denied { retry_after } => {
                assert!(retry_after.as_millis() > 0);
                assert!(retry_after <= Duration::from_secs(1));
            }
            _ => panic!("expected Denied"),
        }
    }

    #[test]
    fn requests_allowed_after_interval() {
        let limiter = make_limiter(10.0, 0); // 10 rps, no burst
        let now = Instant::now();

        assert_eq!(limiter.check_at("a", now), RateLimitResult::Allowed);

        // After 100ms (= 1/10s), another should be allowed
        let later = now + Duration::from_millis(100);
        assert_eq!(limiter.check_at("a", later), RateLimitResult::Allowed);
    }

    #[test]
    fn different_keys_are_independent() {
        let limiter = make_limiter(1.0, 0); // 1 rps, no burst
        let now = Instant::now();

        assert_eq!(limiter.check_at("agent-1", now), RateLimitResult::Allowed);
        assert_eq!(limiter.check_at("agent-2", now), RateLimitResult::Allowed);

        // agent-1 should be denied, agent-2 should still work
        let result = limiter.check_at("agent-1", now);
        assert!(matches!(result, RateLimitResult::Denied { .. }));
    }

    #[test]
    fn remove_clears_key_state() {
        let limiter = make_limiter(1.0, 0);
        let now = Instant::now();

        assert_eq!(limiter.check_at("a", now), RateLimitResult::Allowed);
        let result = limiter.check_at("a", now);
        assert!(matches!(result, RateLimitResult::Denied { .. }));

        // After removing, should be allowed again
        limiter.remove("a");
        assert_eq!(limiter.check_at("a", now), RateLimitResult::Allowed);
    }

    #[test]
    fn tracked_keys_counts_active_entries() {
        let limiter = make_limiter(10.0, 1);
        assert_eq!(limiter.tracked_keys(), 0);

        limiter.check("a");
        assert_eq!(limiter.tracked_keys(), 1);

        limiter.check("b");
        assert_eq!(limiter.tracked_keys(), 2);

        limiter.remove("a");
        assert_eq!(limiter.tracked_keys(), 1);
    }

    #[test]
    fn high_rate_allows_many_requests() {
        let limiter = make_limiter(1000.0, 10); // 1000 rps, burst 10
        let now = Instant::now();

        // Should allow 11 requests (1 + burst of 10)
        for _ in 0..11 {
            assert_eq!(limiter.check_at("a", now), RateLimitResult::Allowed);
        }

        // 12th should be denied
        let result = limiter.check_at("a", now);
        assert!(matches!(result, RateLimitResult::Denied { .. }));
    }

    #[test]
    fn recovery_after_time_passes() {
        let limiter = make_limiter(2.0, 0); // 2 rps, no burst
        let now = Instant::now();

        // Use up the allowance
        assert_eq!(limiter.check_at("a", now), RateLimitResult::Allowed);
        assert!(matches!(limiter.check_at("a", now), RateLimitResult::Denied { .. }));

        // 500ms later (= 1/2s), should be allowed again
        let later = now + Duration::from_millis(500);
        assert_eq!(limiter.check_at("a", later), RateLimitResult::Allowed);
    }
}
