// SPDX-License-Identifier: GPL-3.0-or-later
//! Queries that change one field group of an existing download: queue order, address and checksum.
use rusqlite::{params, Connection};

use crate::{
    error::Result,
    model::{ChecksumState, DownloadId, ProbeInfo},
};

/// Writes new queue positions in one transaction.
pub(super) fn set_queue_positions(
    conn: &mut Connection,
    updates: &[(DownloadId, i64)],
) -> Result<()> {
    let tx = conn.transaction()?;
    for (id, position) in updates {
        tx.execute(
            "UPDATE downloads SET queue_position = ?2 WHERE id = ?1",
            params![id, position],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// Stores a new address together with the size, resume support and validators the probe found.
pub(super) fn update_url(
    conn: &Connection,
    id: DownloadId,
    url: &str,
    probe: &ProbeInfo,
) -> Result<()> {
    conn.execute(
        "UPDATE downloads SET url = ?2, final_url = ?3, total_size = ?4, resumable = ?5, \
         content_type = ?6, etag = ?7, last_modified = ?8 WHERE id = ?1",
        params![
            id,
            url,
            probe.final_url,
            probe.total_size.map(super::rows::to_db),
            i64::from(probe.resumable),
            probe.content_type,
            probe.etag,
            probe.last_modified,
        ],
    )?;
    Ok(())
}

/// Stores the expected SHA-256 and the checksum state. `None` clears the field.
pub(super) fn set_checksum(
    conn: &Connection,
    id: DownloadId,
    expected: Option<&str>,
    state: Option<ChecksumState>,
) -> Result<()> {
    conn.execute(
        "UPDATE downloads SET expected_sha256 = ?2, checksum_state = ?3 WHERE id = ?1",
        params![id, expected, state.map(ChecksumState::as_str)],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::{downloads::sample_download, Store};
    use crate::model::{ChecksumState, DownloadStatus, ProbeInfo};

    #[test]
    fn queue_positions_are_written_together() {
        let store = Store::open_in_memory().expect("open");
        let a = store
            .insert_download(&sample_download("a.bin"))
            .expect("a")
            .id;
        let b = store
            .insert_download(&sample_download("b.bin"))
            .expect("b")
            .id;
        store
            .set_queue_positions(&[(a, 2), (b, 1)])
            .expect("positions");
        let order: Vec<_> = store
            .list_by_status(DownloadStatus::Queued)
            .expect("list")
            .iter()
            .map(|r| r.id)
            .collect();
        assert_eq!(order, vec![b, a]);
    }

    #[test]
    fn update_url_stores_new_address_and_probe_fields() {
        let store = Store::open_in_memory().expect("open");
        let id = store
            .insert_download(&sample_download("u.bin"))
            .expect("i")
            .id;
        let probe = ProbeInfo {
            url: "https://new.example.com/u.bin".into(),
            final_url: "https://cdn.example.com/u.bin".into(),
            file_name: "u.bin".into(),
            total_size: Some(77),
            resumable: true,
            content_type: None,
            etag: Some("\"new\"".into()),
            last_modified: None,
            category: crate::model::Category::Other,
        };
        store
            .update_url(id, "https://new.example.com/u.bin", &probe)
            .expect("update");
        let row = store.get_download(id).expect("get").expect("row");
        assert_eq!(row.url, "https://new.example.com/u.bin");
        assert_eq!(
            row.final_url.as_deref(),
            Some("https://cdn.example.com/u.bin")
        );
        assert_eq!(row.total_size, Some(77));
        assert_eq!(row.etag.as_deref(), Some("\"new\""));
    }

    #[test]
    fn checksum_fields_round_trip_and_clear() {
        let store = Store::open_in_memory().expect("open");
        let id = store
            .insert_download(&sample_download("c.bin"))
            .expect("i")
            .id;
        store
            .set_checksum(id, Some("ab"), Some(ChecksumState::Mismatch))
            .expect("set");
        let row = store.get_download(id).expect("get").expect("row");
        assert_eq!(row.expected_sha256.as_deref(), Some("ab"));
        assert_eq!(row.checksum, Some(ChecksumState::Mismatch));
        store.set_checksum(id, None, None).expect("clear");
        let row = store.get_download(id).expect("get").expect("row");
        assert_eq!(row.expected_sha256, None);
        assert_eq!(row.checksum, None);
    }
}
