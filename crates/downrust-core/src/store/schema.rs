// SPDX-License-Identifier: GPL-3.0-or-later
//! Database schema and migrations.
use rusqlite::Connection;

use super::stats;
use crate::error::Result;

/// Connection settings. `execute_batch` accepts PRAGMAs that return a row.
const PRAGMAS: &str = "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; \
    PRAGMA foreign_keys=ON; PRAGMA busy_timeout=5000;";

/// Migration steps. Step `n` (1-based) runs once and then sets `PRAGMA user_version = n`.
const MIGRATIONS: &[&str] = &[
    r#"
CREATE TABLE downloads (
  id INTEGER PRIMARY KEY AUTOINCREMENT, url TEXT NOT NULL, final_url TEXT, referrer TEXT,
  file_name TEXT NOT NULL, save_dir TEXT NOT NULL, category TEXT NOT NULL, status TEXT NOT NULL,
  total_size INTEGER, downloaded INTEGER NOT NULL DEFAULT 0, resumable INTEGER NOT NULL DEFAULT 0,
  connections INTEGER NOT NULL, speed_limit INTEGER, etag TEXT, last_modified TEXT, content_type TEXT,
  error TEXT, queue_position INTEGER NOT NULL, headers_json TEXT NOT NULL DEFAULT '[]',
  created_at INTEGER NOT NULL, completed_at INTEGER);
CREATE INDEX idx_downloads_status ON downloads(status, queue_position);
CREATE TABLE segments (
  download_id INTEGER NOT NULL REFERENCES downloads(id) ON DELETE CASCADE,
  seg_index INTEGER NOT NULL, seg_start INTEGER NOT NULL, seg_end INTEGER NOT NULL, seg_written INTEGER NOT NULL,
  PRIMARY KEY (download_id, seg_index));
CREATE TABLE app_settings (id INTEGER PRIMARY KEY CHECK (id = 1), json TEXT NOT NULL);
"#,
    r#"
ALTER TABLE downloads ADD COLUMN expected_sha256 TEXT NULL;
ALTER TABLE downloads ADD COLUMN checksum_state TEXT NULL;
"#,
    r#"
ALTER TABLE downloads ADD COLUMN scan_state TEXT NULL;
CREATE TABLE stats_daily (day TEXT PRIMARY KEY, bytes INTEGER NOT NULL, files INTEGER NOT NULL);
CREATE TABLE stats_category (category TEXT PRIMARY KEY, bytes INTEGER NOT NULL, files INTEGER NOT NULL);
"#,
    r#"
ALTER TABLE downloads ADD COLUMN kind TEXT NOT NULL DEFAULT 'file';
"#,
    r#"
ALTER TABLE downloads ADD COLUMN mirrors_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE downloads ADD COLUMN options_json TEXT NULL;
"#,
];

/// Version that creates the statistics tables. Its step also fills them from older downloads.
const STATS_VERSION: usize = 3;

/// Applies the connection PRAGMAs. Must run before any transaction.
pub(super) fn configure(conn: &Connection) -> Result<()> {
    conn.execute_batch(PRAGMAS)?;
    Ok(())
}

/// Runs every migration whose number is greater than `PRAGMA user_version`.
/// Each step runs in its own transaction together with the version update.
pub(super) fn migrate(conn: &mut Connection) -> Result<()> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let start = usize::try_from(current).unwrap_or(0);
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(start) {
        let version = index + 1;
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        if version == STATS_VERSION {
            stats::backfill(&tx)?;
        }
        tx.execute_batch(&format!("PRAGMA user_version = {version}"))?;
        tx.commit()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::{configure, migrate, MIGRATIONS};
    use crate::store::Store;

    fn user_version(store: &Store) -> i64 {
        store
            .lock()
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("read user_version")
    }

    fn table_count(store: &Store) -> i64 {
        store
            .lock()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' \
                 AND name IN ('downloads', 'segments', 'app_settings')",
                [],
                |row| row.get(0),
            )
            .expect("count tables")
    }

    fn has_checksum_columns(conn: &Connection) -> bool {
        let mut stmt = conn
            .prepare("PRAGMA table_info(downloads)")
            .expect("table info");
        let names: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .expect("query")
            .map(|name| name.expect("column name"))
            .collect();
        names.iter().any(|n| n == "expected_sha256") && names.iter().any(|n| n == "checksum_state")
    }

    #[test]
    fn fresh_file_database_is_migrated() {
        let dir = tempfile::tempdir().expect("temp dir");
        let store = Store::open(&dir.path().join("downrust.db")).expect("open");
        assert_eq!(user_version(&store), 5);
        assert_eq!(table_count(&store), 3);
        assert!(has_checksum_columns(&store.lock()));
    }

    #[test]
    fn reopening_keeps_version_and_tables() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("downrust.db");
        drop(Store::open(&path).expect("first open"));
        let store = Store::open(&path).expect("second open");
        assert_eq!(user_version(&store), 5);
        assert_eq!(table_count(&store), 3);
    }

    #[test]
    fn in_memory_database_is_migrated() {
        let store = Store::open_in_memory().expect("open");
        assert_eq!(user_version(&store), 5);
        assert_eq!(table_count(&store), 3);
    }

    #[test]
    fn version_one_database_is_upgraded_and_keeps_its_rows() {
        let mut conn = Connection::open_in_memory().expect("open");
        configure(&conn).expect("configure");
        conn.execute_batch(MIGRATIONS[0]).expect("version 1 schema");
        conn.execute_batch("PRAGMA user_version = 1")
            .expect("set version 1");
        conn.execute(
            "INSERT INTO downloads (url, file_name, save_dir, category, status, connections, \
             queue_position, created_at) VALUES ('https://example.com/a', 'a.bin', 'C:/d', \
             'other', 'paused', 4, 1, 0)",
            [],
        )
        .expect("insert version 1 row");

        migrate(&mut conn).expect("upgrade");

        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("version");
        assert_eq!(version, 5);
        assert!(has_checksum_columns(&conn));
        let (name, expected): (String, Option<String>) = conn
            .query_row(
                "SELECT file_name, expected_sha256 FROM downloads",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("old row");
        assert_eq!(name, "a.bin");
        assert_eq!(expected, None);
    }

    #[test]
    fn stats_tables_are_filled_from_completed_downloads() {
        let mut conn = Connection::open_in_memory().expect("open");
        configure(&conn).expect("configure");
        conn.execute_batch(MIGRATIONS[0]).expect("version 1 schema");
        conn.execute_batch(MIGRATIONS[1]).expect("version 2 schema");
        conn.execute_batch("PRAGMA user_version = 2")
            .expect("set version 2");
        let day_ms = 1_700_000_000_000_i64;
        conn.execute(
            "INSERT INTO downloads (url, file_name, save_dir, category, status, connections,              queue_position, created_at, completed_at, downloaded) VALUES              ('https://example.com/a', 'a.zip', 'C:/d', 'archives', 'completed', 4, 1, ?1, ?1, 2048)",
            [day_ms],
        )
        .expect("insert completed row");
        conn.execute(
            "INSERT INTO downloads (url, file_name, save_dir, category, status, connections,              queue_position, created_at, downloaded) VALUES              ('https://example.com/b', 'b.bin', 'C:/d', 'other', 'paused', 4, 2, ?1, 99)",
            [day_ms],
        )
        .expect("insert paused row");

        migrate(&mut conn).expect("upgrade");

        let (bytes, files): (i64, i64) = conn
            .query_row(
                "SELECT bytes, files FROM stats_category WHERE category = 'archives'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("category row");
        assert_eq!((bytes, files), (2048, 1));
        let daily: i64 = conn
            .query_row("SELECT COUNT(*) FROM stats_daily", [], |row| row.get(0))
            .expect("daily rows");
        assert_eq!(daily, 1);
        let other: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM stats_category WHERE category = 'other'",
                [],
                |row| row.get(0),
            )
            .expect("other rows");
        assert_eq!(other, 0);
    }

    #[test]
    fn version_three_database_gets_file_kind() {
        let mut conn = Connection::open_in_memory().expect("open");
        configure(&conn).expect("configure");
        for sql in &MIGRATIONS[..3] {
            conn.execute_batch(sql).expect("version 1 to 3 schema");
        }
        conn.execute_batch("PRAGMA user_version = 3")
            .expect("set version 3");
        conn.execute(
            "INSERT INTO downloads (url, file_name, save_dir, category, status, connections, \
             queue_position, created_at) VALUES ('https://example.com/a', 'a.bin', 'C:/d', \
             'other', 'paused', 4, 1, 0)",
            [],
        )
        .expect("insert version 3 row");

        migrate(&mut conn).expect("upgrade");

        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("version");
        assert_eq!(version, 5);
        let kind: String = conn
            .query_row("SELECT kind FROM downloads", [], |row| row.get(0))
            .expect("kind column");
        assert_eq!(kind, "file");
    }

    #[test]
    fn fresh_database_has_kind_column() {
        let store = Store::open_in_memory().expect("open");
        let conn = store.lock();
        let found: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('downloads') WHERE name = 'kind'",
                [],
                |row| row.get(0),
            )
            .expect("table info");
        assert_eq!(found, 1);
    }
}
