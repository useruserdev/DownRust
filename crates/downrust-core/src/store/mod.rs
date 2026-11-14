// SPDX-License-Identifier: GPL-3.0-or-later
//! SQLite persistence for downloads, segments and settings.
mod downloads;
mod edits;
mod rows;
mod schema;
mod segments;
mod settings;
mod stats;

use std::sync::{Arc, Mutex, MutexGuard};

use rusqlite::Connection;

use crate::{
    error::Result,
    model::{
        Category, ChecksumState, DownloadId, DownloadRecord, DownloadStatus, NewDownload,
        ProbeInfo, ScanState, SegmentRow, StatsDay, StatsTotals,
    },
};

#[derive(Clone)]
pub struct Store {
    conn: Arc<Mutex<Connection>>,
}

impl Store {
    pub fn open(path: &std::path::Path) -> Result<Self> {
        Self::from_connection(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(mut conn: Connection) -> Result<Self> {
        schema::configure(&conn)?;
        schema::migrate(&mut conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    fn lock(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().expect("store connection lock poisoned")
    }

    pub fn insert_download(&self, new: &NewDownload) -> Result<DownloadRecord> {
        downloads::insert(&self.lock(), new)
    }

    pub fn get_download(&self, id: DownloadId) -> Result<Option<DownloadRecord>> {
        downloads::get(&self.lock(), id)
    }

    /// ORDER BY id DESC
    pub fn list_downloads(&self) -> Result<Vec<DownloadRecord>> {
        downloads::list_all(&self.lock())
    }

    /// ORDER BY queue_position, id
    pub fn list_by_status(&self, status: DownloadStatus) -> Result<Vec<DownloadRecord>> {
        downloads::list_by_status(&self.lock(), status)
    }

    pub fn update_status(
        &self,
        id: DownloadId,
        status: DownloadStatus,
        error: Option<&str>,
    ) -> Result<()> {
        downloads::update_status(&self.lock(), id, status, error)
    }

    /// Sets status to completed and error to NULL.
    pub fn mark_completed(&self, id: DownloadId, completed_at: i64) -> Result<()> {
        downloads::mark_completed(&self.lock(), id, completed_at)
    }

    /// Updates final_url, total_size, resumable, content_type, etag and last_modified.
    pub fn update_meta(&self, id: DownloadId, probe: &ProbeInfo) -> Result<()> {
        downloads::update_meta(&self.lock(), id, probe)
    }

    /// Sets file_name, downloaded to total, and deletes the segments.
    pub fn record_finished_file(&self, id: DownloadId, file_name: &str, total: u64) -> Result<()> {
        downloads::record_finished_file(&mut self.lock(), id, file_name, total)
    }

    /// Stores the virus scan result. `None` clears it.
    pub fn set_scan_state(&self, id: DownloadId, state: Option<ScanState>) -> Result<()> {
        downloads::set_scan_state(&self.lock(), id, state)
    }

    /// Adds one completed file of `bytes` to the totals of `day` (`YYYY-MM-DD`) and `category`.
    pub fn add_completion_stats(&self, day: &str, category: Category, bytes: u64) -> Result<()> {
        stats::add_completion(&mut self.lock(), day, category, bytes)
    }

    /// Daily totals on or after `first_day` (`YYYY-MM-DD`), oldest first.
    pub fn daily_stats_since(&self, first_day: &str) -> Result<Vec<StatsDay>> {
        stats::daily_since(&self.lock(), first_day)
    }

    /// All-time totals per category, for the categories that have any rows.
    pub fn category_stats(&self) -> Result<Vec<(Category, StatsTotals)>> {
        stats::category_totals(&self.lock())
    }

    pub fn set_connections(&self, id: DownloadId, connections: u8) -> Result<()> {
        downloads::set_connections(&self.lock(), id, connections)
    }

    pub fn set_speed_limit(&self, id: DownloadId, limit: Option<u64>) -> Result<()> {
        downloads::set_speed_limit(&self.lock(), id, limit)
    }

    /// Writes new queue positions in one transaction.
    pub fn set_queue_positions(&self, updates: &[(DownloadId, i64)]) -> Result<()> {
        edits::set_queue_positions(&mut self.lock(), updates)
    }

    /// Stores a new address with the probe's size, resume support and validators.
    pub fn update_url(&self, id: DownloadId, url: &str, probe: &ProbeInfo) -> Result<()> {
        edits::update_url(&self.lock(), id, url, probe)
    }

    /// Stores the expected SHA-256 and the checksum state. `None` clears the field.
    pub fn set_checksum(
        &self,
        id: DownloadId,
        expected: Option<&str>,
        state: Option<ChecksumState>,
    ) -> Result<()> {
        edits::set_checksum(&self.lock(), id, expected, state)
    }

    pub fn delete_download(&self, id: DownloadId) -> Result<()> {
        downloads::delete(&self.lock(), id)
    }

    /// COALESCE(MAX, 0) + 1
    pub fn next_queue_position(&self) -> Result<i64> {
        downloads::next_queue_position(&self.lock())
    }

    /// Case-insensitive match on save_dir and file_name.
    pub fn is_name_taken(&self, save_dir: &str, file_name: &str) -> Result<bool> {
        downloads::is_name_taken(&self.lock(), save_dir, file_name)
    }

    /// Changes rows with status 'downloading' to `to`. Returns the number of rows changed.
    pub fn recover_interrupted(&self, to: DownloadStatus) -> Result<usize> {
        downloads::recover_interrupted(&self.lock(), to)
    }

    pub fn save_checkpoint(
        &self,
        id: DownloadId,
        downloaded: u64,
        segments: &[SegmentRow],
    ) -> Result<()> {
        segments::save_checkpoint(&mut self.lock(), id, downloaded, segments)
    }

    /// ORDER BY seg_index
    pub fn load_segments(&self, id: DownloadId) -> Result<Vec<SegmentRow>> {
        segments::load(&self.lock(), id)
    }

    /// Sets downloaded to 0, completed_at and error to NULL, and deletes the segments.
    pub fn clear_progress(&self, id: DownloadId) -> Result<()> {
        downloads::clear_progress(&mut self.lock(), id)
    }

    pub fn load_settings_json(&self) -> Result<Option<String>> {
        settings::load_json(&self.lock())
    }

    pub fn save_settings_json(&self, json: &str) -> Result<()> {
        settings::save_json(&self.lock(), json)
    }
}
