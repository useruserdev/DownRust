// SPDX-License-Identifier: GPL-3.0-or-later
//! Settings storage queries.
use rusqlite::{params, Connection, OptionalExtension};

use crate::error::Result;

pub(super) fn load_json(conn: &Connection) -> Result<Option<String>> {
    Ok(conn
        .query_row("SELECT json FROM app_settings WHERE id = 1", [], |row| {
            row.get(0)
        })
        .optional()?)
}

pub(super) fn save_json(conn: &Connection, json: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO app_settings (id, json) VALUES (1, ?1) \
         ON CONFLICT(id) DO UPDATE SET json = excluded.json",
        params![json],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::store::Store;

    #[test]
    fn settings_round_trip_and_upsert() {
        let store = Store::open_in_memory().expect("open");
        assert_eq!(store.load_settings_json().expect("empty"), None);
        store.save_settings_json("{\"a\":1}").expect("first save");
        assert_eq!(
            store.load_settings_json().expect("load"),
            Some("{\"a\":1}".to_owned())
        );
        store.save_settings_json("{\"a\":2}").expect("second save");
        assert_eq!(
            store.load_settings_json().expect("load"),
            Some("{\"a\":2}".to_owned())
        );
    }
}
