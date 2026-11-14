// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared data types of the download engine.
mod category;
mod checksum;
mod event;
mod kind;
mod probe;
mod progress;
mod queue;
mod record;
mod request;
mod scan;
mod segment;
mod stats;
mod status;

pub use category::Category;
pub use checksum::{ChecksumReport, ChecksumState};
pub use event::EngineEvent;
pub use kind::DownloadKind;
pub use probe::ProbeInfo;
pub use progress::{ProgressBatch, ProgressItem, SegmentView};
pub use queue::QueueMove;
pub use record::{DownloadId, DownloadRecord, HeaderPair, NewDownload, PART_EXTENSION};
pub use request::{AddRequest, TargetSuggestion};
pub use scan::ScanState;
pub use segment::SegmentRow;
pub use stats::{StatsCategory, StatsDay, StatsSnapshot, StatsTotals};
pub use status::{DownloadStatus, Phase};
