// SPDX-License-Identifier: GPL-3.0-or-later
//! Segment table: hands out byte ranges and splits the largest one in half.
use std::sync::{Mutex, MutexGuard};

use crate::model::SegmentView;

/// End value of a segment whose total size is not known yet.
pub(crate) const UNKNOWN_END: u64 = u64::MAX;

/// One byte range of the file. `end` is exclusive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SegmentSlot {
    pub start: u64,
    pub end: u64,
    pub fetched: u64,
    pub written: u64,
    pub active: bool,
}

impl SegmentSlot {
    pub fn pos(&self) -> u64 {
        self.start.saturating_add(self.fetched)
    }

    pub fn is_fetched(&self) -> bool {
        self.end != UNKNOWN_END && self.pos() >= self.end
    }
}

/// Result of accepting received bytes for a segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Accept {
    pub take: u64,
    pub reached_end: bool,
}

/// Shared table of segments. Slots are only appended, never removed.
pub(crate) struct SegmentTable {
    slots: Mutex<Vec<SegmentSlot>>,
}

impl SegmentTable {
    pub fn new(slots: Vec<SegmentSlot>) -> Self {
        Self {
            slots: Mutex::new(slots),
        }
    }

    /// One slot covering the whole file, or the unknown-length slot when the size is unknown.
    pub fn single(total: Option<u64>) -> Vec<SegmentSlot> {
        vec![SegmentSlot {
            start: 0,
            end: total.unwrap_or(UNKNOWN_END),
            fetched: 0,
            written: 0,
            active: false,
        }]
    }

    fn lock(&self) -> MutexGuard<'_, Vec<SegmentSlot>> {
        self.slots.lock().expect("segment table lock poisoned")
    }

    /// Hands a slot to a new worker. Unassigned slots come first. Otherwise the active slot
    /// with the largest remainder is split in half, unless the half would be below `min_split`.
    pub fn assign_next(&self, min_split: u64) -> Option<usize> {
        let mut slots = self.lock();
        if let Some(i) = slots.iter().position(|s| !s.active && !s.is_fetched()) {
            slots[i].active = true;
            return Some(i);
        }
        let mut best: Option<(usize, u64)> = None;
        for (i, s) in slots.iter().enumerate() {
            if !s.active || s.end == UNKNOWN_END {
                continue;
            }
            let rem = s.end.saturating_sub(s.pos());
            if best.is_none_or(|(_, b)| rem > b) {
                best = Some((i, rem));
            }
        }
        let (i, rem) = best?;
        if rem / 2 < min_split {
            return None;
        }
        let mid = slots[i].pos() + rem / 2;
        let old_end = slots[i].end;
        slots[i].end = mid;
        slots.push(SegmentSlot {
            start: mid,
            end: old_end,
            fetched: 0,
            written: 0,
            active: true,
        });
        Some(slots.len() - 1)
    }

    /// Records received bytes. Returns how many of them belong to this segment.
    pub fn accept(&self, idx: usize, len: u64) -> Accept {
        let mut slots = self.lock();
        let s = &mut slots[idx];
        if s.end == UNKNOWN_END {
            s.fetched = s.fetched.saturating_add(len);
            return Accept {
                take: len,
                reached_end: false,
            };
        }
        let take = len.min(s.end.saturating_sub(s.pos()));
        s.fetched += take;
        Accept {
            take,
            reached_end: s.pos() >= s.end,
        }
    }

    /// Marks bytes as written to disk.
    pub fn commit(&self, idx: usize, len: u64) {
        let mut slots = self.lock();
        let s = &mut slots[idx];
        s.written = s.written.saturating_add(len).min(s.fetched);
    }

    pub fn release(&self, idx: usize) {
        self.lock()[idx].active = false;
    }

    /// Returns `(pos, end)` of a segment.
    pub fn position(&self, idx: usize) -> (u64, u64) {
        let slots = self.lock();
        (slots[idx].pos(), slots[idx].end)
    }

    /// Ends an unknown-length segment at its current position.
    pub fn finish_unknown(&self, idx: usize) {
        let mut slots = self.lock();
        slots[idx].end = slots[idx].pos();
    }

    /// Clears the progress of a segment. Used when the server cannot resume.
    pub fn reset_fetched(&self, idx: usize) {
        let mut slots = self.lock();
        slots[idx].fetched = 0;
        slots[idx].written = 0;
    }

    pub fn snapshot(&self) -> Vec<SegmentSlot> {
        self.lock().clone()
    }

    pub fn fetched_total(&self) -> u64 {
        self.lock().iter().map(|s| s.fetched).sum()
    }

    pub fn all_fetched(&self) -> bool {
        self.lock().iter().all(SegmentSlot::is_fetched)
    }

    /// Segments for display, sorted by start. Finished idle neighbours are merged.
    pub fn view(&self) -> Vec<SegmentView> {
        let mut sorted = self.snapshot();
        sorted.sort_by_key(|s| s.start);
        let mut out: Vec<SegmentView> = Vec::new();
        for s in sorted {
            let view = SegmentView {
                start: s.start,
                end: if s.end == UNKNOWN_END { s.pos() } else { s.end },
                done: s.fetched,
                active: s.active,
            };
            if let Some(last) = out.last_mut() {
                let last_full = last.done == last.end - last.start;
                let view_full = view.done == view.end - view.start;
                if last.end == view.start && last_full && view_full && !last.active && !view.active
                {
                    last.end = view.end;
                    last.done += view.done;
                    continue;
                }
            }
            out.push(view);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(start: u64, end: u64, fetched: u64, active: bool) -> SegmentSlot {
        SegmentSlot {
            start,
            end,
            fetched,
            written: 0,
            active,
        }
    }

    #[test]
    fn fan_out_gives_eight_equal_slots() {
        let table = SegmentTable::new(SegmentTable::single(Some(800)));
        for _ in 0..8 {
            assert!(table.assign_next(1).is_some());
        }
        let snap = table.snapshot();
        assert_eq!(snap.len(), 8);
        assert!(snap.iter().all(|s| s.end - s.start == 100 && s.active));
    }

    #[test]
    fn unassigned_slots_are_preferred() {
        let table = SegmentTable::new(vec![slot(0, 50, 0, false), slot(50, 100, 0, true)]);
        assert_eq!(table.assign_next(1), Some(0));
        assert_eq!(table.snapshot().len(), 2);
    }

    #[test]
    fn small_remainder_is_not_split() {
        let table = SegmentTable::new(SegmentTable::single(Some(15)));
        assert_eq!(table.assign_next(10), Some(0));
        assert_eq!(table.assign_next(10), None);
    }

    #[test]
    fn accept_truncates_at_split_point() {
        let table = SegmentTable::new(SegmentTable::single(Some(100)));
        assert_eq!(table.assign_next(1), Some(0));
        assert_eq!(table.assign_next(1), Some(1));
        assert_eq!(
            table.accept(0, 80),
            Accept {
                take: 50,
                reached_end: true
            }
        );
        assert_eq!(table.position(0), (50, 50));
    }

    #[test]
    fn split_picks_largest_remaining() {
        let table = SegmentTable::new(vec![
            slot(0, 10, 0, true),
            slot(10, 100, 0, true),
            slot(100, 130, 0, true),
        ]);
        assert_eq!(table.assign_next(1), Some(3));
        let snap = table.snapshot();
        assert_eq!((snap[1].start, snap[1].end), (10, 55));
        assert_eq!((snap[3].start, snap[3].end), (55, 100));
    }

    #[test]
    fn view_merges_contiguous_finished_neighbours() {
        let table = SegmentTable::new(vec![
            slot(50, 100, 50, false),
            slot(0, 50, 50, false),
            slot(100, 150, 10, true),
        ]);
        let v = table.view();
        assert_eq!(v.len(), 2);
        assert_eq!(
            v[0],
            SegmentView {
                start: 0,
                end: 100,
                done: 100,
                active: false
            }
        );
        assert_eq!(
            v[1],
            SegmentView {
                start: 100,
                end: 150,
                done: 10,
                active: true
            }
        );
    }

    fn check_cover(slots: &[SegmentSlot], total: u64) {
        let mut sorted = slots.to_vec();
        sorted.sort_by_key(|s| s.start);
        assert_eq!(sorted[0].start, 0);
        for w in sorted.windows(2) {
            assert_eq!(w[0].end, w[1].start);
        }
        assert_eq!(sorted[sorted.len() - 1].end, total);
        for s in &sorted {
            assert!(s.start < s.end);
            assert!(s.start <= s.pos() && s.pos() <= s.end);
            assert!(s.written <= s.fetched);
        }
    }

    #[test]
    fn random_operations_keep_slots_covering_total() {
        const TOTAL: u64 = 10_000;
        let table = SegmentTable::new(SegmentTable::single(Some(TOTAL)));
        let mut seed: u64 = 0x2545_F491_4F6C_DD1D;
        let mut next = move |n: u64| {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (seed >> 33) % n
        };
        for _ in 0..5000 {
            let len = table.snapshot().len() as u64;
            match next(3) {
                0 => {
                    let min = 1 + next(400);
                    table.assign_next(min);
                }
                1 => {
                    let idx = next(len) as usize;
                    let n = 1 + next(300);
                    table.accept(idx, n);
                    table.commit(idx, n);
                }
                _ => table.release(next(len) as usize),
            }
            check_cover(&table.snapshot(), TOTAL);
        }
    }
}
