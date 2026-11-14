// SPDX-License-Identifier: GPL-3.0-or-later
//! Coordinator of one transfer. Starts and stops segment workers, saves checkpoints and reports
//! how the transfer ended.
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinSet;
use tokio::time::{interval_at, Instant, MissedTickBehavior};
use tokio_util::sync::CancellationToken;

use super::retry::RetryPolicy;
use super::segments::{SegmentSlot, SegmentTable, UNKNOWN_END};
use super::task::TaskContext;
use super::worker::{run_worker, AddressPool, WorkerContext, WorkerExit};
use super::writer::WriteCmd;
use crate::error::{Error, Result};
use crate::model::{HeaderPair, SegmentRow};

const CONTROL_EVERY: Duration = Duration::from_millis(250);
const RESOURCE_CHANGED: &str = "the file on the server changed";

/// Settings of the transfer that the workers share.
pub(crate) struct Job {
    /// The main address and the verified mirrors.
    pub addresses: Arc<AddressPool>,
    pub headers: Vec<HeaderPair>,
    /// Error text used when the server answers `401 Unauthorized`.
    pub unauthorized: String,
    pub segmented: bool,
    pub use_range: bool,
    pub if_range: Option<String>,
    pub min_split: u64,
    pub retry: RetryPolicy,
    pub read_timeout: Duration,
    pub table: Arc<SegmentTable>,
    pub write_tx: mpsc::Sender<WriteCmd>,
}

/// How the coordinator ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CoordEnd {
    Finished,
    Cancelled,
    Failed(String),
    Restart(String),
}

struct Running {
    slot: usize,
    token: CancellationToken,
    stopping: bool,
}

#[derive(Default)]
struct Control {
    running: Vec<Running>,
    failure: Option<String>,
    restart: Option<String>,
    cancel_seen: bool,
}

impl Control {
    fn active(&self) -> usize {
        self.running.iter().filter(|r| !r.stopping).count()
    }

    fn halted(&self, ctx: &TaskContext) -> bool {
        self.failure.is_some()
            || self.restart.is_some()
            || self.cancel_seen
            || ctx.cancel.is_cancelled()
    }

    fn cancel_all(&self) {
        for running in &self.running {
            running.token.cancel();
        }
    }

    /// Stops the newest workers until the active count matches `target`.
    fn trim(&mut self, target: usize) {
        let mut excess = self.active().saturating_sub(target);
        for running in self.running.iter_mut().rev() {
            if excess == 0 {
                break;
            }
            if !running.stopping {
                running.stopping = true;
                running.token.cancel();
                excess -= 1;
            }
        }
    }

    fn on_exit(&mut self, ctx: &TaskContext, table: &SegmentTable, slot: usize, exit: WorkerExit) {
        self.running.retain(|r| r.slot != slot);
        table.release(slot);
        match exit {
            WorkerExit::Finished | WorkerExit::Cancelled => {}
            WorkerExit::ServerBusy => ctx.live.reduce_target(),
            WorkerExit::ResourceChanged => {
                if self.restart.is_none() {
                    self.restart = Some(RESOURCE_CHANGED.to_owned());
                }
                self.cancel_all();
            }
            WorkerExit::Failed(msg) => {
                if self.failure.is_none() {
                    self.failure = Some(msg);
                }
                self.cancel_all();
            }
        }
    }

    fn end(self, ctx: &TaskContext, table: &SegmentTable) -> CoordEnd {
        if let Some(msg) = self.failure {
            CoordEnd::Failed(msg)
        } else if let Some(msg) = self.restart {
            CoordEnd::Restart(msg)
        } else if self.cancel_seen || ctx.cancel.is_cancelled() {
            CoordEnd::Cancelled
        } else if table.all_fetched() {
            CoordEnd::Finished
        } else {
            CoordEnd::Failed("no segment could be scheduled".to_owned())
        }
    }
}

/// Runs the workers of one attempt until none is left.
pub(crate) async fn run_coordinator(ctx: &TaskContext, job: Job) -> CoordEnd {
    let mut workers: JoinSet<(usize, WorkerExit)> = JoinSet::new();
    let mut control = Control::default();
    let period = Duration::from_millis(ctx.settings.checkpoint_interval_ms);
    let mut checkpoints = interval_at(Instant::now() + period, period);
    checkpoints.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut control_tick = interval_at(Instant::now() + CONTROL_EVERY, CONTROL_EVERY);
    control_tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        if !control.halted(ctx) {
            let target = target_workers(ctx, &job);
            while control.active() < target {
                let Some(slot) = job.table.assign_next(job.min_split) else {
                    break;
                };
                let token = ctx.cancel.child_token();
                let worker = worker_context(ctx, &job, slot, token.clone());
                workers.spawn(run_worker(worker));
                control.running.push(Running {
                    slot,
                    token,
                    stopping: false,
                });
            }
        }
        ctx.live.set_connections(control.running.len() as u32);
        if workers.is_empty() {
            return control.end(ctx, &job.table);
        }
        tokio::select! {
            joined = workers.join_next() => match joined {
                Some(Ok((slot, exit))) => control.on_exit(ctx, &job.table, slot, exit),
                Some(Err(err)) => {
                    if control.failure.is_none() {
                        control.failure = Some(format!("download worker stopped: {err}"));
                    }
                    control.cancel_all();
                }
                None => {}
            },
            _ = checkpoints.tick(), if job.segmented => {
                if let Err(err) = checkpoint(ctx, &job.write_tx).await {
                    tracing::warn!(id = ctx.id, error = %err, "checkpoint failed");
                }
            }
            _ = control_tick.tick() => control.trim(target_workers(ctx, &job)),
            _ = ctx.cancel.cancelled(), if !control.cancel_seen => control.cancel_seen = true,
        }
    }
}

fn target_workers(ctx: &TaskContext, job: &Job) -> usize {
    if job.segmented {
        ctx.live.target().max(1) as usize
    } else {
        1
    }
}

fn worker_context(
    ctx: &TaskContext,
    job: &Job,
    slot: usize,
    cancel: CancellationToken,
) -> WorkerContext {
    WorkerContext {
        slot,
        client: ctx.client.clone(),
        addresses: Arc::clone(&job.addresses),
        headers: job.headers.clone(),
        unauthorized: job.unauthorized.clone(),
        use_range: job.use_range,
        if_range: job.if_range.clone(),
        table: Arc::clone(&job.table),
        write_tx: job.write_tx.clone(),
        global_limiter: Arc::clone(&ctx.global_limiter),
        limiter: Arc::clone(&ctx.limiter),
        live: Arc::clone(&ctx.live),
        cancel,
        retry: job.retry,
        read_timeout: job.read_timeout,
    }
}

/// Asks the writer for a synced snapshot and saves it as the resume point.
pub(crate) async fn checkpoint(ctx: &TaskContext, write_tx: &mpsc::Sender<WriteCmd>) -> Result<()> {
    let (reply, answer) = oneshot::channel();
    write_tx
        .send(WriteCmd::Checkpoint { reply })
        .await
        .map_err(|_| Error::Transfer("disk writer stopped".to_owned()))?;
    let slots = answer
        .await
        .map_err(|_| Error::Transfer("disk writer stopped".to_owned()))?;
    let Some((rows, written)) = checkpoint_rows(&slots) else {
        return Ok(());
    };
    ctx.store.save_checkpoint(ctx.id, written, &rows)
}

/// Turns a snapshot into saved rows and the total written. A segment with unknown end gives
/// `None`, because such a transfer cannot resume.
pub(crate) fn checkpoint_rows(slots: &[SegmentSlot]) -> Option<(Vec<SegmentRow>, u64)> {
    if slots.iter().any(|s| s.end == UNKNOWN_END) {
        return None;
    }
    let rows: Vec<SegmentRow> = slots
        .iter()
        .map(|s| SegmentRow {
            start: s.start,
            end: s.end,
            written: s.written,
        })
        .collect();
    let total = rows
        .iter()
        .fold(0u64, |sum, r| sum.saturating_add(r.written));
    Some((rows, total))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(start: u64, end: u64, written: u64) -> SegmentSlot {
        SegmentSlot {
            start,
            end,
            fetched: written,
            written,
            active: false,
        }
    }

    #[test]
    fn checkpoint_rows_sum_written_bytes() {
        let slots = [slot(0, 100, 40), slot(100, 200, 0), slot(200, 300, 60)];
        let (rows, total) = checkpoint_rows(&slots).expect("known ends");
        assert_eq!(total, 100);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[2].start, 200);
        assert_eq!(rows[2].end, 300);
        assert_eq!(rows[2].written, 60);
    }

    #[test]
    fn unknown_end_gives_no_checkpoint() {
        let slots = [slot(0, UNKNOWN_END, 5)];
        assert!(checkpoint_rows(&slots).is_none());
    }

    #[test]
    fn trim_stops_newest_active_workers_only() {
        let mut control = Control::default();
        for slot in 0..3 {
            control.running.push(Running {
                slot,
                token: CancellationToken::new(),
                stopping: false,
            });
        }
        control.trim(1);
        assert!(!control.running[0].token.is_cancelled());
        assert!(control.running[1].token.is_cancelled());
        assert!(control.running[2].token.is_cancelled());
        assert_eq!(control.active(), 1);
        control.trim(1);
        assert_eq!(control.active(), 1);
    }
}
