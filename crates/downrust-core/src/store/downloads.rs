// SPDX-License-Identifier: GPL-3.0-or-later
//! Download table queries.
use rusqlite::{params, Connection, OptionalExtension, Params};

use super::{
    rows::{headers_to_json, row_to_record, to_db, RECORD_COLUMNS},
    segments,
};
use crate::{
    error::{Error, Result},
    model::{DownloadId, DownloadRecord, DownloadStatus, NewDownload, ProbeInfo, ScanState},
};

pub(super) fn insert(conn: &Connection, new: &NewDownload) -> Result<DownloadRecord> {
    let headers_json = headers_to_json(&new.headers)?;
    conn.execute(
        "INSERT INTO downloads (url, final_url, referrer, file_name, save_dir, category, status, \
         total_size, downloaded, resumable, connections, speed_limit, etag, last_modified, \
         content_type, error, queue_position, headers_json, created_at, completed_at, \
         expected_sha256, kind, mirrors_json, options_json) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9, ?10, ?11, ?12, ?13, ?14, NULL, ?15, ?16, ?17, \
         NULL, ?18, ?19, ?20, ?21)",
        params![
            new.url,
            new.final_url,
            new.referrer,
            new.file_name,
            new.save_dir,
            new.category.as_str(),
            new.status.as_str(),
            new.total_size.map(to_db),
            i64::from(new.resumable),
            i64::from(new.connections),
            new.speed_limit.map(to_db),
            new.etag,
            new.last_modified,
            new.content_type,
            new.queue_position,
            headers_json,
            new.created_at,
            new.expected_sha256,
            new.kind.as_str(),
            serde_json::to_string(&new.mirrors)?,
            new.options,
        ],
    )?;
    let id = conn.last_insert_rowid();
    get(conn, id)?.ok_or(Error::NotFound(id))
}

pub(super) fn get(conn: &Connection, id: DownloadId) -> Result<Option<DownloadRecord>> {
    let sql = format!("SELECT {RECORD_COLUMNS} FROM downloads WHERE id = ?1");
    Ok(conn
        .query_row(&sql, params![id], row_to_record)
        .optional()?)
}

/// ORDER BY id DESC
pub(super) fn list_all(conn: &Connection) -> Result<Vec<DownloadRecord>> {
    let sql = format!("SELECT {RECORD_COLUMNS} FROM downloads ORDER BY id DESC");
    query_records(conn, &sql, [])
}

/// ORDER BY queue_position, id
pub(super) fn list_by_status(
    conn: &Connection,
    status: DownloadStatus,
) -> Result<Vec<DownloadRecord>> {
    let sql = format!(
        "SELECT {RECORD_COLUMNS} FROM downloads WHERE status = ?1 ORDER BY queue_position, id"
    );
    query_records(conn, &sql, params![status.as_str()])
}

fn query_records(conn: &Connection, sql: &str, args: impl Params) -> Result<Vec<DownloadRecord>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(args, row_to_record)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub(super) fn update_status(
    conn: &Connection,
    id: DownloadId,
    status: DownloadStatus,
    error: Option<&str>,
) -> Result<()> {
    conn.execute(
        "UPDATE downloads SET status = ?2, error = ?3 WHERE id = ?1",
        params![id, status.as_str(), error],
    )?;
    Ok(())
}

pub(super) fn mark_completed(conn: &Connection, id: DownloadId, completed_at: i64) -> Result<()> {
    conn.execute(
        "UPDATE downloads SET status = 'completed', error = NULL, completed_at = ?2 WHERE id = ?1",
        params![id, completed_at],
    )?;
    Ok(())
}

pub(super) fn update_meta(conn: &Connection, id: DownloadId, probe: &ProbeInfo) -> Result<()> {
    conn.execute(
        "UPDATE downloads SET final_url = ?2, total_size = ?3, resumable = ?4, \
         content_type = ?5, etag = ?6, last_modified = ?7 WHERE id = ?1",
        params![
            id,
            probe.final_url,
            probe.total_size.map(to_db),
            i64::from(probe.resumable),
            probe.content_type,
            probe.etag,
            probe.last_modified,
        ],
    )?;
    Ok(())
}

pub(super) fn record_finished_file(
    conn: &mut Connection,
    id: DownloadId,
    file_name: &str,
    total: u64,
) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE downloads SET file_name = ?2, downloaded = ?3, total_size = ?3 WHERE id = ?1",
        params![id, file_name, to_db(total)],
    )?;
    segments::delete(&tx, id)?;
    tx.commit()?;
    Ok(())
}

/// Stores the name and the total size of a torrent once its metadata is known.
pub(super) fn set_name_and_size(
    conn: &Connection,
    id: DownloadId,
    file_name: &str,
    total: u64,
) -> Result<()> {
    conn.execute(
        "UPDATE downloads SET file_name = ?2, total_size = ?3 WHERE id = ?1",
        params![id, file_name, to_db(total)],
    )?;
    Ok(())
}

pub(super) fn set_scan_state(
    conn: &Connection,
    id: DownloadId,
    state: Option<ScanState>,
) -> Result<()> {
    conn.execute(
        "UPDATE downloads SET scan_state = ?2 WHERE id = ?1",
        params![id, state.map(ScanState::as_str)],
    )?;
    Ok(())
}

pub(super) fn set_connections(conn: &Connection, id: DownloadId, connections: u8) -> Result<()> {
    conn.execute(
        "UPDATE downloads SET connections = ?2 WHERE id = ?1",
        params![id, i64::from(connections)],
    )?;
    Ok(())
}

pub(super) fn set_speed_limit(conn: &Connection, id: DownloadId, limit: Option<u64>) -> Result<()> {
    conn.execute(
        "UPDATE downloads SET speed_limit = ?2 WHERE id = ?1",
        params![id, limit.map(to_db)],
    )?;
    Ok(())
}

/// Segments are removed by the foreign key cascade.
pub(super) fn delete(conn: &Connection, id: DownloadId) -> Result<()> {
    conn.execute("DELETE FROM downloads WHERE id = ?1", params![id])?;
    Ok(())
}

/// COALESCE(MAX, 0) + 1
pub(super) fn next_queue_position(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT COALESCE(MAX(queue_position), 0) + 1 FROM downloads",
        [],
        |row| row.get(0),
    )?)
}

/// Case-insensitive match on save_dir and file_name.
pub(super) fn is_name_taken(conn: &Connection, save_dir: &str, file_name: &str) -> Result<bool> {
    let found: i64 = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM downloads WHERE LOWER(save_dir) = LOWER(?1) \
         AND LOWER(file_name) = LOWER(?2))",
        params![save_dir, file_name],
        |row| row.get(0),
    )?;
    Ok(found != 0)
}

pub(super) fn recover_interrupted(conn: &Connection, to: DownloadStatus) -> Result<usize> {
    Ok(conn.execute(
        "UPDATE downloads SET status = ?1 WHERE status = 'downloading'",
        params![to.as_str()],
    )?)
}

/// Sets downloaded to 0, completed_at and error to NULL, and deletes the segments.
pub(super) fn clear_progress(conn: &mut Connection, id: DownloadId) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE downloads SET downloaded = 0, completed_at = NULL, error = NULL WHERE id = ?1",
        params![id],
    )?;
    segments::delete(&tx, id)?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
pub(super) fn sample_download(file_name: &str) -> NewDownload {
    NewDownload {
        url: format!("https://example.com/files/{file_name}"),
        final_url: None,
        referrer: None,
        file_name: file_name.to_owned(),
        save_dir: "C:/Downloads".to_owned(),
        category: crate::model::Category::Documents,
        status: DownloadStatus::Queued,
        total_size: Some(1000),
        resumable: true,
        connections: 8,
        speed_limit: None,
        etag: None,
        last_modified: None,
        content_type: None,
        headers: Vec::new(),
        queue_position: 1,
        created_at: 100,
        expected_sha256: None,
        kind: crate::model::DownloadKind::File,
        mirrors: Vec::new(),
        options: None,
    }
}

#[cfg(test)]
mod tests {
    use super::{super::Store, sample_download};
    use crate::model::{DownloadStatus, HeaderPair, ProbeInfo};

    #[test]
    fn insert_then_get_round_trips_every_field() {
        let store = Store::open_in_memory().expect("open");
        let mut new = sample_download("guide.pdf");
        new.final_url = Some("https://cdn.example.com/guide.pdf".into());
        new.referrer = Some("https://example.com/".into());
        new.etag = Some("\"abc\"".into());
        new.last_modified = Some("Wed, 01 May 2024 12:00:00 GMT".into());
        new.content_type = Some("application/pdf".into());
        new.speed_limit = Some(4096);
        new.headers = vec![HeaderPair {
            name: "Cookie".into(),
            value: "a=b".into(),
        }];
        let inserted = store.insert_download(&new).expect("insert");
        let got = store
            .get_download(inserted.id)
            .expect("get")
            .expect("row exists");
        assert_eq!(got, inserted);
        assert_eq!(got.file_name, "guide.pdf");
        assert_eq!(got.headers, new.headers);
        assert_eq!(got.speed_limit, Some(4096));
        assert_eq!(got.downloaded, 0);
        assert_eq!(got.error, None);
        assert_eq!(got.completed_at, None);
        assert_eq!(got.status, DownloadStatus::Queued);
    }

    #[test]
    fn get_missing_returns_none() {
        let store = Store::open_in_memory().expect("open");
        assert_eq!(store.get_download(42).expect("get"), None);
    }

    #[test]
    fn list_downloads_is_newest_first() {
        let store = Store::open_in_memory().expect("open");
        let a = store.insert_download(&sample_download("a.zip")).expect("a");
        let b = store.insert_download(&sample_download("b.zip")).expect("b");
        let ids: Vec<_> = store
            .list_downloads()
            .expect("list")
            .iter()
            .map(|r| r.id)
            .collect();
        assert_eq!(ids, vec![b.id, a.id]);
    }

    #[test]
    fn list_by_status_orders_by_queue_position_then_id() {
        let store = Store::open_in_memory().expect("open");
        let mut late = sample_download("late.zip");
        late.queue_position = 5;
        let mut early = sample_download("early.zip");
        early.queue_position = 2;
        let mut other = sample_download("other.zip");
        other.status = DownloadStatus::Paused;
        let late_id = store.insert_download(&late).expect("late").id;
        let early_id = store.insert_download(&early).expect("early").id;
        store.insert_download(&other).expect("other");
        let ids: Vec<_> = store
            .list_by_status(DownloadStatus::Queued)
            .expect("list")
            .iter()
            .map(|r| r.id)
            .collect();
        assert_eq!(ids, vec![early_id, late_id]);
    }

    #[test]
    fn status_updates_and_completion_clear_error() {
        let store = Store::open_in_memory().expect("open");
        let id = store
            .insert_download(&sample_download("x.bin"))
            .expect("i")
            .id;
        store
            .update_status(id, DownloadStatus::Error, Some("HTTP 404"))
            .expect("error");
        let row = store.get_download(id).expect("get").expect("row");
        assert_eq!(row.status, DownloadStatus::Error);
        assert_eq!(row.error.as_deref(), Some("HTTP 404"));
        store.mark_completed(id, 777).expect("complete");
        let row = store.get_download(id).expect("get").expect("row");
        assert_eq!(row.status, DownloadStatus::Completed);
        assert_eq!(row.error, None);
        assert_eq!(row.completed_at, Some(777));
    }

    #[test]
    fn update_meta_stores_probe_fields() {
        let store = Store::open_in_memory().expect("open");
        let id = store
            .insert_download(&sample_download("m.bin"))
            .expect("i")
            .id;
        let probe = ProbeInfo {
            url: "https://example.com/m.bin".into(),
            final_url: "https://cdn.example.com/m.bin".into(),
            file_name: "m.bin".into(),
            total_size: Some(5_000),
            resumable: false,
            content_type: Some("application/octet-stream".into()),
            etag: Some("\"e1\"".into()),
            last_modified: Some("lm".into()),
            category: crate::model::Category::Other,
        };
        store.update_meta(id, &probe).expect("meta");
        let row = store.get_download(id).expect("get").expect("row");
        assert_eq!(
            row.final_url.as_deref(),
            Some("https://cdn.example.com/m.bin")
        );
        assert_eq!(row.total_size, Some(5_000));
        assert!(!row.resumable);
        assert_eq!(row.etag.as_deref(), Some("\"e1\""));
        assert_eq!(row.last_modified.as_deref(), Some("lm"));
        assert_eq!(
            row.content_type.as_deref(),
            Some("application/octet-stream")
        );
    }

    #[test]
    fn record_finished_file_and_clear_progress() {
        let store = Store::open_in_memory().expect("open");
        let id = store
            .insert_download(&sample_download("f.bin"))
            .expect("i")
            .id;
        store
            .save_checkpoint(
                id,
                400,
                &[crate::model::SegmentRow {
                    start: 0,
                    end: 1000,
                    written: 400,
                }],
            )
            .expect("checkpoint");
        store
            .record_finished_file(id, "renamed.bin", 1000)
            .expect("finish");
        let row = store.get_download(id).expect("get").expect("row");
        assert_eq!(row.file_name, "renamed.bin");
        assert_eq!(row.downloaded, 1000);
        assert!(store.load_segments(id).expect("segments").is_empty());

        store
            .save_checkpoint(
                id,
                10,
                &[crate::model::SegmentRow {
                    start: 0,
                    end: 1000,
                    written: 10,
                }],
            )
            .expect("checkpoint");
        store.mark_completed(id, 5).expect("complete");
        store.clear_progress(id).expect("clear");
        let row = store.get_download(id).expect("get").expect("row");
        assert_eq!(row.downloaded, 0);
        assert_eq!(row.completed_at, None);
        assert_eq!(row.error, None);
        assert!(store.load_segments(id).expect("segments").is_empty());
    }

    #[test]
    fn set_connections_and_speed_limit() {
        let store = Store::open_in_memory().expect("open");
        let id = store
            .insert_download(&sample_download("s.bin"))
            .expect("i")
            .id;
        store.set_connections(id, 3).expect("connections");
        store.set_speed_limit(id, Some(2048)).expect("limit");
        let row = store.get_download(id).expect("get").expect("row");
        assert_eq!(row.connections, 3);
        assert_eq!(row.speed_limit, Some(2048));
        store.set_speed_limit(id, None).expect("clear limit");
        let row = store.get_download(id).expect("get").expect("row");
        assert_eq!(row.speed_limit, None);
    }

    #[test]
    fn delete_removes_row() {
        let store = Store::open_in_memory().expect("open");
        let id = store
            .insert_download(&sample_download("d.bin"))
            .expect("i")
            .id;
        store.delete_download(id).expect("delete");
        assert_eq!(store.get_download(id).expect("get"), None);
    }

    #[test]
    fn next_queue_position_is_max_plus_one() {
        let store = Store::open_in_memory().expect("open");
        assert_eq!(store.next_queue_position().expect("empty"), 1);
        let mut new = sample_download("q.bin");
        new.queue_position = 7;
        store.insert_download(&new).expect("insert");
        assert_eq!(store.next_queue_position().expect("next"), 8);
    }

    #[test]
    fn is_name_taken_ignores_case() {
        let store = Store::open_in_memory().expect("open");
        store
            .insert_download(&sample_download("Report.PDF"))
            .expect("insert");
        assert!(store
            .is_name_taken("c:/downloads", "report.pdf")
            .expect("check"));
        assert!(!store
            .is_name_taken("C:/Other", "report.pdf")
            .expect("check"));
    }

    #[test]
    fn recover_interrupted_counts_downloading_rows() {
        let store = Store::open_in_memory().expect("open");
        let mut busy = sample_download("busy.bin");
        busy.status = DownloadStatus::Downloading;
        let id = store.insert_download(&busy).expect("busy").id;
        store
            .insert_download(&sample_download("idle.bin"))
            .expect("idle");
        let changed = store
            .recover_interrupted(DownloadStatus::Paused)
            .expect("recover");
        assert_eq!(changed, 1);
        let row = store.get_download(id).expect("get").expect("row");
        assert_eq!(row.status, DownloadStatus::Paused);
    }

    #[test]
    fn u64_values_above_half_of_i64_max_round_trip() {
        let store = Store::open_in_memory().expect("open");
        let half = u64::try_from(i64::MAX / 2 + 1).expect("fits");
        let mut new = sample_download("huge.bin");
        new.total_size = Some(u64::MAX - 1);
        new.speed_limit = Some(half);
        let id = store.insert_download(&new).expect("insert").id;
        let row = store.get_download(id).expect("get").expect("row");
        assert_eq!(row.total_size, Some(u64::MAX - 1));
        assert_eq!(row.speed_limit, Some(half));
    }
}
