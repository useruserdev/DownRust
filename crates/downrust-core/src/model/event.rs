// SPDX-License-Identifier: GPL-3.0-or-later
//! Events the engine sends to the application.
use super::{DownloadId, DownloadRecord, ProgressBatch};

#[derive(Debug, Clone)]
pub enum EngineEvent {
    Added(DownloadRecord),
    Updated(DownloadRecord),
    Removed(DownloadId),
    Progress(ProgressBatch),
    Completed(DownloadRecord),
    /// The queue went from busy to empty after at least one download completed.
    QueueIdle,
}
