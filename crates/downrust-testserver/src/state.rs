// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared server state: content version, failure injection, concurrency limit and counters.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Snapshot of the server counters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stats {
    /// Requests received on `/file/{size}`, including rejected ones.
    pub requests: u64,
    /// Requests answered with `206 Partial Content`.
    pub range_requests: u64,
    /// Body bytes handed to the network layer.
    pub bytes_sent: u64,
    /// File requests whose body is still being sent.
    pub active: u64,
    /// Highest value `active` has reached.
    pub peak_active: u64,
    /// File requests whose address is absolute, as a proxy receives them.
    pub proxied_requests: u64,
}

/// State shared by all requests. Every field is an atomic, so the state is read and changed without locks.
#[derive(Debug)]
pub struct ServerState {
    version: AtomicU64,
    fail_requests: AtomicU64,
    fail_after: AtomicU64,
    max_concurrent: AtomicU64,
    requests: AtomicU64,
    range_requests: AtomicU64,
    bytes_sent: AtomicU64,
    active: AtomicU64,
    peak_active: AtomicU64,
    proxied_requests: AtomicU64,
    hls_segments: AtomicU64,
    hls_delay_ms: AtomicU64,
}

impl Default for ServerState {
    fn default() -> Self {
        Self {
            version: AtomicU64::new(1),
            fail_requests: AtomicU64::new(0),
            fail_after: AtomicU64::new(0),
            max_concurrent: AtomicU64::new(0),
            requests: AtomicU64::new(0),
            range_requests: AtomicU64::new(0),
            bytes_sent: AtomicU64::new(0),
            active: AtomicU64::new(0),
            peak_active: AtomicU64::new(0),
            proxied_requests: AtomicU64::new(0),
            hls_segments: AtomicU64::new(0),
            hls_delay_ms: AtomicU64::new(0),
        }
    }
}

impl ServerState {
    /// Segment requests received on the HLS fixtures so far.
    pub fn hls_segment_requests(&self) -> u64 {
        self.hls_segments.load(Ordering::SeqCst)
    }

    /// Delay before each HLS segment is sent. Zero by default.
    pub fn set_hls_delay_ms(&self, ms: u64) {
        self.hls_delay_ms.store(ms, Ordering::SeqCst);
    }

    pub(crate) fn count_hls_segment(&self) -> u64 {
        self.hls_segments.fetch_add(1, Ordering::SeqCst);
        self.hls_delay_ms.load(Ordering::SeqCst)
    }

    /// Current content version. Starts at 1.
    pub fn version(&self) -> u64 {
        self.version.load(Ordering::SeqCst)
    }

    /// Changes the content version. The content, ETag and Last-Modified change with it.
    pub fn set_version(&self, version: u64) {
        self.version.store(version, Ordering::SeqCst);
    }

    /// Makes the next `count` file responses stop after `after_bytes` body bytes.
    /// The stream then yields an I/O error, so the client sees a cut transfer.
    pub fn fail_next(&self, count: u64, after_bytes: u64) {
        self.fail_after.store(after_bytes, Ordering::SeqCst);
        self.fail_requests.store(count, Ordering::SeqCst);
    }

    /// Limits concurrent file bodies. Requests above the limit get `503`. `0` means no limit.
    pub fn set_max_concurrent(&self, limit: u64) {
        self.max_concurrent.store(limit, Ordering::SeqCst);
    }

    /// Returns a snapshot of all counters.
    pub fn stats(&self) -> Stats {
        Stats {
            requests: self.requests.load(Ordering::SeqCst),
            range_requests: self.range_requests.load(Ordering::SeqCst),
            bytes_sent: self.bytes_sent.load(Ordering::SeqCst),
            active: self.active.load(Ordering::SeqCst),
            peak_active: self.peak_active.load(Ordering::SeqCst),
            proxied_requests: self.proxied_requests.load(Ordering::SeqCst),
        }
    }

    pub(crate) fn count_request(&self) {
        self.requests.fetch_add(1, Ordering::SeqCst);
    }

    pub(crate) fn count_proxied(&self) {
        self.proxied_requests.fetch_add(1, Ordering::SeqCst);
    }

    pub(crate) fn count_range(&self) {
        self.range_requests.fetch_add(1, Ordering::SeqCst);
    }

    pub(crate) fn add_bytes(&self, n: u64) {
        self.bytes_sent.fetch_add(n, Ordering::SeqCst);
    }

    /// Consumes one pending failure. Returns the byte count at which this response must fail.
    pub(crate) fn take_failure(&self) -> Option<u64> {
        let mut remaining = self.fail_requests.load(Ordering::SeqCst);
        loop {
            if remaining == 0 {
                return None;
            }
            match self.fail_requests.compare_exchange(
                remaining,
                remaining - 1,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => return Some(self.fail_after.load(Ordering::SeqCst)),
                Err(actual) => remaining = actual,
            }
        }
    }

    /// Takes one slot of the concurrency limit. Returns `None` when the limit is reached.
    pub(crate) fn try_acquire(self: &Arc<Self>) -> Option<ActiveGuard> {
        let limit = self.max_concurrent.load(Ordering::SeqCst);
        let mut current = self.active.load(Ordering::SeqCst);
        loop {
            if limit > 0 && current >= limit {
                return None;
            }
            match self.active.compare_exchange(
                current,
                current + 1,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => break,
                Err(actual) => current = actual,
            }
        }
        self.peak_active.fetch_max(current + 1, Ordering::SeqCst);
        Some(ActiveGuard {
            state: Arc::clone(self),
        })
    }
}

/// Holds one active slot. Dropping it releases the slot.
#[derive(Debug)]
pub(crate) struct ActiveGuard {
    state: Arc<ServerState>,
}

impl Drop for ActiveGuard {
    fn drop(&mut self) {
        self.state.active.fetch_sub(1, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limit_rejects_above_max_and_guard_releases() {
        let state = Arc::new(ServerState::default());
        state.set_max_concurrent(2);
        let a = state.try_acquire().expect("first slot");
        let b = state.try_acquire().expect("second slot");
        assert!(state.try_acquire().is_none());
        drop(a);
        let c = state.try_acquire();
        assert!(c.is_some());
        drop(b);
        drop(c);
        let stats = state.stats();
        assert_eq!(stats.active, 0);
        assert_eq!(stats.peak_active, 2);
    }

    #[test]
    fn failures_are_consumed_one_per_response() {
        let state = ServerState::default();
        state.fail_next(2, 4096);
        assert_eq!(state.take_failure(), Some(4096));
        assert_eq!(state.take_failure(), Some(4096));
        assert_eq!(state.take_failure(), None);
    }
}
