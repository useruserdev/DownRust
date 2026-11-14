// SPDX-License-Identifier: GPL-3.0-or-later
//! Persisted state of one transfer segment.

/// One byte range of a download. `end` is exclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegmentRow {
    pub start: u64,
    pub end: u64,
    pub written: u64,
}
