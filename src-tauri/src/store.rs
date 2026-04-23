use std::path::Path;

use anyhow::Result;
use parking_lot::Mutex;
use rusqlite::{params, Connection};

pub struct Store {
    conn: Mutex<Connection>,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             CREATE TABLE IF NOT EXISTS keystrokes (
                 minute_ts   INTEGER PRIMARY KEY,
                 count       INTEGER NOT NULL,
                 corrections INTEGER NOT NULL DEFAULT 0
             );",
        )?;
        ensure_corrections_column(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn add_minute(&self, minute_ts: i64, keys: i64, corrections: i64) -> Result<()> {
        let k = keys.max(0);
        let c = corrections.max(0);
        if k == 0 && c == 0 {
            return Ok(());
        }
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO keystrokes (minute_ts, count, corrections) VALUES (?1, ?2, ?3)
             ON CONFLICT(minute_ts) DO UPDATE SET
                count       = count       + excluded.count,
                corrections = corrections + excluded.corrections",
            params![minute_ts, k, c],
        )?;
        Ok(())
    }

    pub fn total(&self) -> Result<(i64, i64)> {
        let conn = self.conn.lock();
        let (k, c): (Option<i64>, Option<i64>) = conn.query_row(
            "SELECT SUM(count), SUM(corrections) FROM keystrokes",
            [],
            |r| Ok((r.get::<_, Option<i64>>(0)?, r.get::<_, Option<i64>>(1)?)),
        )?;
        Ok((k.unwrap_or(0), c.unwrap_or(0)))
    }

    pub fn earliest_minute(&self) -> Result<Option<i64>> {
        let conn = self.conn.lock();
        let n: Option<i64> = conn.query_row(
            "SELECT MIN(minute_ts) FROM keystrokes",
            [],
            |r| r.get::<_, Option<i64>>(0),
        )?;
        Ok(n)
    }

    pub fn rows_in_range(
        &self,
        start_minute: i64,
        end_minute_exclusive: i64,
    ) -> Result<Vec<(i64, i64, i64)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT minute_ts, count, corrections FROM keystrokes \
             WHERE minute_ts >= ?1 AND minute_ts < ?2 ORDER BY minute_ts",
        )?;
        let rows = stmt.query_map(params![start_minute, end_minute_exclusive], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })?;
        let mut v = Vec::new();
        for r in rows {
            v.push(r?);
        }
        Ok(v)
    }
}

fn ensure_corrections_column(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(keystrokes)")?;
    let exists = stmt
        .query_map([], |r| r.get::<_, String>(1))?
        .filter_map(|r| r.ok())
        .any(|name| name == "corrections");
    if !exists {
        conn.execute(
            "ALTER TABLE keystrokes ADD COLUMN corrections INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    pub(crate) fn temp_db_path() -> std::path::PathBuf {
        let ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let i = COUNTER.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!("keycount-test-{ns}-{i}.sqlite"))
    }

    fn temp_store() -> Store {
        Store::open(&temp_db_path()).unwrap()
    }

    #[test]
    fn insert_and_sum() {
        let s = temp_store();
        s.add_minute(100, 5, 1).unwrap();
        s.add_minute(100, 3, 0).unwrap();
        s.add_minute(101, 7, 2).unwrap();
        assert_eq!(s.total().unwrap(), (15, 3));
        let rows = s.rows_in_range(100, 102).unwrap();
        assert_eq!(rows, vec![(100, 8, 1), (101, 7, 2)]);
    }

    #[test]
    fn earliest() {
        let s = temp_store();
        assert_eq!(s.earliest_minute().unwrap(), None);
        s.add_minute(200, 1, 0).unwrap();
        s.add_minute(150, 1, 0).unwrap();
        assert_eq!(s.earliest_minute().unwrap(), Some(150));
    }

    #[test]
    fn ignore_non_positive_delta() {
        let s = temp_store();
        s.add_minute(100, 0, 0).unwrap();
        s.add_minute(100, -3, -1).unwrap();
        assert_eq!(s.total().unwrap(), (0, 0));
    }

    #[test]
    fn migration_adds_corrections_column() {
        let path = temp_db_path();
        // 旧スキーマ（corrections列なし）のDBを手動で用意
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE keystrokes (
                    minute_ts INTEGER PRIMARY KEY,
                    count     INTEGER NOT NULL
                 );
                 INSERT INTO keystrokes (minute_ts, count) VALUES (100, 42);",
            )
            .unwrap();
        }
        // Store::open が ALTER TABLE で列追加 → 既存行の corrections は 0
        let s = Store::open(&path).unwrap();
        assert_eq!(s.total().unwrap(), (42, 0));
        s.add_minute(100, 0, 5).unwrap();
        assert_eq!(s.total().unwrap(), (42, 5));
    }
}
