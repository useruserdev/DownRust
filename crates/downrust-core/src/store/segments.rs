// SPDX-License-Identifier: GPL-3.0-or-later
//! Segment table queries.
use rusqlite::{params, Connection};

use super::rows::{from_db, to_db};
use crate::{
    error::Result,
    model::{DownloadId, SegmentRow},
};

/// Replaces the stored progress of one download in a single transaction.
pub(super) fn save_checkpoint(
    conn: &mut Connection,
    id: DownloadId,
    downloaded: u64,
    segments: &[SegmentRow],
) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE downloads SET downloaded = ?2 WHERE id = ?1",
        params![id, to_db(downloaded)],
    )?;
    delete(&tx, id)?;
    for (index, seg) in segments.iter().enumerate() {
        tx.execute(
            "INSERT INTO segments (download_id, seg_index, seg_start, seg_end, seg_written) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                id,
                index as i64,
                to_db(seg.start),
                to_db(seg.end),
                to_db(seg.written)
            ],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// ORDER BY seg_index
pub(super) fn load(conn: &Connection, id: DownloadId) -> Result<Vec<SegmentRow>> {
    let mut stmt = conn.prepare(
        "SELECT seg_start, seg_end, seg_written FROM segments \
         WHERE download_id = ?1 ORDER BY seg_index",
    )?;
    let rows = stmt.query_map(params![id], |row| {
        Ok(SegmentRow {
            start: from_db(row.get(0)?),
            end: from_db(row.get(1)?),
            written: from_db(row.get(2)?),
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub(super) fn delete(conn: &Connection, id: DownloadId) -> Result<()> {
    conn.execute("DELETE FROM segments WHERE download_id = ?1", params![id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::{downloads::sample_download, Store};
    use crate::model::SegmentRow;

    fn seg(start: u64, end: u64, written: u64) -> SegmentRow {
        SegmentRow {
            start,
            end,
            written,
        }
    }

    #[test]
    fn save_then_load_keeps_order() {
        let store = Store::open_in_memory().expect("open");
        let id = store
            .insert_download(&sample_download("s.bin"))
            .expect("i")
            .id;
        let rows = [seg(0, 500, 120), seg(500, 1000, 0)];
        store.save_checkpoint(id, 120, &rows).expect("save");
        assert_eq!(store.load_segments(id).expect("load"), rows.to_vec());
        let row = store.get_download(id).expect("get").expect("row");
        assert_eq!(row.downloaded, 120);
    }

    #[test]
    fn save_replaces_previous_segments() {
        let store = Store::open_in_memory().expect("open");
        let id = store
            .insert_download(&sample_download("r.bin"))
            .expect("i")
            .id;
        store
            .save_checkpoint(id, 10, &[seg(0, 500, 5), seg(500, 1000, 5)])
            .expect("first");
        store
            .save_checkpoint(id, 300, &[seg(0, 300, 300)])
            .expect("second");
        assert_eq!(
            store.load_segments(id).expect("load"),
            vec![seg(0, 300, 300)]
        );
    }

    #[test]
    fn segments_of_other_downloads_are_separate() {
        let store = Store::open_in_memory().expect("open");
        let a = store
            .insert_download(&sample_download("a.bin"))
            .expect("a")
            .id;
        let b = store
            .insert_download(&sample_download("b.bin"))
            .expect("b")
            .id;
        store.save_checkpoint(a, 1, &[seg(0, 10, 1)]).expect("a");
        store.save_checkpoint(b, 2, &[seg(0, 20, 2)]).expect("b");
        assert_eq!(store.load_segments(a).expect("load a"), vec![seg(0, 10, 1)]);
        assert_eq!(store.load_segments(b).expect("load b"), vec![seg(0, 20, 2)]);
    }

    #[test]
    fn deleting_download_removes_its_segments() {
        let store = Store::open_in_memory().expect("open");
        let id = store
            .insert_download(&sample_download("c.bin"))
            .expect("i")
            .id;
        store
            .save_checkpoint(id, 1, &[seg(0, 10, 1)])
            .expect("save");
        store.delete_download(id).expect("delete");
        assert!(store.load_segments(id).expect("load").is_empty());
    }

    #[test]
    fn large_u64_segment_values_round_trip() {
        let store = Store::open_in_memory().expect("open");
        let id = store
            .insert_download(&sample_download("big.bin"))
            .expect("i")
            .id;
        let start = u64::try_from(i64::MAX / 2 + 5).expect("fits");
        let row = seg(start, u64::MAX, u64::MAX - 9);
        store
            .save_checkpoint(id, u64::MAX - 7, &[row])
            .expect("save");
        assert_eq!(store.load_segments(id).expect("load"), vec![row]);
        let stored = store.get_download(id).expect("get").expect("row");
        assert_eq!(stored.downloaded, u64::MAX - 7);
    }
}
