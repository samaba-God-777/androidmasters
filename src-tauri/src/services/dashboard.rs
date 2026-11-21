//! Laboratory statistics computed from real records only (no sample data).

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::error::AppResult;
use crate::services::audit::{self, AuditEntry};

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct StatusCount {
    pub status: String,
    pub count: i64,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct DayCount {
    pub day: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct DashboardStats {
    pub orders_active: i64,
    pub orders_pending_verification: i64,
    pub orders_completed_30d: i64,
    pub diagnostics_30d: i64,
    pub entries_pending_verification: i64,
    pub orders_by_status: Vec<StatusCount>,
    pub operations_14d: Vec<DayCount>,
    pub recent_activity: Vec<AuditEntry>,
}

fn count(conn: &Connection, sql: &str) -> AppResult<i64> {
    Ok(conn.query_row(sql, [], |r| r.get(0))?)
}

pub fn stats(conn: &Connection) -> AppResult<DashboardStats> {
    let orders_by_status = {
        let mut stmt = conn.prepare("SELECT status, COUNT(*) FROM service_orders GROUP BY status ORDER BY COUNT(*) DESC")?;
        let rows = stmt.query_map([], |r| Ok(StatusCount { status: r.get(0)?, count: r.get(1)? }))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    let operations_14d = {
        let mut stmt = conn.prepare(
            "WITH RECURSIVE days(d) AS (SELECT date('now', 'localtime', '-13 days') UNION ALL SELECT date(d, '+1 day') FROM days \
             WHERE d < date('now', 'localtime')) \
             SELECT d, (SELECT COUNT(*) FROM operations WHERE substr(started_at, 1, 10) = d) FROM days ORDER BY d",
        )?;
        let rows = stmt.query_map(params![], |r| Ok(DayCount { day: r.get(0)?, count: r.get(1)? }))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    Ok(DashboardStats {
        orders_active: count(conn, "SELECT COUNT(*) FROM service_orders WHERE status IN ('open', 'in_progress', 'pending_verification')")?,
        orders_pending_verification: count(conn, "SELECT COUNT(*) FROM service_orders WHERE status = 'pending_verification'")?,
        orders_completed_30d: count(
            conn,
            "SELECT COUNT(*) FROM service_orders WHERE status IN ('completed', 'delivered') AND updated_at >= datetime('now', 'localtime', '-30 days')",
        )?,
        // Every diagnosis (USB, FRP, accounts) is logged once as an operation; automatic identifications are not diagnoses.
        diagnostics_30d: count(
            conn,
            "SELECT COUNT(*) FROM operations WHERE category IN ('diagnostics', 'usb', 'frp', 'accounts') \
             AND operation <> 'Identificación del dispositivo' AND started_at >= datetime('now', 'localtime', '-30 days')",
        )?,
        entries_pending_verification: count(conn, "SELECT COUNT(*) FROM order_entries WHERE status = 'pending_verification'")?,
        orders_by_status,
        operations_14d,
        recent_activity: audit::recent(conn, 10)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;

    #[test]
    fn empty_database_reports_zeros_not_samples() {
        let db = Database::open_in_memory().unwrap();
        let s = stats(&db.conn()).unwrap();
        assert_eq!((s.orders_active, s.diagnostics_30d, s.entries_pending_verification), (0, 0, 0));
        assert!(s.orders_by_status.is_empty());
        assert_eq!(s.operations_14d.len(), 14);
        assert!(s.operations_14d.iter().all(|d| d.count == 0));
    }

    #[test]
    fn counts_real_records() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.conn();
        let now = crate::util::now();
        conn.execute("INSERT INTO customers (display_name, created_at) VALUES ('C', ?1)", params![now]).unwrap();
        for (n, status) in [("OS-1", "open"), ("OS-2", "pending_verification"), ("OS-3", "completed")] {
            conn.execute(
                "INSERT INTO service_orders (number, customer_id, request_type, status, received_at, created_at, updated_at) \
                 VALUES (?1, 1, 'frp', ?2, ?3, ?3, ?3)",
                params![n, status, now],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO operations (started_at, operation, category, result) VALUES (?1, 'Read', 'diagnostics', 'success')",
            params![now],
        )
        .unwrap();
        let s = stats(&conn).unwrap();
        assert_eq!(s.orders_active, 2);
        assert_eq!(s.orders_pending_verification, 1);
        assert_eq!(s.orders_completed_30d, 1);
        assert_eq!(s.diagnostics_30d, 1);
        // An FRP diagnosis is one operation plus its detailed record: it must count once.
        conn.execute("INSERT INTO frp_assessments (created_at, observations_json, outcome) VALUES (?1, '{}', 'no_indication')", params![now]).unwrap();
        conn.execute("INSERT INTO operations (started_at, operation, category, result) VALUES (?1, 'Diagnóstico FRP', 'frp', 'success')", params![now]).unwrap();
        conn.execute("INSERT INTO operations (started_at, operation, category, result) VALUES (?1, 'Identificación del dispositivo', 'usb', 'success')", params![now]).unwrap();
        assert_eq!(stats(&conn).unwrap().diagnostics_30d, 2);
        assert_eq!(s.operations_14d.last().unwrap().count, 1);
    }
}
