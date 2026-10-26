//! Process-local fixed-window rate limiter (docs/PHASE5_PLAN.md, WP A).
//!
//! Why fixed window: one window per key, opened by the first hit and closed by
//! the clock. No token bucket, no background sweeper, no dependency — the
//! behaviour is easy to state, test and explain ("5 per 10 minutes").
//!
//! ponytail: the counters live in this process only. A restart forgets them
//! and a second replica would keep its own copy. That is accepted for the
//! single-instance pre-launch box because the expensive action (an SMS) is
//! ALSO bounded by the database-backed per-phone cap (`otp_challenges`).
//! Upgrade path when replicas exist: move `hit` behind a shared store
//! (Redis / a `rate_limits` table) — the call sites do not change.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

/// Pruning is opportunistic: it only runs when the map has grown past this
/// many keys, so a normal load never pays for a full scan.
const PRUNE_THRESHOLD: usize = 4096;

/// Cheap to clone (handlers get one per request): all clones share the map.
#[derive(Clone, Default)]
pub struct RateLimiter {
    windows: Arc<Mutex<HashMap<String, Window>>>,
}

struct Window {
    started: Instant,
    /// The window length is stored per entry so pruning can judge each key by
    /// its own window instead of the window of whoever happens to call next.
    window: Duration,
    hits: u32,
}

impl Window {
    fn expired(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.started) >= self.window
    }
}

/// The answer a handler needs: either proceed, or 429 with a `Retry-After`.
#[derive(Debug, PartialEq, Eq)]
pub enum LimitOutcome {
    Allowed,
    Limited { retry_after_secs: u64 },
}

impl LimitOutcome {
    /// `Err(429 rate_limited ...)` when the window is exhausted, so handlers
    /// stay linear (`?` instead of a match per call site).
    pub fn into_result(self) -> Result<(), crate::error::ApiError> {
        match self {
            LimitOutcome::Allowed => Ok(()),
            LimitOutcome::Limited { retry_after_secs } => {
                Err(crate::error::ApiError::RateLimited {
                    retry_after_secs: Some(retry_after_secs),
                })
            }
        }
    }
}

impl RateLimiter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Counts one hit for `key` inside the current window.
    ///
    /// `limit == 0` or a zero window disables the check (useful in tests and
    /// for an operator who deliberately turns a limit off with env vars).
    pub fn hit(&self, key: &str, limit: u32, window: Duration) -> LimitOutcome {
        if limit == 0 || window.is_zero() {
            return LimitOutcome::Allowed;
        }
        let now = Instant::now();
        let mut windows = self.windows.lock().unwrap_or_else(PoisonError::into_inner);
        if windows.len() > PRUNE_THRESHOLD {
            Self::prune_expired_in(&mut windows, now);
        }
        match windows.get_mut(key) {
            Some(w) if !w.expired(now) => {
                if w.hits >= limit {
                    return LimitOutcome::Limited {
                        retry_after_secs: retry_after_secs(w, now),
                    };
                }
                w.hits += 1;
                LimitOutcome::Allowed
            }
            // First hit, or the previous window ran out: start a fresh one.
            _ => {
                windows.insert(
                    key.to_string(),
                    Window {
                        started: now,
                        window,
                        hits: 1,
                    },
                );
                LimitOutcome::Allowed
            }
        }
    }

    /// Drops every window that has run out. Called opportunistically from
    /// `hit`; the rule lives in one place so it is testable on its own.
    fn prune_expired_in(windows: &mut HashMap<String, Window>, now: Instant) {
        windows.retain(|_, w| !w.expired(now));
    }

    #[cfg(test)]
    fn prune_expired(&self) {
        let now = Instant::now();
        let mut windows = self.windows.lock().unwrap_or_else(PoisonError::into_inner);
        Self::prune_expired_in(&mut windows, now);
    }

    /// Number of tracked keys. Test-only: production code never needs it.
    #[cfg(test)]
    fn tracked_keys(&self) -> usize {
        self.windows
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }
}

/// Whole seconds left in the window, rounded up, never below 1: a client that
/// retries immediately must not spin on `Retry-After: 0`.
fn retry_after_secs(window: &Window, now: Instant) -> u64 {
    let left = window
        .window
        .saturating_sub(now.saturating_duration_since(window.started));
    let millis = left.as_millis();
    u64::try_from(millis.div_ceil(1000))
        .unwrap_or(u64::MAX)
        .max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "otp:req:ip:203.0.113.9";

    #[test]
    fn allows_up_to_the_limit_then_limits_with_retry_after() {
        let limiter = RateLimiter::new();
        let window = Duration::from_secs(60);
        assert_eq!(limiter.hit(KEY, 2, window), LimitOutcome::Allowed);
        assert_eq!(limiter.hit(KEY, 2, window), LimitOutcome::Allowed);
        match limiter.hit(KEY, 2, window) {
            LimitOutcome::Limited { retry_after_secs } => {
                assert!(
                    (1..=60).contains(&retry_after_secs),
                    "retry_after must be inside the window, got {retry_after_secs}"
                );
            }
            LimitOutcome::Allowed => panic!("third hit with limit 2 must be limited"),
        }
    }

    #[test]
    fn separate_keys_have_separate_windows() {
        let limiter = RateLimiter::new();
        let window = Duration::from_secs(60);
        assert_eq!(limiter.hit("a", 1, window), LimitOutcome::Allowed);
        assert!(matches!(
            limiter.hit("a", 1, window),
            LimitOutcome::Limited { .. }
        ));
        assert_eq!(
            limiter.hit("b", 1, window),
            LimitOutcome::Allowed,
            "another key must not inherit the first key's window"
        );
    }

    #[test]
    fn expired_window_allows_again() {
        let limiter = RateLimiter::new();
        let window = Duration::from_millis(50);
        assert_eq!(limiter.hit(KEY, 1, window), LimitOutcome::Allowed);
        assert!(matches!(
            limiter.hit(KEY, 1, window),
            LimitOutcome::Limited { .. }
        ));
        std::thread::sleep(Duration::from_millis(80));
        assert_eq!(
            limiter.hit(KEY, 1, window),
            LimitOutcome::Allowed,
            "an expired window must start over"
        );
    }

    #[test]
    fn zero_limit_or_zero_window_disables_the_check() {
        let limiter = RateLimiter::new();
        for _ in 0..10 {
            assert_eq!(
                limiter.hit(KEY, 0, Duration::from_secs(60)),
                LimitOutcome::Allowed
            );
            assert_eq!(limiter.hit(KEY, 5, Duration::ZERO), LimitOutcome::Allowed);
        }
    }

    #[test]
    fn prune_drops_expired_entries() {
        let limiter = RateLimiter::new();
        let short = Duration::from_millis(1);
        for i in 0..(PRUNE_THRESHOLD + 10) {
            limiter.hit(&format!("k{i}"), 5, short);
        }
        std::thread::sleep(Duration::from_millis(10));
        limiter.prune_expired();
        assert_eq!(
            limiter.tracked_keys(),
            0,
            "every 1 ms window has expired, nothing should be tracked"
        );
    }
}
