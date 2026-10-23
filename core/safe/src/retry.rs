//! Retry with exponential backoff: up to three retries on network errors, timeouts, 429 and
//! 5xx. Anything else fails at once.

use crate::error::SendError;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Retries after the first attempt (default 3, so at most 4 attempts).
    pub max_retries: u32,
    /// Wait before the first retry; doubles each time (default 500 ms).
    pub base_delay: Duration,
    /// Upper bound for the computed wait (default 8 s).
    pub max_delay: Duration,
    /// A `Retry-After` longer than this ends the retries instead of blocking the share
    /// (default 30 s).
    pub max_retry_after: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(8),
            max_retry_after: Duration::from_secs(30),
        }
    }
}

impl RetryPolicy {
    /// Wait before retry number `retry` (1 for the first retry). A `Retry-After` hint wins
    /// when it is longer than the computed wait.
    pub fn delay_before_retry(&self, retry: u32, hint: Option<Duration>) -> Duration {
        let shift = retry.saturating_sub(1).min(20);
        let computed = self
            .base_delay
            .saturating_mul(1u32 << shift)
            .min(self.max_delay);
        match hint {
            Some(h) => h.max(computed),
            None => computed,
        }
    }
}

/// Final result and how many attempts were made.
#[derive(Debug)]
pub struct RetryResult<T> {
    pub result: Result<T, SendError>,
    pub attempts: u32,
}

/// Run `op` until it succeeds, fails for good, runs out of retries or `cancelled` returns true.
///
/// `op` receives the attempt number, starting at 1. `sleep` is injected so tests run
/// instantly and the app can wait on a cancellable timer.
pub fn run_with_retry<T>(
    policy: &RetryPolicy,
    sleep: &mut dyn FnMut(Duration),
    cancelled: &dyn Fn() -> bool,
    op: &mut dyn FnMut(u32) -> Result<T, SendError>,
) -> RetryResult<T> {
    let mut attempts = 0;
    loop {
        attempts += 1;
        match op(attempts) {
            Ok(value) => {
                return RetryResult {
                    result: Ok(value),
                    attempts,
                }
            }
            Err(err) => {
                let retries_used = attempts - 1;
                let too_long = err
                    .retry_after
                    .is_some_and(|after| after > policy.max_retry_after);
                if !err.kind.is_retryable()
                    || retries_used >= policy.max_retries
                    || too_long
                    || cancelled()
                {
                    return RetryResult {
                        result: Err(err),
                        attempts,
                    };
                }
                sleep(policy.delay_before_retry(retries_used + 1, err.retry_after));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::FailureKind;
    use std::cell::{Cell, RefCell};

    fn net() -> SendError {
        SendError::new(FailureKind::NoNetwork, "offline")
    }

    #[test]
    fn delays_double_and_are_capped() {
        let p = RetryPolicy::default();
        assert_eq!(p.delay_before_retry(1, None), Duration::from_millis(500));
        assert_eq!(p.delay_before_retry(2, None), Duration::from_secs(1));
        assert_eq!(p.delay_before_retry(3, None), Duration::from_secs(2));
        assert_eq!(p.delay_before_retry(10, None), Duration::from_secs(8));
        assert_eq!(p.delay_before_retry(100, None), Duration::from_secs(8));
    }

    #[test]
    fn retry_after_wins_when_longer() {
        let p = RetryPolicy::default();
        let d = p.delay_before_retry(1, Some(Duration::from_secs(5)));
        assert_eq!(d, Duration::from_secs(5));
        let d = p.delay_before_retry(3, Some(Duration::from_millis(10)));
        assert_eq!(d, Duration::from_secs(2));
    }

    #[test]
    fn succeeds_after_two_failures() {
        let waits = RefCell::new(Vec::new());
        let r = run_with_retry(
            &RetryPolicy::default(),
            &mut |d| waits.borrow_mut().push(d),
            &|| false,
            &mut |attempt| if attempt < 3 { Err(net()) } else { Ok("sent") },
        );
        assert_eq!(r.result.unwrap(), "sent");
        assert_eq!(r.attempts, 3);
        assert_eq!(
            *waits.borrow(),
            vec![Duration::from_millis(500), Duration::from_secs(1)]
        );
    }

    #[test]
    fn gives_up_after_three_retries() {
        let r: RetryResult<()> =
            run_with_retry(&RetryPolicy::default(), &mut |_| {}, &|| false, &mut |_| {
                Err(SendError::new(FailureKind::Server(503), "down"))
            });
        assert_eq!(r.attempts, 4);
        assert_eq!(r.result.unwrap_err().kind, FailureKind::Server(503));
    }

    #[test]
    fn permanent_failures_do_not_retry() {
        for kind in [
            FailureKind::AuthExpired,
            FailureKind::Rejected(422),
            FailureKind::ConnectorUnavailable,
        ] {
            let r: RetryResult<()> = run_with_retry(
                &RetryPolicy::default(),
                &mut |_| panic!("must not sleep"),
                &|| false,
                &mut |_| Err(SendError::new(kind.clone(), "no")),
            );
            assert_eq!(r.attempts, 1);
        }
    }

    #[test]
    fn long_retry_after_ends_retries() {
        let r: RetryResult<()> = run_with_retry(
            &RetryPolicy::default(),
            &mut |_| panic!("must not sleep"),
            &|| false,
            &mut |_| {
                Err(SendError::new(FailureKind::Throttled(429), "slow down")
                    .with_retry_after(Duration::from_secs(120)))
            },
        );
        assert_eq!(r.attempts, 1);
    }

    #[test]
    fn cancel_stops_before_the_next_wait() {
        let calls = Cell::new(0);
        let r: RetryResult<()> = run_with_retry(
            &RetryPolicy::default(),
            &mut |_| panic!("must not sleep"),
            &|| true,
            &mut |_| {
                calls.set(calls.get() + 1);
                Err(net())
            },
        );
        assert_eq!(calls.get(), 1);
        assert_eq!(r.attempts, 1);
    }

    #[test]
    fn zero_retries_means_one_attempt() {
        let policy = RetryPolicy {
            max_retries: 0,
            ..RetryPolicy::default()
        };
        let r: RetryResult<()> =
            run_with_retry(&policy, &mut |_| {}, &|| false, &mut |_| Err(net()));
        assert_eq!(r.attempts, 1);
    }
}
