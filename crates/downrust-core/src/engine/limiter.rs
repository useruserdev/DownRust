// SPDX-License-Identifier: GPL-3.0-or-later
//! Token bucket that limits how many bytes per second may pass.
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use tokio::time::Instant;

const MIN_CAPACITY: f64 = 16_384.0;

struct Bucket {
    rate: u64,
    tokens: f64,
    last: Instant,
}

/// Token bucket. A rate of 0 means unlimited.
pub(crate) struct RateLimiter {
    state: Mutex<Bucket>,
}

fn capacity(rate: u64) -> f64 {
    (rate as f64 / 4.0).max(MIN_CAPACITY)
}

impl RateLimiter {
    pub fn new(bytes_per_sec: u64) -> Self {
        Self {
            state: Mutex::new(Bucket {
                rate: bytes_per_sec,
                tokens: 0.0,
                last: Instant::now(),
            }),
        }
    }

    pub fn set_rate(&self, bytes_per_sec: u64) {
        let mut b = self.lock();
        b.rate = bytes_per_sec;
        if bytes_per_sec > 0 {
            b.tokens = b.tokens.min(capacity(bytes_per_sec));
        }
    }

    #[cfg(test)]
    pub fn rate(&self) -> u64 {
        self.lock().rate
    }

    /// Waits until `n` bytes may pass. The bucket may go into debt, and the wait pays it back.
    pub async fn acquire(&self, n: u64) {
        let wait = {
            let mut b = self.lock();
            if b.rate == 0 {
                return;
            }
            let now = Instant::now();
            let elapsed = now.saturating_duration_since(b.last).as_secs_f64();
            b.last = now;
            let rate = b.rate as f64;
            b.tokens = (b.tokens + elapsed * rate).min(capacity(b.rate));
            b.tokens -= n as f64;
            if b.tokens < 0.0 {
                Duration::from_secs_f64(-b.tokens / rate)
            } else {
                Duration::ZERO
            }
        };
        if !wait.is_zero() {
            tokio::time::sleep(wait).await;
        }
    }

    fn lock(&self) -> MutexGuard<'_, Bucket> {
        self.state.lock().expect("rate limiter lock poisoned")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn rate_is_respected() {
        let limiter = RateLimiter::new(100_000);
        let start = Instant::now();
        for _ in 0..10 {
            limiter.acquire(50_000).await;
        }
        let elapsed = start.elapsed().as_secs_f64();
        assert!((4.0..=5.5).contains(&elapsed), "elapsed {elapsed}");
    }

    #[tokio::test(start_paused = true)]
    async fn zero_rate_is_unlimited() {
        let limiter = RateLimiter::new(0);
        let start = Instant::now();
        limiter.acquire(u64::MAX).await;
        assert_eq!(start.elapsed(), Duration::ZERO);
    }

    #[test]
    fn set_rate_clamps_tokens_to_new_capacity() {
        let limiter = RateLimiter::new(100_000);
        limiter.lock().tokens = 1_000_000.0;
        limiter.set_rate(40_000);
        assert_eq!(limiter.rate(), 40_000);
        assert_eq!(limiter.lock().tokens, 16_384.0);
    }
}
