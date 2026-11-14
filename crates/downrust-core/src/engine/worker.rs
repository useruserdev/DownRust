// SPDX-License-Identifier: GPL-3.0-or-later
//! Segment worker. Fetches one segment and retries transient failures with backoff.
use std::sync::Arc;
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

/// Everything one worker needs. The coordinator builds it for each segment it starts.
pub(crate) struct WorkerContext {
    pub slot: usize,
    pub client: reqwest::Client,
    pub url: String,
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
        let (msg, progressed) = match fetch_once(ctx, pos, end).await {
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
