//! Checklist to prepare a device for a new owner. Items about accounts can only be ticked with
//! evidence: a recent account check showing no protected accounts, or a note describing what the
//! technician verified on screen when the device cannot be connected.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::services::audit::{self, Severity};
use crate::util::now;

pub struct ItemDef {
    pub key: &'static str,
    pub label: &'static str,
    /// Needs evidence from an account check (or a written on-screen verification).
    pub needs_account_evidence: bool,
}

pub const ITEMS: &[ItemDef] = &[
    ItemDef { key: "authorization", label: "Autorización del propietario verificada y registrada", needs_account_evidence: false },
    ItemDef { key: "backup", label: "El propietario hizo su copia de seguridad y la sincronización terminó", needs_account_evidence: false },
    ItemDef { key: "google_removed", label: "Cuentas Google retiradas desde Ajustes", needs_account_evidence: true },
    ItemDef { key: "brand_removed", label: "Cuentas del fabricante cerradas y búsqueda del dispositivo desactivada", needs_account_evidence: true },
    ItemDef { key: "sim_removed", label: "Tarjeta SIM retirada y eSIM eliminada (si aplica)", needs_account_evidence: false },
    ItemDef { key: "accessories", label: "Relojes y accesorios desvinculados", needs_account_evidence: false },
    ItemDef { key: "erase_confirmed", label: "El propietario confirmó expresamente el borrado de todos los datos", needs_account_evidence: false },
];

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ItemState {
    pub done: bool,
    pub note: String,
    pub evidence: String,
    pub by: Option<i64>,
    pub at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChecklistItem {
    pub key: &'static str,
    pub label: &'static str,
    pub needs_account_evidence: bool,
    pub state: ItemState,
}

#[derive(Debug, Clone, Serialize)]
pub struct Checklist {
    pub id: i64,
    pub device_id: Option<i64>,
    pub device_label: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub items: Vec<ChecklistItem>,
}

fn load_states(json: &str) -> std::collections::BTreeMap<String, ItemState> {
    serde_json::from_str(json).unwrap_or_default()
}

pub fn get(conn: &Connection, id: i64) -> AppResult<Checklist> {
    let row: (i64, Option<i64>, String, String, String, String, String) = conn
        .query_row(
            "SELECT id, device_id, device_label, status, created_at, updated_at, items_json FROM checklists WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound("Lista no encontrada.".into()))?;
    let states = load_states(&row.6);
    Ok(Checklist {
        id: row.0,
        device_id: row.1,
        device_label: row.2,
        status: row.3,
        created_at: row.4,
        updated_at: row.5,
        items: ITEMS
            .iter()
            .map(|d| ChecklistItem { key: d.key, label: d.label, needs_account_evidence: d.needs_account_evidence, state: states.get(d.key).cloned().unwrap_or_default() })
            .collect(),
    })
}

/// Latest open checklist for the device (or label), creating one if none is in progress.
pub fn open(conn: &Connection, user_id: i64, device_id: Option<i64>, device_label: &str) -> AppResult<Checklist> {
    if device_id.is_none() && device_label.trim().is_empty() {
        return Err(AppError::Validation("Indique el dispositivo.".into()));
    }
    let existing: Option<i64> = match device_id {
        Some(d) => conn.query_row("SELECT id FROM checklists WHERE device_id = ?1 AND status = 'in_progress' ORDER BY id DESC LIMIT 1", params![d], |r| r.get(0)).optional()?,
        None => conn
            .query_row(
                "SELECT id FROM checklists WHERE device_id IS NULL AND device_label = ?1 AND status = 'in_progress' ORDER BY id DESC LIMIT 1",
                params![device_label.trim()],
                |r| r.get(0),
            )
            .optional()?,
    };
    if let Some(id) = existing {
        return get(conn, id);
    }
    let ts = now();
    conn.execute(
        "INSERT INTO checklists (kind, device_id, device_label, status, items_json, created_at, updated_at, author_id) VALUES ('transfer', ?1, ?2, 'in_progress', '{}', ?3, ?3, ?4)",
        params![device_id, device_label.trim(), ts, user_id],
    )?;
    get(conn, conn.last_insert_rowid())
}

/// Last account check for a device: (google_count, protected_count, created_at).
pub fn last_account_check(conn: &Connection, device_id: i64) -> AppResult<Option<(i64, i64, String)>> {
    Ok(conn
        .query_row(
            "SELECT google_count, protected_count, created_at FROM account_checks WHERE device_id = ?1 ORDER BY id DESC LIMIT 1",
            params![device_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?)
}

pub fn set_item(conn: &Connection, user_id: i64, id: i64, key: &str, done: bool, note: &str) -> AppResult<Checklist> {
    let list = get(conn, id)?;
    if list.status == "complete" {
        return Err(AppError::Validation("Esta lista ya está completa.".into()));
    }
    let def = ITEMS.iter().find(|d| d.key == key).ok_or_else(|| AppError::NotFound("Elemento desconocido.".into()))?;
    let mut evidence = String::new();
    if done && def.needs_account_evidence {
        let check = match list.device_id {
            Some(device) => last_account_check(conn, device)?,
            None => None,
        };
        match check {
            Some((google, protected, at)) => {
                let ok = if key == "google_removed" { google == 0 } else { protected - google == 0 };
                if !ok {
                    return Err(AppError::Validation(
                        "La última comprobación todavía muestra cuentas en el equipo. El propietario debe retirarlas y debe repetirse la comprobación.".into(),
                    ));
                }
                evidence = format!("Comprobación de cuentas del {at}: sin cuentas con protección");
            }
            None if note.trim().len() >= 10 => evidence = format!("Verificado en pantalla por el técnico: {}", note.trim()),
            None => {
                return Err(AppError::Validation(
                    "Conecte el equipo y use «Comprobar cuentas en el equipo», o describa lo que verificó en la pantalla del teléfono.".into(),
                ))
            }
        }
    }
    let raw: String = conn.query_row("SELECT items_json FROM checklists WHERE id = ?1", params![id], |r| r.get(0))?;
    let mut states = load_states(&raw);
    states.insert(
        key.to_string(),
        ItemState { done, note: note.trim().to_string(), evidence, by: Some(user_id), at: Some(now()) },
    );
    let complete = ITEMS.iter().all(|d| states.get(d.key).is_some_and(|s| s.done));
    conn.execute(
        "UPDATE checklists SET items_json = ?1, status = ?2, updated_at = ?3 WHERE id = ?4",
        params![serde_json::to_string(&states).map_err(|e| AppError::Internal(e.to_string()))?, if complete { "complete" } else { "in_progress" }, now(), id],
    )?;
    if complete {
        audit::record(conn, "transfer_checklist_completed", Severity::Info, Some(user_id), "", serde_json::json!({ "checklist": id }))?;
    }
    get(conn, id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;

    fn setup(conn: &Connection) {
        conn.execute("INSERT INTO users (username, display_name, role, password_hash, created_at) VALUES ('t','Tec','technician','h','t')", []).unwrap();
        conn.execute("INSERT INTO devices (fingerprint, serial_masked, serial_encrypted, first_seen, last_seen) VALUES ('f','S**1','x','t','t')", []).unwrap();
    }

    #[test]
    fn account_items_require_evidence() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.conn();
        setup(&conn);
        let list = open(&conn, 1, Some(1), "").unwrap();
        assert_eq!(open(&conn, 1, Some(1), "").unwrap().id, list.id, "reuses the open list");
        assert!(set_item(&conn, 1, list.id, "google_removed", true, "").is_err(), "no evidence yet");
        conn.execute("INSERT INTO account_checks (device_id, created_at, summary_json, google_count, protected_count) VALUES (1, 't1', '[]', 1, 1)", []).unwrap();
        assert!(set_item(&conn, 1, list.id, "google_removed", true, "").is_err(), "the check still shows a Google account");
        conn.execute("INSERT INTO account_checks (device_id, created_at, summary_json, google_count, protected_count) VALUES (1, 't2', '[]', 0, 0)", []).unwrap();
        let l = set_item(&conn, 1, list.id, "google_removed", true, "").unwrap();
        let item = l.items.iter().find(|i| i.key == "google_removed").unwrap();
        assert!(item.state.done && item.state.evidence.contains("t2"));
    }

    #[test]
    fn manual_device_needs_a_written_verification_and_list_completes() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.conn();
        setup(&conn);
        let list = open(&conn, 1, None, "Samsung A14").unwrap();
        assert!(set_item(&conn, 1, list.id, "google_removed", true, "ok").is_err(), "a short note is not evidence");
        let mut l = list;
        for d in ITEMS {
            l = set_item(&conn, 1, l.id, d.key, true, "Pantalla Cuentas sin cuentas, comprobado con el cliente").unwrap();
        }
        assert_eq!(l.status, "complete");
        assert!(set_item(&conn, 1, l.id, "backup", false, "").is_err(), "a complete list is frozen");
        assert!(open(&conn, 1, None, " ").is_err());
    }
}
