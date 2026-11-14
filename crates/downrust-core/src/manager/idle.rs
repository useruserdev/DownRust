// SPDX-License-Identifier: GPL-3.0-or-later
//! Detects the moment the queue empties after downloads completed.
use super::actor::ManagerActor;
use crate::model::{DownloadStatus, EngineEvent};

impl ManagerActor {
    /// Sends `QueueIdle` once when the queue goes from busy to empty after a completion.
    /// Busy means something runs or waits in the queue. Paused and failed downloads do not count.
    pub(super) fn check_queue_idle(&mut self) {
        if !self.active.is_empty() || self.has_queued() {
            self.queue_busy = true;
            return;
        }
        if self.queue_busy {
            self.queue_busy = false;
            if std::mem::take(&mut self.completed_in_run) {
                self.emit(EngineEvent::QueueIdle);
            }
        }
    }

    fn has_queued(&self) -> bool {
        match self.shared.store.list_by_status(DownloadStatus::Queued) {
            Ok(rows) => !rows.is_empty(),
            Err(err) => {
                tracing::error!("cannot read the queue: {err}");
                false
            }
        }
    }
}
