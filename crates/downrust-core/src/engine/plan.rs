// SPDX-License-Identifier: GPL-3.0-or-later
//! Decides whether saved progress can be reused or the transfer starts over.
use crate::model::{DownloadRecord, ProbeInfo, SegmentRow};

use super::segments::{SegmentSlot, SegmentTable};

const DISCARD_NOTE: &str =
    "Saved progress cannot be reused. The download starts again from the beginning.";

/// What the transfer task does with the file on this attempt.
pub(crate) struct TransferPlan {
    pub segmented: bool,
    pub total: Option<u64>,
    pub slots: Vec<SegmentSlot>,
    pub fresh: bool,
    pub preallocate: Option<u64>,
    pub note: Option<String>,
}

/// Plans one attempt. Saved segments are reused only when the server still sends the same file.
pub(crate) fn decide(
    prev: &DownloadRecord,
    probe: &ProbeInfo,
    saved: &[SegmentRow],
    part_exists: bool,
    preallocate: bool,
    force_single: bool,
) -> TransferPlan {
    let segmented = !force_single && probe.resumable && probe.total_size.is_some_and(|t| t > 0);
    let reuse = segmented
        && prev.resumable
        && part_exists
        && !saved.is_empty()
        && prev.total_size == probe.total_size
        && validator_ok(&prev.etag, &probe.etag)
        && validator_ok(&prev.last_modified, &probe.last_modified);
    if reuse {
        let slots = saved
            .iter()
            .map(|r| SegmentSlot {
                start: r.start,
                end: r.end,
                fetched: r.written,
                written: r.written,
                active: false,
            })
            .collect();
        return TransferPlan {
            segmented,
            total: probe.total_size,
            slots,
            fresh: false,
            preallocate: None,
            note: None,
        };
    }
    TransferPlan {
        segmented,
        total: probe.total_size,
        slots: SegmentTable::single(probe.total_size),
        fresh: true,
        preallocate: if segmented && preallocate {
            probe.total_size
        } else {
            None
        },
        note: (prev.downloaded > 0).then(|| DISCARD_NOTE.to_string()),
    }
}

/// Two validators conflict only when both sides hold a value and the values differ.
fn validator_ok(prev: &Option<String>, now: &Option<String>) -> bool {
    match (prev, now) {
        (Some(a), Some(b)) => a == b,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Category, DownloadId, DownloadStatus};

    fn record(total: Option<u64>, downloaded: u64, etag: Option<&str>) -> DownloadRecord {
        DownloadRecord {
            id: 1 as DownloadId,
            url: "https://example.test/a.bin".into(),
            final_url: None,
            referrer: None,
            file_name: "a.bin".into(),
            save_dir: "C:/dl".into(),
            category: Category::Other,
            status: DownloadStatus::Paused,
            total_size: total,
            downloaded,
            resumable: true,
            connections: 4,
            speed_limit: None,
            etag: etag.map(Into::into),
            last_modified: None,
            content_type: None,
            error: None,
            queue_position: 1,
            headers: vec![],
            created_at: 0,
            completed_at: None,
            expected_sha256: None,
            checksum: None,
            scan: None,
            kind: crate::model::DownloadKind::File,
        }
    }

    fn probe(total: Option<u64>, etag: Option<&str>) -> ProbeInfo {
        ProbeInfo {
            url: "https://example.test/a.bin".into(),
            final_url: "https://example.test/a.bin".into(),
            file_name: "a.bin".into(),
            total_size: total,
            resumable: true,
            content_type: None,
            etag: etag.map(Into::into),
            last_modified: None,
            category: Category::Other,
        }
    }

    fn rows() -> Vec<SegmentRow> {
        vec![
            SegmentRow {
                start: 0,
                end: 500,
                written: 300,
            },
            SegmentRow {
                start: 500,
                end: 1000,
                written: 100,
            },
        ]
    }

    #[test]
    fn reuses_saved_segments_when_validators_match() {
        let p = decide(
            &record(Some(1000), 400, Some("a")),
            &probe(Some(1000), Some("a")),
            &rows(),
            true,
            true,
            false,
        );
        assert!(p.segmented);
        assert!(!p.fresh);
        assert_eq!(p.total, Some(1000));
        assert_eq!(p.slots.len(), 2);
        assert_eq!(
            (p.slots[0].fetched, p.slots[0].written, p.slots[0].active),
            (300, 300, false)
        );
        assert_eq!((p.slots[1].start, p.slots[1].fetched), (500, 100));
        assert_eq!(p.preallocate, None);
        assert_eq!(p.note, None);
    }

    #[test]
    fn etag_change_starts_fresh() {
        let p = decide(
            &record(Some(1000), 0, Some("a")),
            &probe(Some(1000), Some("b")),
            &rows(),
            true,
            false,
            false,
        );
        assert!(p.fresh);
        assert_eq!(p.slots.len(), 1);
        assert_eq!(p.slots[0].end, 1000);
    }

    #[test]
    fn size_change_starts_fresh() {
        let p = decide(
            &record(Some(1000), 0, None),
            &probe(Some(2000), None),
            &rows(),
            true,
            false,
            false,
        );
        assert!(p.fresh);
        assert_eq!(p.total, Some(2000));
    }

    #[test]
    fn last_modified_change_starts_fresh() {
        let prev = DownloadRecord {
            last_modified: Some("Mon".into()),
            ..record(Some(1000), 0, None)
        };
        let pr = ProbeInfo {
            last_modified: Some("Tue".into()),
            ..probe(Some(1000), None)
        };
        assert!(decide(&prev, &pr, &rows(), true, false, false).fresh);
    }

    #[test]
    fn missing_part_file_starts_fresh() {
        let p = decide(
            &record(Some(1000), 0, None),
            &probe(Some(1000), None),
            &rows(),
            false,
            false,
            false,
        );
        assert!(p.fresh);
    }

    #[test]
    fn force_single_disables_segments_and_reuse() {
        let p = decide(
            &record(Some(1000), 400, None),
            &probe(Some(1000), None),
            &rows(),
            true,
            true,
            true,
        );
        assert!(!p.segmented);
        assert!(p.fresh);
        assert_eq!(p.slots.len(), 1);
        assert_eq!(p.preallocate, None);
    }

    #[test]
    fn fresh_segmented_plan_preallocates_when_enabled() {
        let p = decide(
            &record(Some(1000), 0, None),
            &probe(Some(1000), None),
            &[],
            false,
            true,
            false,
        );
        assert!(p.fresh);
        assert_eq!(p.preallocate, Some(1000));
        let off = decide(
            &record(Some(1000), 0, None),
            &probe(Some(1000), None),
            &[],
            false,
            false,
            false,
        );
        assert_eq!(off.preallocate, None);
    }

    #[test]
    fn note_is_set_only_when_progress_is_discarded() {
        let lost = decide(
            &record(Some(1000), 300, Some("a")),
            &probe(Some(1000), Some("b")),
            &rows(),
            true,
            false,
            false,
        );
        assert!(lost.note.is_some());
        let none = decide(
            &record(Some(1000), 0, Some("a")),
            &probe(Some(1000), Some("b")),
            &rows(),
            true,
            false,
            false,
        );
        assert!(none.note.is_none());
    }
}
