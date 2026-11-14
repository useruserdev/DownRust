// SPDX-License-Identifier: GPL-3.0-or-later
//! Progress bookkeeping for one HLS stream: which parts are done, the size estimate and the
//! segment buckets shown in the interface.
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::engine::live::LiveStats;
use crate::model::SegmentView;

/// Most buckets published for the segment bar.
const MAX_VIEWS: usize = 64;
/// Parts that must be done before the total is estimated from their average size.
const ESTIMATE_AFTER: usize = 3;

/// Files of one downloaded stream, in playback order.
#[derive(Debug, Clone)]
pub(crate) struct Stream {
    pub init: Option<PathBuf>,
    pub parts: Vec<PathBuf>,
    pub fmp4: bool,
    /// Bytes of the stored parts.
    pub bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PartState {
    Pending,
    Active,
    Done,
}

struct Inner {
    states: Vec<PartState>,
    done: usize,
    done_bytes: u64,
    base_bytes: u64,
}

/// Tracks the parts of one stream and publishes progress to the live statistics.
pub(crate) struct Tracker {
    live: Arc<LiveStats>,
    inner: Mutex<Inner>,
}

impl Tracker {
    /// `base_bytes` is the size of streams finished before this one.
    pub fn new(live: Arc<LiveStats>, count: usize, base_bytes: u64) -> Self {
        Self {
            live,
            inner: Mutex::new(Inner {
                states: vec![PartState::Pending; count],
                done: 0,
                done_bytes: 0,
                base_bytes,
            }),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().expect("tracker lock poisoned")
    }

    pub fn start(&self, index: usize) {
        let mut inner = self.lock();
        if let Some(state) = inner.states.get_mut(index) {
            if *state == PartState::Pending {
                *state = PartState::Active;
            }
        }
        self.publish(&inner);
    }

    /// Marks a part as stored. Calling it twice for one part counts it once.
    pub fn finish(&self, index: usize, len: u64) {
        let mut inner = self.lock();
        if let Some(state) = inner.states.get_mut(index) {
            if *state != PartState::Done {
                *state = PartState::Done;
                inner.done += 1;
                inner.done_bytes += len;
            }
        }
        self.publish(&inner);
    }

    /// Bytes of the parts stored so far in this stream.
    pub fn total_bytes(&self) -> u64 {
        self.lock().done_bytes
    }

    fn publish(&self, inner: &Inner) {
        self.live.set_media_views(views(&inner.states));
        self.live.set_media_total(estimate(inner));
    }
}

/// Exact total when all parts are done, an estimate from the average part size after a few
/// parts, and nothing before that.
fn estimate(inner: &Inner) -> Option<u64> {
    let count = inner.states.len();
    if inner.done == count {
        Some(inner.base_bytes + inner.done_bytes)
    } else if inner.done >= ESTIMATE_AFTER {
        let average_total = inner.done_bytes * count as u64 / inner.done as u64;
        Some(inner.base_bytes + average_total)
    } else {
        None
    }
}

/// Splits the parts into at most `MAX_VIEWS` buckets. `start` and `end` are part indices.
pub(crate) fn views(states: &[PartState]) -> Vec<SegmentView> {
    let count = states.len();
    if count == 0 {
        return Vec::new();
    }
    let buckets = count.min(MAX_VIEWS);
    (0..buckets)
        .map(|bucket| {
            let lo = bucket * count / buckets;
            let hi = (bucket + 1) * count / buckets;
            let slice = &states[lo..hi];
            SegmentView {
                start: lo as u64,
                end: hi as u64,
                done: slice.iter().filter(|s| **s == PartState::Done).count() as u64,
                active: slice.contains(&PartState::Active),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn views_count_done_parts_per_bucket() {
        let states = [
            PartState::Done,
            PartState::Done,
            PartState::Active,
            PartState::Pending,
        ];
        let views = views(&states);
        assert_eq!(views.len(), 4);
        assert_eq!(views[0].done, 1);
        assert!(!views[0].active);
        assert!(views[2].active);
        assert_eq!(views[3].done, 0);
    }

    #[test]
    fn views_are_capped() {
        let states = vec![PartState::Pending; 500];
        assert_eq!(views(&states).len(), MAX_VIEWS);
        assert!(views(&[]).is_empty());
    }

    #[test]
    fn total_is_estimated_then_exact() {
        let live = Arc::new(LiveStats::new(1));
        let tracker = Tracker::new(Arc::clone(&live), 10, 0);
        assert_eq!(live.total(), None);
        for index in 0..3 {
            tracker.finish(index, 100);
        }
        assert_eq!(live.total(), Some(1000));
        tracker.finish(0, 100);
        assert_eq!(tracker.total_bytes(), 300);
        for index in 3..10 {
            tracker.finish(index, 100);
        }
        assert_eq!(live.total(), Some(1000));
    }

    #[test]
    fn base_bytes_are_added_to_the_estimate() {
        let live = Arc::new(LiveStats::new(1));
        let tracker = Tracker::new(Arc::clone(&live), 2, 500);
        tracker.finish(0, 10);
        tracker.finish(1, 10);
        assert_eq!(live.total(), Some(520));
    }
}
