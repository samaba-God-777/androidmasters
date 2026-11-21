//! SQLite storage: versioned migrations, integrity check and online backups.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension};

use crate::error::{AppError, AppResult};

/// Migrations embedded in the binary, applied in order. Never edit an applied migration: add a new one.
pub const MIGRATIONS: &[(i64, &str, &str)] = &[
    (1, "initial", include_str!("../../migrations/0001_initial.sql")),
    (2, "accounts_transfer", include_str!("../../migrations/0002_accounts_transfer.sql")),
    (3, "resets", include_str!("../../migrations/0003_resets.sql")),
    (4, "orders", include_str!("../../migrations/0004_orders.sql")),
];

pub struct Database {
    conn: Mutex<Connection>,
    path: PathBuf,
}

impl Database {
    pub fn open(path: &Path) -> AppResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        Self::configure(&conn)?;
        let db = Self { conn: Mutex::new(conn), path: path.to_path_buf() };
        db.migrate()?;
        Ok(db)
    }

    /// In-memory database for tests.
    pub fn open_in_memory() -> AppResult<Self> {
        let conn = Connection::open_in_memory()?;
        Self::configure(&conn)?;
        let db = Self { conn: Mutex::new(conn), path: PathBuf::from(":memory:") };
        db.migrate()?;
        Ok(db)
    }

    fn configure(conn: &Connection) -> AppResult<()> {
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "secure_delete", "ON")?;
        Ok(())
    }

    pub fn conn(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn migrate(&self) -> AppResult<()> {
        let mut conn = self.conn();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, applied_at TEXT NOT NULL)",
        )?;
        for (version, name, sql) in MIGRATIONS {
            let applied: Option<i64> = conn
                .query_row("SELECT version FROM schema_migrations WHERE version = ?1", params![version], |r| r.get(0))
                .optional()?;
            if applied.is_some() {
                continue;
            }
            let tx = conn.transaction()?;
            tx.execute_batch(sql)?;
            tx.execute(
                "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
                params![version, name, crate::util::now()],
            )?;
            tx.commit()?;
            log::info!("Applied migration {version} ({name})");
        }
        Ok(())
    }

    pub fn schema_version(&self) -> AppResult<i64> {
        Ok(self.conn().query_row("SELECT COALESCE(MAX(version), 0) FROM schema_migrations", [], |r| r.get(0))?)
    }

    pub fn integrity_ok(&self) -> AppResult<bool> {
        let result: String = self.conn().query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        Ok(result == "ok")
    }

    /// Consistent online copy of the database (SQLite backup API) into `dest`.
    pub fn backup_to(&self, dest: &Path) -> AppResult<()> {
        if dest.exists() {
            return Err(AppError::Validation(format!("El archivo de copia ya existe: {}", dest.display())));
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = self.conn();
        let mut target = Connection::open(dest)?;
        {
            let backup = rusqlite::backup::Backup::new(&conn, &mut target)?;
            backup.run_to_completion(256, Duration::from_millis(5), None)?;
        }
        let check: String = target.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        if check != "ok" {
            return Err(AppError::Internal("La copia de seguridad no superó la comprobación de integridad".into()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_apply_once_and_are_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("am.sqlite3");
        let db = Database::open(&path).unwrap();
        assert_eq!(db.schema_version().unwrap(), MIGRATIONS.len() as i64);
        drop(db);
        let db = Database::open(&path).unwrap();
        let count: i64 = db.conn().query_row("SELECT COUNT(*) FROM schema_migrations", [], |r| r.get(0)).unwrap();
        assert_eq!(count, MIGRATIONS.len() as i64);
        assert!(db.integrity_ok().unwrap());
    }

    #[test]
    fn audit_log_is_append_only() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.conn();
        conn.execute("INSERT INTO audit_log (created_at, event, severity) VALUES ('t', 'e', 'info')", []).unwrap();
        assert!(conn.execute("UPDATE audit_log SET event = 'x'", []).is_err());
        assert!(conn.execute("DELETE FROM audit_log", []).is_err());
    }

    #[test]
    fn instruction_can_never_be_recorded_as_executed() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.conn();
        conn.execute("INSERT INTO customers (display_name, created_at) VALUES ('C', 't')", []).unwrap();
        conn.execute(
            "INSERT INTO service_orders (number, customer_id, request_type, status, received_at, created_at, updated_at) \
             VALUES ('OS-1', 1, 'frp', 'open', 't', 't', 't')",
            [],
        )
        .unwrap();
        let ok = "INSERT INTO order_entries (order_id, created_at, kind, title, status) VALUES (1, 't', 'instruction', 'x', 'info')";
        assert!(conn.execute(ok, []).is_ok());
        let bad = "INSERT INTO order_entries (order_id, created_at, kind, title, status) VALUES (1, 't', 'instruction', 'x', 'ok')";
        assert!(conn.execute(bad, []).is_err());
    }

    #[test]
    fn rejects_invalid_enums_and_dangling_references() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.conn();
        assert!(conn
            .execute("INSERT INTO users (username, display_name, role, password_hash, created_at) VALUES ('a','A','root','h','t')", [])
            .is_err());
        assert!(conn
            .execute(
                "INSERT INTO service_orders (number, customer_id, request_type, status, received_at, created_at, updated_at) \
                 VALUES ('OS-2', 999, 'frp', 'open', 't', 't', 't')",
                [],
            )
            .is_err());
    }

    #[test]
    fn backup_produces_a_valid_copy() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(&dir.path().join("am.sqlite3")).unwrap();
        db.conn().execute("INSERT INTO settings (key, value) VALUES ('k', 'v')", []).unwrap();
        let dest = dir.path().join("backups").join("copy.sqlite3");
        db.backup_to(&dest).unwrap();
        let copy = Connection::open(&dest).unwrap();
        let v: String = copy.query_row("SELECT value FROM settings WHERE key = 'k'", [], |r| r.get(0)).unwrap();
        assert_eq!(v, "v");
        assert!(db.backup_to(&dest).is_err(), "never overwrite an existing backup");
    }
}
