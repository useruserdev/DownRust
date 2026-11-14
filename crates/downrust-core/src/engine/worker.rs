// SPDX-License-Identifier: GPL-3.0-or-later
//! Segment worker. Fetches one segment and retries transient failures with backoff.
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::limiter::RateLimiter;
use super::live::LiveStats;
use super::retry::RetryPolicy;
use super::segments::{SegmentTable, UNKNOWN_END};
use super::worker_stream::{fetch_once, Attempt};
use super::writer::WriteCmd;
use crate::model::HeaderPair;

pub(crate) const FLUSH_BYTES: usize = 512 * 1024;
pub(crate) const FLUSH_EVERY: Duration = Duration::from_secs(1);
/// A mirror that fails this many times in a row is not used again in the attempt.
pub(crate) const MIRROR_DROP_AFTER: u32 = 3;

/// The addresses of one transfer. Index 0 is the main address; the others are verified mirrors.
/// Connection `slot` uses address `slot % len`.
pub(crate) struct AddressPool {
    addresses: Vec<String>,
    /// Consecutive failures per address, same order as `addresses`.
    failures: Mutex<Vec<u32>>,
}

impl AddressPool {
    pub(crate) fn new(main: String, mirrors: Vec<String>) -> Self {
        let mut addresses = vec![main];
        addresses.extend(mirrors);
        let failures = vec![0; addresses.len()];
        Self {
            addresses,
            failures: Mutex::new(failures),
        }
    }

    /// The index and address for a connection. A dropped mirror gives the main address.
    pub(crate) fn for_slot(&self, slot: usize) -> (usize, &str) {
        let index = slot % self.addresses.len();
        let failures = self.failures.lock().unwrap_or_else(PoisonError::into_inner);
        if index != 0 && failures[index] >= MIRROR_DROP_AFTER {
            return (0, &self.addresses[0]);
        }
        (index, &self.addresses[index])
    }

    /// Records whether a request to the address at `index` worked. The main address is not counted.
    pub(crate) fn report(&self, index: usize, ok: bool) {
        if index == 0 {
            return;
        }
        let mut failures = self.failures.lock().unwrap_or_else(PoisonError::into_inner);
        failures[index] = if ok {
            0
        } else {
            failures[index].saturating_add(1)
        };
    }
}

/// Everything one worker needs. The coordinator builds it for each segment it starts.
pub(crate) struct WorkerContext {
    pub slot: usize,
    pub client: reqwest::Client,
    pub addresses: Arc<AddressPool>,
    pub headers: Vec<HeaderPair>,
    pub unauthorized: String,
    pub use_range: bool,
    pub if_range: Option<String>,
    pub table: Arc<SegmentTable>,
    pub write_tx: mpsc::Sender<WriteCmd>,
    pub global_limiter: Arc<RateLimiter>,
    pub limiter: Arc<RateLimiter>,
    pub live: Arc<LiveStats>,
    pub cancel: CancellationToken,
    pub retry: RetryPolicy,
    pub read_timeout: Duration,
}

/// Why a worker stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WorkerExit {
    Finished,
    Cancelled,
    ServerBusy,
    ResourceChanged,
    Failed(String),
}

/// Runs one worker and returns its slot with the exit reason.
pub(crate) async fn run_worker(ctx: WorkerContext) -> (usize, WorkerExit) {
    let slot = ctx.slot;
    (slot, worker_loop(&ctx).await)
}

async fn worker_loop(ctx: &WorkerContext) -> WorkerExit {
    let mut attempt: u32 = 0;
    loop {
        if ctx.cancel.is_cancelled() {
            return WorkerExit::Cancelled;
        }
        let (pos, end) = ctx.table.position(ctx.slot);
        if end != UNKNOWN_END && pos >= end {
            return WorkerExit::Finished;
        }
        let (index, url) = ctx.addresses.for_slot(ctx.slot);
        let result = fetch_once(ctx, url, pos, end).await;
        let result = if index == 0 {
            result
        } else {
            mirror_outcome(ctx, index, result)
        };
        let (msg, progressed) = match result {
            Attempt::Done => return WorkerExit::Finished,
            Attempt::Cancelled => return WorkerExit::Cancelled,
            Attempt::ResourceChanged => return WorkerExit::ResourceChanged,
            Attempt::Fatal(msg) => return WorkerExit::Failed(msg),
            Attempt::Busy(_) if ctx.live.connections() > 1 => return WorkerExit::ServerBusy,
            Attempt::Busy(msg) => (msg, false),
            Attempt::Retry { msg, progressed } => (msg, progressed),
        };
        if progressed {
            attempt = 0;
        }
        attempt += 1;
        if attempt > ctx.retry.max_retries {
            return WorkerExit::Failed(msg);
        }
        if !ctx.use_range {
            ctx.table.reset_fetched(ctx.slot);
        }
        tokio::select! {
            biased;
            _ = ctx.cancel.cancelled() => return WorkerExit::Cancelled,
            _ = tokio::time::sleep(ctx.retry.delay_for(attempt)) => {}
        }
    }
}

/// Records the result of a mirror request. Any failure of a mirror becomes a retryable failure,
/// so the connection tries again and the mirror is dropped after repeated failures.
fn mirror_outcome(ctx: &WorkerContext, index: usize, result: Attempt) -> Attempt {
    match result {
        Attempt::Done => {
            ctx.addresses.report(index, true);
            Attempt::Done
        }
        Attempt::Cancelled => Attempt::Cancelled,
        Attempt::Retry { msg, progressed } => {
            ctx.addresses.report(index, progressed);
            Attempt::Retry { msg, progressed }
        }
        _ => {
            ctx.addresses.report(index, false);
            Attempt::Retry {
                msg: "a mirror address failed".to_owned(),
                progressed: false,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connections_take_addresses_in_turn() {
        let pool = AddressPool::new("main".into(), vec!["m1".into(), "m2".into()]);
        assert_eq!(pool.for_slot(0), (0, "main"));
        assert_eq!(pool.for_slot(1), (1, "m1"));
        assert_eq!(pool.for_slot(2), (2, "m2"));
        assert_eq!(pool.for_slot(3), (0, "main"));
    }

    #[test]
    fn mirror_is_dropped_after_three_failures_in_a_row() {
        let pool = AddressPool::new("main".into(), vec!["m1".into()]);
        pool.report(1, false);
        pool.report(1, false);
        assert_eq!(pool.for_slot(1), (1, "m1"));
        pool.report(1, true);
        pool.report(1, false);
        pool.report(1, false);
        assert_eq!(pool.for_slot(1), (1, "m1"));
        pool.report(1, false);
        assert_eq!(pool.for_slot(1), (0, "main"));
    }

    #[test]
    fn main_address_is_never_dropped() {
        let pool = AddressPool::new("main".into(), Vec::new());
        for _ in 0..10 {
            pool.report(0, false);
        }
        assert_eq!(pool.for_slot(0), (0, "main"));
    }
}
