// SPDX-License-Identifier: GPL-3.0-or-later
//! Queue rules, plus the code that starts, ends, pauses and resumes transfers.
use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use super::active::ActiveTask;
use super::actor::ManagerActor;
use super::command::Cmd;
use crate::engine::limiter::RateLimiter;
use crate::engine::live::LiveStats;
use crate::engine::media::run_download;
use crate::engine::scan::scan_completed;
use crate::engine::task::{TaskContext, TaskOutcome};
use crate::error::Result;
use crate::model::{DownloadId, DownloadRecord, DownloadStatus, EngineEvent};
use crate::settings::{is_open_now, MAX_CONNECTIONS};
use crate::util::time::{local_today, now_ms};

/// Number of transfers that may still start.
pub(super) fn free_slots(running: usize, max: u32) -> usize {
    usize::try_from(max)
        .unwrap_or(usize::MAX)
        .saturating_sub(running)
}

/// The queued downloads to start now, in queue order (queue position, then id).
pub(super) fn pick_next(mut queued: Vec<DownloadRecord>, free: usize) -> Vec<DownloadRecord> {
    queued.sort_by_key(|record| (record.queue_position, record.id));
    queued.truncate(free);
    queued
}

/// Pause moves only a queued download. A running one is cancelled and pauses when it stops.
pub(super) fn pause_applies(status: DownloadStatus) -> bool {
    status == DownloadStatus::Queued
}

/// Resume queues every download except a completed one.
pub(super) fn resume_applies(status: DownloadStatus) -> bool {
    status != DownloadStatus::Completed
}

/// Resume all queues only paused and failed downloads.
pub(super) fn resume_all_applies(status: DownloadStatus) -> bool {
    matches!(status, DownloadStatus::Paused | DownloadStatus::Error)
}

/// Status of a cancelled transfer. It is queued again only when the app
/// shuts down with resume-on-startup enabled.
pub(super) fn cancelled_status(shutting_down: bool, resume_on_startup: bool) -> DownloadStatus {
    if shutting_down && resume_on_startup {
        DownloadStatus::Queued
    } else {
        DownloadStatus::Paused
    }
}

impl ManagerActor {
    /// Starts queued downloads until the concurrency limit is reached.
    pub(super) fn schedule(&mut self) {
        if self.shutting_down {
            return;
        }
        let settings = self.shared.settings();
        if !is_open_now(&settings.schedule) {
            self.stop_for_schedule();
            return;
        }
        let free = free_slots(self.active.len(), settings.max_concurrent_downloads);
        if free == 0 {
            return;
        }
        let queued = match self.shared.store.list_by_status(DownloadStatus::Queued) {
            Ok(rows) => rows,
            Err(err) => {
                tracing::error!("cannot read the queue: {err}");
                return;
            }
        };
        let queued: Vec<DownloadRecord> = queued
            .into_iter()
            .filter(|record| !self.active.contains_key(&record.id))
            .collect();
        for record in pick_next(queued, free) {
            if let Err(err) = self.spawn_task(record) {
                tracing::error!("cannot start a download: {err}");
            }
        }
    }

    /// Cancels every running transfer while the time window is closed. Each one returns to the
    /// queue with its progress kept.
    fn stop_for_schedule(&mut self) {
        for (id, task) in &self.active {
            if self.window_stop.insert(*id) {
                task.cancel.cancel();
            }
        }
    }

    /// Marks a download as downloading and runs its task on the runtime.
    fn spawn_task(&mut self, record: DownloadRecord) -> Result<()> {
        let id = record.id;
        self.set_status(id, DownloadStatus::Downloading, None)?;
        let live = Arc::new(LiveStats::new(u32::from(
            record.connections.clamp(1, MAX_CONNECTIONS),
        )));
        let limiter = Arc::new(RateLimiter::new(record.speed_limit.unwrap_or(0)));
        let cancel = CancellationToken::new();
        let settings = self.shared.settings();
        let scan = settings.scan.clone();
        let ctx = TaskContext {
            id,
            store: self.shared.store.clone(),
            client: self.shared.client(),
            settings,
            global_limiter: Arc::clone(&self.global_limiter),
            limiter: Arc::clone(&limiter),
            live: Arc::clone(&live),
            cancel: cancel.clone(),
            events: self.events.clone(),
        };
        self.active
            .insert(id, ActiveTask::new(cancel, live, limiter));
        let self_tx = self.self_tx.clone();
        let scan_store = self.shared.store.clone();
        let scan_events = self.events.clone();
        tokio::spawn(async move {
            // The inner spawn turns a panic in the task into a failure.
            let outcome = match tokio::spawn(run_download(ctx)).await {
                Ok(TaskOutcome::Completed) => {
                    scan_completed(&scan_store, &scan_events, id, &scan).await
                }
                Ok(outcome) => outcome,
                Err(err) => TaskOutcome::Failed(format!("internal error: {err}")),
            };
            let _ = self_tx.send(Cmd::TaskFinished { id, outcome });
        });
        Ok(())
    }

    /// Applies the end of a transfer: a pending remove or restart first, otherwise the outcome.
    pub(super) fn finish_task(&mut self, id: DownloadId, outcome: TaskOutcome) -> Result<()> {
        self.active.remove(&id);
        let stopped_by_window = self.window_stop.remove(&id);
        let removal = self.removing.remove(&id);
        let restart = self.restarting.remove(&id);
        if let Some(delete_file) = removal {
            if let Some(record) = self.shared.store.get_download(id)? {
                let finished = outcome == TaskOutcome::Completed;
                self.remove_record(&record, delete_file, finished)?;
            }
        } else if restart {
            self.restart_record(id)?;
        } else {
            match outcome {
                TaskOutcome::Completed => {
                    self.shared.store.mark_completed(id, now_ms())?;
                    let record = self.load(id)?;
                    self.record_completion_stats(&record);
                    self.completed_in_run = true;
                    self.emit(EngineEvent::Updated(record.clone()));
                    self.emit(EngineEvent::Completed(record));
                }
                TaskOutcome::Cancelled => {
                    let resume_on_startup = self.shared.settings().resume_on_startup;
                    let status = if stopped_by_window {
                        DownloadStatus::Queued
                    } else {
                        cancelled_status(self.shutting_down, resume_on_startup)
                    };
                    self.set_status(id, status, None)?;
                }
                TaskOutcome::Failed(message) => {
                    self.set_status(id, DownloadStatus::Error, Some(message.as_str()))?;
                }
            }
        }
        self.schedule();
        Ok(())
    }

    /// Adds a finished file to today's statistics. A failure is logged and does not stop the download.
    fn record_completion_stats(&self, record: &DownloadRecord) {
        let result = self.shared.store.add_completion_stats(
            &local_today(),
            record.category,
            record.downloaded,
        );
        if let Err(err) = result {
            tracing::warn!(
                "cannot record the statistics of download {}: {err}",
                record.id
            );
        }
    }

    pub(super) fn pause(&mut self, id: DownloadId) -> Result<()> {
        let record = self.load(id)?;
        if self.active.contains_key(&id) {
            self.cancel_task(id);
            return Ok(());
        }
        if pause_applies(record.status) {
            self.set_status(id, DownloadStatus::Paused, None)?;
        }
        Ok(())
    }

    pub(super) fn resume(&mut self, id: DownloadId) -> Result<()> {
        let record = self.load(id)?;
        if self.active.contains_key(&id) || !resume_applies(record.status) {
            return Ok(());
        }
        self.set_status(id, DownloadStatus::Queued, None)?;
        self.schedule();
        Ok(())
    }

    pub(super) fn pause_all(&mut self) -> Result<()> {
        for task in self.active.values() {
            task.cancel.cancel();
        }
        for record in self.shared.store.list_by_status(DownloadStatus::Queued)? {
            self.set_status(record.id, DownloadStatus::Paused, None)?;
        }
        Ok(())
    }

    pub(super) fn resume_all(&mut self) -> Result<()> {
        for record in self.shared.store.list_downloads()? {
            if resume_all_applies(record.status) && !self.active.contains_key(&record.id) {
                self.set_status(record.id, DownloadStatus::Queued, None)?;
            }
        }
        self.schedule();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Category;

    fn record(id: DownloadId, queue_position: i64) -> DownloadRecord {
        DownloadRecord {
            id,
            url: String::new(),
            final_url: None,
            referrer: None,
            file_name: format!("file-{id}"),
            save_dir: String::new(),
            category: Category::Other,
            status: DownloadStatus::Queued,
            total_size: None,
            downloaded: 0,
            resumable: false,
            connections: 1,
            speed_limit: None,
            etag: None,
            last_modified: None,
            content_type: None,
            error: None,
            queue_position,
            headers: Vec::new(),
            created_at: 0,
            completed_at: None,
            expected_sha256: None,
            checksum: None,
            scan: None,
            kind: crate::model::DownloadKind::File,
        }
    }

    #[test]
    fn free_slots_counts_down_to_zero() {
        assert_eq!(free_slots(0, 3), 3);
        assert_eq!(free_slots(2, 3), 1);
        assert_eq!(free_slots(5, 3), 0);
        assert_eq!(free_slots(0, 0), 0);
    }

    #[test]
    fn pick_next_respects_the_limit_and_queue_order() {
        let queued = vec![record(3, 30), record(1, 10), record(2, 10)];
        let picked: Vec<DownloadId> = pick_next(queued.clone(), 2).iter().map(|r| r.id).collect();
        assert_eq!(picked, vec![1, 2]);
        assert!(pick_next(queued.clone(), 0).is_empty());
        let all: Vec<DownloadId> = pick_next(queued, 10).iter().map(|r| r.id).collect();
        assert_eq!(all, vec![1, 2, 3]);
    }

    #[test]
    fn pause_applies_only_to_queued() {
        assert!(pause_applies(DownloadStatus::Queued));
        assert!(!pause_applies(DownloadStatus::Downloading));
        assert!(!pause_applies(DownloadStatus::Paused));
        assert!(!pause_applies(DownloadStatus::Completed));
    }

    #[test]
    fn resume_applies_to_all_but_completed() {
        assert!(resume_applies(DownloadStatus::Paused));
        assert!(resume_applies(DownloadStatus::Error));
        assert!(resume_applies(DownloadStatus::Queued));
        assert!(resume_applies(DownloadStatus::Downloading));
        assert!(!resume_applies(DownloadStatus::Completed));
    }

    #[test]
    fn resume_all_applies_to_paused_and_failed_only() {
        assert!(resume_all_applies(DownloadStatus::Paused));
        assert!(resume_all_applies(DownloadStatus::Error));
        assert!(!resume_all_applies(DownloadStatus::Queued));
        assert!(!resume_all_applies(DownloadStatus::Downloading));
        assert!(!resume_all_applies(DownloadStatus::Completed));
    }

    #[test]
    fn cancelled_transfer_is_queued_only_on_shutdown_with_resume() {
        assert_eq!(cancelled_status(true, true), DownloadStatus::Queued);
        assert_eq!(cancelled_status(true, false), DownloadStatus::Paused);
        assert_eq!(cancelled_status(false, true), DownloadStatus::Paused);
        assert_eq!(cancelled_status(false, false), DownloadStatus::Paused);
    }
}
