//! Key/value settings stored in the `settings` table.

use rusqlite::{params, Connection, OptionalExtension};

use crate::error::AppResult;

pub const PLATFORM_TOOLS_DIR: &str = "platform_tools_dir";
pub const POLL_INTERVAL_MS: &str = "device_poll_interval_ms";
pub const WORKSHOP_NAME: &str = "workshop_name";

pub fn get(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    Ok(conn.query_row("SELECT value FROM settings WHERE key = ?1", params![key], |r| r.get(0)).optional()?)
}

pub fn set(conn: &Connection, key: &str, value: Option<&str>) -> AppResult<()> {
    match value {
        Some(v) => conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, v],
        )?,
        None => conn.execute("DELETE FROM settings WHERE key = ?1", params![key])?,
    };
    Ok(())
}

pub fn poll_interval_ms(conn: &Connection) -> u64 {
    get(conn, POLL_INTERVAL_MS).ok().flatten().and_then(|v| v.parse().ok()).map_or(2000, |v: u64| v.clamp(1000, 30_000))
}
