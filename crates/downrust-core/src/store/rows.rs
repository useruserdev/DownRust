// SPDX-License-Identifier: GPL-3.0-or-later
//! Row mapping for stored records.
use rusqlite::{types::Type, Row};

use crate::{
    error::Result,
    model::{
        Category, ChecksumState, DownloadKind, DownloadRecord, DownloadStatus, HeaderPair,
        ScanState,
    },
};

/// Column list of `downloads`, in the order that `row_to_record` reads them.
pub(super) const RECORD_COLUMNS: &str = "id, url, final_url, referrer, file_name, save_dir, \
    category, status, total_size, downloaded, resumable, connections, speed_limit, etag, \
    last_modified, content_type, error, queue_position, headers_json, created_at, completed_at, \
    expected_sha256, checksum_state, scan_state, kind";

/// Stores a u64 as an i64 with the same bits.
pub(super) fn to_db(value: u64) -> i64 {
    value as i64
}

/// Reads a u64 back from an i64 written by `to_db`.
pub(super) fn from_db(value: i64) -> u64 {
    value as u64
}

pub(super) fn headers_to_json(headers: &[HeaderPair]) -> Result<String> {
    Ok(serde_json::to_string(headers)?)
}

/// Maps one row selected with `RECORD_COLUMNS`. Unknown category text becomes `Other`,
/// and unknown status text becomes `Paused`.
pub(super) fn row_to_record(row: &Row<'_>) -> rusqlite::Result<DownloadRecord> {
    let category: String = row.get(6)?;
    let status: String = row.get(7)?;
    let headers_json: String = row.get(18)?;
    let headers: Vec<HeaderPair> = serde_json::from_str(&headers_json)
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(18, Type::Text, Box::new(err)))?;
    Ok(DownloadRecord {
        id: row.get(0)?,
        url: row.get(1)?,
        final_url: row.get(2)?,
        referrer: row.get(3)?,
        file_name: row.get(4)?,
        save_dir: row.get(5)?,
        category: Category::parse(&category).unwrap_or(Category::Other),
        status: DownloadStatus::parse(&status).unwrap_or(DownloadStatus::Paused),
        total_size: row.get::<_, Option<i64>>(8)?.map(from_db),
        downloaded: from_db(row.get(9)?),
        resumable: row.get::<_, i64>(10)? != 0,
        connections: row.get::<_, i64>(11)? as u8,
        speed_limit: row.get::<_, Option<i64>>(12)?.map(from_db),
        etag: row.get(13)?,
        last_modified: row.get(14)?,
        content_type: row.get(15)?,
        error: row.get(16)?,
        queue_position: row.get(17)?,
        headers,
        created_at: row.get(19)?,
        completed_at: row.get(20)?,
        expected_sha256: row.get(21)?,
        checksum: row
            .get::<_, Option<String>>(22)?
            .and_then(|text| ChecksumState::parse(&text)),
        scan: row
            .get::<_, Option<String>>(23)?
            .and_then(|text| ScanState::parse(&text)),
        kind: row
            .get::<_, Option<String>>(24)?
            .and_then(|text| DownloadKind::parse(&text))
            .unwrap_or_default(),
    })
}
