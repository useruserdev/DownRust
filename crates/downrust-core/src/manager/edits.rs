// SPDX-License-Identifier: GPL-3.0-or-later
//! Actor handlers that change an existing download: queue order, checksum result and address.
use super::actor::ManagerActor;
use super::queue::queue_moves;
use crate::error::{Error, Result};
use crate::model::{
    ChecksumState, DownloadId, DownloadRecord, DownloadStatus, EngineEvent, ProbeInfo, QueueMove,
};

const DIFFERENT_FILE: &str = "The new address points to a different file.";

/// Statuses in which the address of a download may change.
pub(super) fn address_change_applies(status: DownloadStatus) -> bool {
    matches!(
        status,
        DownloadStatus::Paused | DownloadStatus::Error | DownloadStatus::Queued
    )
}

/// Two sizes count as the same file only when both are known and equal.
pub(super) fn same_file(old: Option<u64>, new: Option<u64>) -> bool {
    matches!((old, new), (Some(a), Some(b)) if a == b)
}

impl ManagerActor {
    /// Moves a queued download. Only queued downloads take part, so the queue order of the
    /// others does not change.
    pub(super) fn move_in_queue(&mut self, id: DownloadId, to: QueueMove) -> Result<()> {
        let record = self.load(id)?;
        if record.status != DownloadStatus::Queued {
            return Err(Error::InvalidInput(
                "Only queued downloads can be moved in the queue.".to_owned(),
            ));
        }
        let queued: Vec<(DownloadId, i64)> = self
            .shared
            .store
            .list_by_status(DownloadStatus::Queued)?
            .iter()
            .map(|r| (r.id, r.queue_position))
            .collect();
        let changes = queue_moves(&queued, id, to);
        if changes.is_empty() {
            return Ok(());
        }
        self.shared.store.set_queue_positions(&changes)?;
        for (changed, _) in changes {
            self.emit_updated(changed)?;
        }
        Ok(())
    }

    /// Stores the expected checksum and the result of a comparison.
    pub(super) fn record_checksum(
        &self,
        id: DownloadId,
        expected: &str,
        state: ChecksumState,
    ) -> Result<()> {
        self.load(id)?;
        self.shared
            .store
            .set_checksum(id, Some(expected), Some(state))?;
        self.emit_updated(id)
    }

    /// Replaces the address of an idle download when the new file has the same size.
    pub(super) fn update_address(
        &mut self,
        id: DownloadId,
        url: &str,
        probe: &ProbeInfo,
    ) -> Result<DownloadRecord> {
        let record = self.load(id)?;
        if self.active.contains_key(&id) || !address_change_applies(record.status) {
            return Err(Error::InvalidInput(
                "The address can only be changed for a paused, failed or queued download."
                    .to_owned(),
            ));
        }
        if !same_file(record.total_size, probe.total_size) {
            return Err(Error::InvalidInput(DIFFERENT_FILE.to_owned()));
        }
        self.shared.store.update_url(id, url, probe)?;
        let updated = self.load(id)?;
        self.emit(EngineEvent::Updated(updated.clone()));
        Ok(updated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_idle_statuses_accept_a_new_address() {
        assert!(address_change_applies(DownloadStatus::Paused));
        assert!(address_change_applies(DownloadStatus::Error));
        assert!(address_change_applies(DownloadStatus::Queued));
        assert!(!address_change_applies(DownloadStatus::Downloading));
        assert!(!address_change_applies(DownloadStatus::Completed));
    }

    #[test]
    fn same_file_needs_two_equal_known_sizes() {
        assert!(same_file(Some(10), Some(10)));
        assert!(!same_file(Some(10), Some(11)));
        assert!(!same_file(None, Some(10)));
        assert!(!same_file(Some(10), None));
        assert!(!same_file(None, None));
    }
}
