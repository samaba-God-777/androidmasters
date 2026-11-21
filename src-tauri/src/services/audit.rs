//! Append-only audit log (the database rejects UPDATE/DELETE on `audit_log`).

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::error::AppResult;
use crate::util::now;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

impl Severity {
    fn as_str(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Critical => "critical",
        }
    }
}

pub fn record(
    conn: &Connection,
    event: &str,
    severity: Severity,
    user_id: Option<i64>,
    device_fingerprint: &str,
    details: serde_json::Value,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO audit_log (created_at, event, severity, user_id, device_fingerprint, details_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![now(), event, severity.as_str(), user_id, device_fingerprint, details.to_string()],
    )?;
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct AuditEntry {
    pub id: i64,
    pub created_at: String,
    pub event: String,
    pub severity: String,
    pub user: Option<String>,
    pub details: serde_json::Value,
}

pub fn recent(conn: &Connection, limit: i64) -> AppResult<Vec<AuditEntry>> {
    let mut stmt = conn.prepare(
        "SELECT a.id, a.created_at, a.event, a.severity, u.display_name, a.details_json \
         FROM audit_log a LEFT JOIN users u ON u.id = a.user_id ORDER BY a.id DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit], |r| {
        let details: String = r.get(5)?;
        Ok(AuditEntry {
            id: r.get(0)?,
            created_at: r.get(1)?,
            event: r.get(2)?,
            severity: r.get(3)?,
            user: r.get(4)?,
            details: serde_json::from_str(&details).unwrap_or(serde_json::Value::Null),
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}
