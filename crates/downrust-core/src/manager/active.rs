// SPDX-License-Identifier: GPL-3.0-or-later
//! Bookkeeping for transfers that run right now, and the periodic progress batch.
use std::collections::HashMap;
use std::sync::Arc;

use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use super::actor::ManagerActor;
use crate::engine::limiter::RateLimiter;
use crate::engine::live::LiveStats;
use crate::engine::speed::{eta_secs, SpeedMeter};
use crate::model::{DownloadId, EngineEvent, ProgressBatch, ProgressItem};

/// One running transfer: its cancel token, live counters, speed limiter and speed meter.
pub(super) struct ActiveTask {
    pub cancel: CancellationToken,
    pub live: Arc<LiveStats>,
    pub limiter: Arc<RateLimiter>,
    meter: SpeedMeter,
}

impl ActiveTask {
    pub fn new(cancel: CancellationToken, live: Arc<LiveStats>, limiter: Arc<RateLimiter>) -> Self {
        Self {
            cancel,
            live,
            limiter,
            meter: SpeedMeter::default(),
        }
    }

    /// Reads the live counters and returns the progress entry for this transfer.
    fn progress(&mut self, id: DownloadId, now: Instant) -> ProgressItem {
        let downloaded = self.live.downloaded();
        let total = self.live.total();
        let speed = self.meter.sample(downloaded, now);
        let eta = total.and_then(|total| eta_secs(total.saturating_sub(downloaded), speed));
        ProgressItem {
            id,
            downloaded,
            total,
            speed,
            eta_secs: eta,
            connections: self.live.connections(),
            phase: self.live.phase(),
            segments: self.live.segments_view(),
        }
    }
}

/// Progress entries for every running transfer, sorted by download id.
fn progress_items(active: &mut HashMap<DownloadId, ActiveTask>, now: Instant) -> Vec<ProgressItem> {
    let mut ids: Vec<DownloadId> = active.keys().copied().collect();
    ids.sort_unstable();
    ids.into_iter()
        .filter_map(|id| active.get_mut(&id).map(|task| task.progress(id, now)))
        .collect()
}

impl ManagerActor {
    /// Sends one progress batch while transfers run, and one empty batch when they stop.
    pub(super) fn emit_progress(&mut self) {
        if self.active.is_empty() {
            if !self.sent_empty_progress {
                self.sent_empty_progress = true;
                self.emit(EngineEvent::Progress(ProgressBatch {
                    items: Vec::new(),
                    total_speed: 0,
                }));
            }
            return;
        }
        self.sent_empty_progress = false;
        let items = progress_items(&mut self.active, Instant::now());
        let total_speed: u64 = items.iter().map(|item| item.speed).sum();
        self.emit(EngineEvent::Progress(ProgressBatch { items, total_speed }));
    }
}
