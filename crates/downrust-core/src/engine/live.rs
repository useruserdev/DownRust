// SPDX-License-Identifier: GPL-3.0-or-later
//! Live transfer statistics shared between the task, its workers and the manager.
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use crate::model::{Phase, SegmentView};

use super::segments::{SegmentTable, UNKNOWN_END};

pub(crate) struct LiveStats {
    table: Mutex<Option<Arc<SegmentTable>>>,
    total: AtomicU64,
    connections: AtomicU32,
    target: AtomicU32,
    phase: AtomicU8,
    /// Bytes read so far by an HLS or DASH transfer, which has no segment table.
    media_done: AtomicU64,
    /// Estimated total of an HLS or DASH transfer, `UNKNOWN_END` while not known.
    media_total: AtomicU64,
    /// Segment buckets of an HLS transfer; `start` and `end` are segment indices there.
    media_views: Mutex<Vec<SegmentView>>,
}

fn phase_to_u8(phase: Phase) -> u8 {
    match phase {
        Phase::Connecting => 0,
        Phase::Downloading => 1,
        Phase::Finalizing => 2,
    }
}

fn phase_from_u8(value: u8) -> Phase {
    match value {
        1 => Phase::Downloading,
        2 => Phase::Finalizing,
        _ => Phase::Connecting,
    }
}

impl LiveStats {
    pub fn new(target: u32) -> Self {
        Self {
            table: Mutex::new(None),
            total: AtomicU64::new(UNKNOWN_END),
            connections: AtomicU32::new(0),
            target: AtomicU32::new(target),
            phase: AtomicU8::new(phase_to_u8(Phase::Connecting)),
            media_done: AtomicU64::new(0),
            media_total: AtomicU64::new(UNKNOWN_END),
            media_views: Mutex::new(Vec::new()),
        }
    }

    /// Adds bytes read by an HLS or DASH transfer.
    pub fn add_media_bytes(&self, n: u64) {
        self.media_done.fetch_add(n, Ordering::SeqCst);
    }

    /// Takes back bytes of a request that failed and will be sent again.
    pub fn sub_media_bytes(&self, n: u64) {
        let mut current = self.media_done.load(Ordering::SeqCst);
        loop {
            let next = current.saturating_sub(n);
            match self.media_done.compare_exchange(
                current,
                next,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => break,
                Err(actual) => current = actual,
            }
        }
    }

    pub fn set_media_done(&self, n: u64) {
        self.media_done.store(n, Ordering::SeqCst);
    }

    pub fn set_media_total(&self, total: Option<u64>) {
        self.media_total
            .store(total.unwrap_or(UNKNOWN_END), Ordering::SeqCst);
    }

    pub fn set_media_views(&self, views: Vec<SegmentView>) {
        *self.media_views.lock().expect("media view lock poisoned") = views;
    }

    fn table(&self) -> Option<Arc<SegmentTable>> {
        self.table.lock().expect("live table lock poisoned").clone()
    }

    /// Connects the statistics to the segment table of the current attempt.
    pub fn attach(&self, table: Arc<SegmentTable>, total: Option<u64>) {
        *self.table.lock().expect("live table lock poisoned") = Some(table);
        self.total
            .store(total.unwrap_or(UNKNOWN_END), Ordering::SeqCst);
    }

    pub fn downloaded(&self) -> u64 {
        match self.table() {
            Some(table) => table.fetched_total(),
            None => self.media_done.load(Ordering::SeqCst),
        }
    }

    pub fn total(&self) -> Option<u64> {
        let total = match self.total.load(Ordering::SeqCst) {
            UNKNOWN_END => self.media_total.load(Ordering::SeqCst),
            total => total,
        };
        match total {
            UNKNOWN_END => None,
            total => Some(total),
        }
    }

    pub fn connections(&self) -> u32 {
        self.connections.load(Ordering::SeqCst)
    }

    pub fn set_connections(&self, n: u32) {
        self.connections.store(n, Ordering::SeqCst);
    }

    pub fn target(&self) -> u32 {
        self.target.load(Ordering::SeqCst)
    }

    pub fn set_target(&self, n: u32) {
        self.target.store(n, Ordering::SeqCst);
    }

    /// Lowers the connection target by one, never below 1.
    pub fn reduce_target(&self) {
        let mut current = self.target.load(Ordering::SeqCst);
        loop {
            let next = current.saturating_sub(1).max(1);
            match self
                .target
                .compare_exchange(current, next, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => break,
                Err(actual) => current = actual,
            }
        }
    }

    pub fn phase(&self) -> Phase {
        phase_from_u8(self.phase.load(Ordering::SeqCst))
    }

    pub fn set_phase(&self, phase: Phase) {
        self.phase.store(phase_to_u8(phase), Ordering::SeqCst);
    }

    pub fn segments_view(&self) -> Vec<SegmentView> {
        match self.table() {
            Some(table) => table.view(),
            None => self
                .media_views
                .lock()
                .expect("media view lock poisoned")
                .clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::segments::SegmentSlot;

    #[test]
    fn target_never_drops_below_one() {
        let live = LiveStats::new(2);
        live.reduce_target();
        assert_eq!(live.target(), 1);
        live.reduce_target();
        assert_eq!(live.target(), 1);
    }

    #[test]
    fn phase_round_trips_and_total_is_optional() {
        let live = LiveStats::new(1);
        assert_eq!(live.phase(), Phase::Connecting);
        live.set_phase(Phase::Finalizing);
        assert_eq!(live.phase(), Phase::Finalizing);
        assert_eq!(live.total(), None);
        let table = Arc::new(SegmentTable::new(vec![SegmentSlot {
            start: 0,
            end: 10,
            fetched: 4,
            written: 0,
            active: true,
        }]));
        live.attach(table, Some(10));
        assert_eq!(live.total(), Some(10));
        assert_eq!(live.downloaded(), 4);
    }
}
