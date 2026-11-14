// SPDX-License-Identifier: GPL-3.0-or-later
//! Retry policy with capped exponential delays.
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RetryPolicy {
    pub max_retries: u32,
    pub base: Duration,
    pub max: Duration,
}

impl RetryPolicy {
    /// Delay before retry number `attempt` (1-based): `base * 2^(attempt-1)`, capped at `max`.
    pub fn delay_for(&self, attempt: u32) -> Duration {
        let shift = attempt.saturating_sub(1).min(16);
        self.base.saturating_mul(1u32 << shift).min(self.max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> RetryPolicy {
        RetryPolicy {
            max_retries: 5,
            base: Duration::from_secs(1),
            max: Duration::from_secs(30),
        }
    }

    #[test]
    fn delay_doubles_per_attempt() {
        let p = policy();
        assert_eq!(p.delay_for(1), Duration::from_secs(1));
        assert_eq!(p.delay_for(2), Duration::from_secs(2));
        assert_eq!(p.delay_for(3), Duration::from_secs(4));
        assert_eq!(p.delay_for(4), Duration::from_secs(8));
    }

    #[test]
    fn delay_is_capped() {
        let p = policy();
        assert_eq!(p.delay_for(5), Duration::from_secs(16));
        assert_eq!(p.delay_for(6), Duration::from_secs(30));
        assert_eq!(p.delay_for(7), Duration::from_secs(30));
    }

    #[test]
    fn large_attempt_does_not_overflow() {
        let p = policy();
        assert_eq!(p.delay_for(u32::MAX), Duration::from_secs(30));
        assert_eq!(p.delay_for(0), Duration::from_secs(1));
    }
}
