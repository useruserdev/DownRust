// SPDX-License-Identifier: GPL-3.0-or-later
//! Statistics tables: totals per local day and per category, and the backfill of older rows.
use std::collections::BTreeMap;

use rusqlite::{params, Connection};

use super::rows::{from_db, to_db};
use crate::{
    error::Result,
    model::{Category, StatsDay, StatsTotals},
    util::time::local_day,
};

/// Adds one completed file to the totals of its local day and its category.
pub(super) fn add_completion(
    conn: &mut Connection,
    day: &str,
    category: Category,
    bytes: u64,
) -> Result<()> {
    let tx = conn.transaction()?;
    upsert(&tx, "stats_daily", "day", day, bytes)?;
    upsert(&tx, "stats_category", "category", category.as_str(), bytes)?;
    tx.commit()?;
    Ok(())
}

/// Table and column names are fixed in this module, never taken from the user.
fn upsert(conn: &Connection, table: &str, key_column: &str, key: &str, bytes: u64) -> Result<()> {
    let sql = format!(
        "INSERT INTO {table} ({key_column}, bytes, files) VALUES (?1, ?2, 1) \
         ON CONFLICT({key_column}) DO UPDATE SET bytes = bytes + excluded.bytes, files = files + 1"
    );
    conn.execute(&sql, params![key, to_db(bytes)])?;
    Ok(())
}

/// Daily totals on or after `first_day` (`YYYY-MM-DD`), oldest first.
pub(super) fn daily_since(conn: &Connection, first_day: &str) -> Result<Vec<StatsDay>> {
    let mut stmt =
        conn.prepare("SELECT day, bytes, files FROM stats_daily WHERE day >= ?1 ORDER BY day")?;
    let rows = stmt.query_map(params![first_day], |row| {
        Ok(StatsDay {
            day: row.get(0)?,
            bytes: from_db(row.get(1)?),
            files: from_db(row.get(2)?),
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Totals of every stored category. Unknown category names are skipped.
pub(super) fn category_totals(conn: &Connection) -> Result<Vec<(Category, StatsTotals)>> {
    let mut stmt = conn.prepare("SELECT category, bytes, files FROM stats_category")?;
    let rows = stmt.query_map([], |row| {
        let name: String = row.get(0)?;
        Ok((
            name,
            StatsTotals {
                bytes: from_db(row.get(1)?),
                files: from_db(row.get(2)?),
            },
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (name, totals) = row?;
        if let Some(category) = Category::parse(&name) {
            out.push((category, totals));
        }
    }
    Ok(out)
}

/// Fills both tables from the completed downloads. Each download counts on the local day of its
/// completion, or of its creation when no completion time is stored.
pub(super) fn backfill(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT category, downloaded, completed_at, created_at FROM downloads \
         WHERE status = 'completed'",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, Option<i64>>(2)?,
            row.get::<_, i64>(3)?,
        ))
    })?;
    let mut daily: BTreeMap<String, StatsTotals> = BTreeMap::new();
    let mut by_category: BTreeMap<String, StatsTotals> = BTreeMap::new();
    for row in rows {
        let (category, downloaded, completed_at, created_at) = row?;
        let Some(day) = local_day(completed_at.unwrap_or(created_at)) else {
            continue;
        };
        let bytes = from_db(downloaded);
        add_to(daily.entry(day).or_default(), bytes);
        add_to(by_category.entry(category).or_default(), bytes);
    }
    drop(stmt);
    for (day, totals) in &daily {
        insert_totals(conn, "stats_daily", "day", day, *totals)?;
    }
    for (category, totals) in &by_category {
        insert_totals(conn, "stats_category", "category", category, *totals)?;
    }
    Ok(())
}

fn add_to(totals: &mut StatsTotals, bytes: u64) {
    totals.bytes = totals.bytes.saturating_add(bytes);
    totals.files = totals.files.saturating_add(1);
}

fn insert_totals(
    conn: &Connection,
    table: &str,
    key_column: &str,
    key: &str,
    totals: StatsTotals,
) -> Result<()> {
    let sql = format!("INSERT INTO {table} ({key_column}, bytes, files) VALUES (?1, ?2, ?3)");
    conn.execute(&sql, params![key, to_db(totals.bytes), to_db(totals.files)])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> Connection {
        let mut conn = Connection::open_in_memory().expect("open");
        super::super::schema::configure(&conn).expect("configure");
        super::super::schema::migrate(&mut conn).expect("migrate");
        conn
    }

    #[test]
    fn completions_accumulate_per_day_and_category() {
        let mut conn = fresh();
        add_completion(&mut conn, "2026-01-02", Category::Music, 10).expect("add");
        add_completion(&mut conn, "2026-01-02", Category::Music, 5).expect("add");
        add_completion(&mut conn, "2026-01-02", Category::Video, 1).expect("add");
        let days = daily_since(&conn, "2026-01-01").expect("days");
        assert_eq!(days.len(), 1);
        assert_eq!((days[0].bytes, days[0].files), (16, 3));
        let categories = category_totals(&conn).expect("categories");
        assert!(categories.contains(&(
            Category::Music,
            StatsTotals {
                bytes: 15,
                files: 2
            }
        )));
    }

    #[test]
    fn days_before_the_first_day_are_left_out() {
        let mut conn = fresh();
        add_completion(&mut conn, "2025-12-31", Category::Other, 7).expect("add");
        add_completion(&mut conn, "2026-01-01", Category::Other, 3).expect("add");
        let days = daily_since(&conn, "2026-01-01").expect("days");
        assert_eq!(days.len(), 1);
        assert_eq!(days[0].day, "2026-01-01");
    }
}
