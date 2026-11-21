//! Authorized factory reset.
//!
//! AndroidMasters never wipes a device itself. The owner (Settings) or the technician (Recovery) erases it
//! on the device, which is the manufacturer's official path. This service:
//! 1. checks authorization, the transfer checklist and account removal (preflight);
//! 2. requires an explicit and RECENT confirmation (single-use challenge, 5 minutes) naming the device;
//! 3. records the supporting actions it performs (open Settings, reboot into Recovery);
//! 4. records the post-reset verification (setup wizard shown, no previous account requested).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use rand::RngCore;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::services::adb::parsers::Mode;
use crate::services::audit::{self, Severity};
use crate::services::checks::{CheckItem, CheckStatus::*};
use crate::services::devices::{mode_label, DeviceView};
use crate::services::transfer;
use crate::util::now;

pub const CHALLENGE_TTL: Duration = Duration::from_secs(5 * 60);
/// Supporting actions must follow the confirmation closely.
pub const ACTION_WINDOW_MIN: i64 = 30;
pub const MIN_BATTERY: u8 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    Settings,
    Recovery,
}

impl Method {
    fn as_str(self) -> &'static str {
        match self {
            Method::Settings => "settings",
            Method::Recovery => "recovery",
        }
    }
    fn parse(s: &str) -> Method {
        if s == "recovery" { Method::Recovery } else { Method::Settings }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub at: String,
    pub kind: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Verification {
    pub at: String,
    pub setup_wizard_shown: bool,
    pub asks_previous_account: bool,
    pub disconnect_observed: Option<String>,
    pub note: String,
    pub verified: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResetRecord {
    pub id: i64,
    pub device_id: Option<i64>,
    pub device_label: String,
    pub checklist_id: Option<i64>,
    pub method: Method,
    pub status: String,
    pub confirmed_at: Option<String>,
    pub confirmed_by: Option<String>,
    pub owner_name: String,
    pub actions: Vec<Action>,
    pub verification: Option<Verification>,
    pub created_at: String,
    pub instructions: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Preflight {
    pub items: Vec<CheckItem>,
    pub ready: bool,
    pub checklist_id: Option<i64>,
}

/// Official on-device steps (instructions for the owner/technician; never executed by the app).
pub fn instructions(brand: Option<&str>, method: Method) -> Vec<String> {
    let mut steps = Vec::new();
    match method {
        Method::Settings => {
            let path = match brand {
                Some("samsung") => "Ajustes › Administración general › Restablecer › Restablecer valores de fábrica",
                Some("xiaomi") => "Ajustes › Sobre el teléfono › Restablecer datos de fábrica (según la versión: Ajustes adicionales › Copia y restablecimiento)",
                Some("huawei") => "Ajustes › Sistema y actualizaciones › Restablecer › Restablecer teléfono",
                Some("oppo") | Some("realme") => "Ajustes › Ajustes adicionales › Copia de seguridad y restablecimiento › Restablecer teléfono",
                Some("vivo") => "Ajustes › Sistema › Copia de seguridad y restablecimiento › Borrar todos los datos",
                _ => "Ajustes › Sistema › Opciones de restablecimiento › Borrar todos los datos (restablecer valores de fábrica)",
            };
            steps.push("El propietario desbloquea el teléfono con su propio bloqueo de pantalla.".to_string());
            steps.push(format!("Abre {path}. Si el menú no coincide, use el buscador de Ajustes con la palabra «restablecer»."));
            steps.push("Confirma el borrado en el teléfono e introduce su bloqueo de pantalla si se lo pide.".into());
            steps.push("Espera a que el teléfono se reinicie y muestre la pantalla de bienvenida.".into());
        }
        Method::Recovery => {
            steps.push("Con el teléfono en Recovery, use los botones de volumen para moverse y el botón de encendido para seleccionar.".into());
            steps.push("Elija «Wipe data/factory reset» (o «Borrar datos/restablecer») y confírmelo en el teléfono.".into());
            steps.push("Al terminar, elija «Reboot system now» (reiniciar el sistema).".into());
            steps.push("Los botones para entrar en Recovery varían según el modelo; si el teléfono no está conectado, consulte el procedimiento del fabricante.".into());
        }
    }
    steps.push("Después: compruebe que aparece el asistente de configuración y que no pide ninguna cuenta anterior.".into());
    steps
}

pub fn preflight(conn: &Connection, device_id: Option<i64>, device_label: &str, view: Option<&DeviceView>) -> AppResult<Preflight> {
    let mut items = Vec::new();
    match (view, device_id) {
        (Some(v), Some(_)) => items.push(CheckItem::new(
            "identity",
            "Equipo identificado",
            Pass,
            format!("{} {} · serie {}", v.manufacturer.clone().unwrap_or_default(), v.model.clone().unwrap_or_default(), v.serial_masked).trim().to_string(),
        )),
        (Some(v), None) => items.push(
            CheckItem::new("identity", "Equipo identificado", Fail, mode_label(v.mode))
                .advise("Autorice la depuración USB para identificar el equipo, o restablézcalo como equipo no conectado."),
        ),
        (None, Some(id)) => {
            let stored: Option<String> = conn
                .query_row("SELECT TRIM(manufacturer || ' ' || model || ' · serie ' || serial_masked) FROM devices WHERE id = ?1", params![id], |r| r.get(0))
                .optional()?;
            items.push(match stored {
                Some(label) => CheckItem::new("identity", "Equipo identificado", Pass, label),
                None => CheckItem::new("identity", "Equipo identificado", Fail, "Equipo no registrado"),
            });
        }
        (None, None) if !device_label.trim().is_empty() => items.push(
            CheckItem::new("identity", "Equipo identificado", Warn, format!("No conectado: «{}»", device_label.trim()))
                .advise("Confirme con el cliente la marca, el modelo y la serie que aparece en el teléfono."),
        ),
        _ => items.push(CheckItem::new("identity", "Equipo identificado", Fail, "Sin equipo").advise("Seleccione el equipo.")),
    }

    let checklist_id: Option<i64> = match device_id {
        Some(d) => conn.query_row("SELECT id FROM checklists WHERE device_id = ?1 ORDER BY id DESC LIMIT 1", params![d], |r| r.get(0)).optional()?,
        None => conn
            .query_row("SELECT id FROM checklists WHERE device_id IS NULL AND device_label = ?1 ORDER BY id DESC LIMIT 1", params![device_label.trim()], |r| r.get(0))
            .optional()?,
    };
    match checklist_id.map(|id| transfer::get(conn, id)).transpose()? {
        None => {
            items.push(CheckItem::new("authorization", "Autorización del propietario", Fail, "Sin lista de preparación")
                .advise("Abra la lista «Preparación para nuevo propietario» (Cuentas Google) y registre la autorización."));
            items.push(CheckItem::new("checklist", "Preparación para nuevo propietario", Fail, "No iniciada"));
        }
        Some(list) => {
            let auth = list.items.iter().any(|i| i.key == "authorization" && i.state.done);
            items.push(if auth {
                CheckItem::new("authorization", "Autorización del propietario", Pass, "Registrada en la lista de preparación")
            } else {
                CheckItem::new("authorization", "Autorización del propietario", Fail, "No registrada").advise("Sin autorización no se puede restablecer.")
            });
            let done = list.items.iter().filter(|i| i.state.done).count();
            items.push(if list.status == "complete" {
                CheckItem::new("checklist", "Preparación para nuevo propietario", Pass, format!("Completa ({done}/{})", list.items.len()))
            } else {
                CheckItem::new("checklist", "Preparación para nuevo propietario", Fail, format!("{done}/{} puntos", list.items.len()))
                    .advise("Complete la lista: copia de seguridad, retiro de cuentas, SIM y confirmación del borrado.")
            });
        }
    }

    if let Some(d) = device_id {
        match transfer::last_account_check(conn, d)? {
            Some((_, 0, at)) => items.push(CheckItem::new("accounts", "Cuentas retiradas", Pass, format!("Comprobado el {at}: sin cuentas con protección"))),
            Some((google, protected, at)) => items.push(
                CheckItem::new("accounts", "Cuentas retiradas", Fail, format!("Comprobación del {at}: {google} Google, {protected} con protección"))
                    .advise("El propietario debe quitar las cuentas desde Ajustes; después repita la comprobación. Si se borra con cuentas, el equipo pedirá la cuenta anterior (FRP)."),
            ),
            None => items.push(CheckItem::new("accounts", "Cuentas retiradas", Fail, "Sin comprobación de cuentas")
                .advise("Use «Comprobar cuentas en el equipo» en Cuentas Google.")),
        }
    }

    if let Some(v) = view {
        match v.battery.as_ref().and_then(|b| b.level) {
            Some(level) if level >= MIN_BATTERY => items.push(CheckItem::new("battery", "Batería", Pass, format!("{level}%"))),
            Some(level) => items.push(CheckItem::new("battery", "Batería", Warn, format!("{level}%")).advise(format!("Cargue al menos al {MIN_BATTERY}% antes de restablecer."))),
            None => items.push(CheckItem::new("battery", "Batería", Info, "No detectada")),
        }
        items.push(if v.mode == Mode::Adb {
            CheckItem::new("assist", "Ayuda desde la aplicación", Pass, "Puede abrir Ajustes o reiniciar en Recovery desde AndroidMasters")
        } else {
            CheckItem::new("assist", "Ayuda desde la aplicación", Info, mode_label(v.mode)).advise("Los pasos se harán solo en el teléfono.")
        });
    }
    items.push(CheckItem::new(
        "frp",
        "Protección FRP",
        Info,
        "El restablecimiento borra todos los datos del equipo. Si quedara alguna cuenta, el equipo la pedirá al configurarlo.",
    ));
    let ready = !items.iter().any(|i| i.status == Fail);
    Ok(Preflight { items, ready, checklist_id })
}

const COLUMNS: &str = "r.id, r.device_id, r.device_label, r.checklist_id, r.method, r.status, r.confirmed_at, u.display_name, r.owner_name, \
                       r.actions_json, r.verification_json, r.created_at, d.brand, d.manufacturer";

pub fn get(conn: &Connection, id: i64) -> AppResult<ResetRecord> {
    conn.query_row(
        &format!("SELECT {COLUMNS} FROM resets r LEFT JOIN users u ON u.id = r.confirmed_by LEFT JOIN devices d ON d.id = r.device_id WHERE r.id = ?1"),
        params![id],
        row_to_record,
    )
    .optional()?
    .ok_or_else(|| AppError::NotFound("Restablecimiento no encontrado.".into()))
}

fn row_to_record(r: &rusqlite::Row<'_>) -> rusqlite::Result<ResetRecord> {
    let method = Method::parse(&r.get::<_, String>(4)?);
    let actions: String = r.get(9)?;
    let verification: String = r.get(10)?;
    let brand: Option<String> = r.get(12)?;
    let manufacturer: Option<String> = r.get(13)?;
    let brand_key = crate::services::procedures::brand_for(manufacturer.as_deref(), brand.as_deref());
    Ok(ResetRecord {
        id: r.get(0)?,
        device_id: r.get(1)?,
        device_label: r.get(2)?,
        checklist_id: r.get(3)?,
        method,
        status: r.get(5)?,
        confirmed_at: r.get(6)?,
        confirmed_by: r.get(7)?,
        owner_name: r.get(8)?,
        actions: serde_json::from_str(&actions).unwrap_or_default(),
        verification: serde_json::from_str::<Verification>(&verification).ok(),
        created_at: r.get(11)?,
        instructions: instructions(brand_key, method),
    })
}

pub fn list(conn: &Connection, limit: i64) -> AppResult<Vec<ResetRecord>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM resets r LEFT JOIN users u ON u.id = r.confirmed_by LEFT JOIN devices d ON d.id = r.device_id ORDER BY r.id DESC LIMIT ?1"
    ))?;
    let rows = stmt.query_map(params![limit], row_to_record)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Creates a reset record. Refused unless every preflight check passes.
pub fn prepare(conn: &Connection, user_id: i64, device_id: Option<i64>, device_label: &str, view: Option<&DeviceView>, method: Method) -> AppResult<ResetRecord> {
    let pre = preflight(conn, device_id, device_label, view)?;
    if !pre.ready {
        let failed: Vec<String> = pre.items.iter().filter(|i| i.status == Fail).map(|i| i.title.clone()).collect();
        return Err(AppError::Validation(format!("No se puede preparar el restablecimiento: {}.", failed.join(", "))));
    }
    if let Some(checklist) = pre.checklist_id {
        let open: Option<i64> = conn
            .query_row(
                "SELECT id FROM resets WHERE checklist_id = ?1 AND status IN ('prepared','confirmed','pending_verification') LIMIT 1",
                [checklist],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = open {
            return Err(AppError::Validation(format!(
                "Ya hay un restablecimiento abierto para este equipo (n.º {id}). Continúelo o cancélelo en el historial."
            )));
        }
    }
    let ts = now();
    conn.execute(
        "INSERT INTO resets (device_id, device_label, checklist_id, method, status, created_at, updated_at, author_id) VALUES (?1, ?2, ?3, ?4, 'prepared', ?5, ?5, ?6)",
        params![device_id, device_label.trim(), pre.checklist_id, method.as_str(), ts, user_id],
    )?;
    get(conn, conn.last_insert_rowid())
}

/// Single-use, short-lived confirmation challenges (kept in memory: a restart invalidates them).
#[derive(Default)]
pub struct Challenges {
    inner: Mutex<HashMap<String, (i64, String, Instant)>>,
}

impl Challenges {
    pub fn issue(&self, reset_id: i64) -> (String, String) {
        let mut bytes = [0u8; 16];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        let token = hex::encode(bytes);
        let phrase = format!("BORRAR-{reset_id}");
        let mut map = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        map.retain(|_, (_, _, at)| at.elapsed() < CHALLENGE_TTL);
        map.insert(token.clone(), (reset_id, phrase.clone(), Instant::now()));
        (token, phrase)
    }

    /// Consumes the token. Returns the expected phrase when the token is valid for this reset.
    pub fn take(&self, token: &str, reset_id: i64) -> AppResult<String> {
        let mut map = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        match map.remove(token) {
            Some((id, phrase, at)) if id == reset_id && at.elapsed() < CHALLENGE_TTL => Ok(phrase),
            Some(_) => Err(AppError::Validation("La confirmación caducó. Vuelva a confirmar.".into())),
            None => Err(AppError::Validation("Confirmación no válida o ya usada. Vuelva a confirmar.".into())),
        }
    }
}

pub fn confirm(conn: &Connection, challenges: &Challenges, user_id: i64, reset_id: i64, token: &str, typed: &str, owner_name: &str) -> AppResult<ResetRecord> {
    let record = get(conn, reset_id)?;
    if record.status != "prepared" {
        return Err(AppError::Validation("Este restablecimiento ya no está pendiente de confirmación.".into()));
    }
    let phrase = challenges.take(token, reset_id)?;
    if typed.trim() != phrase {
        return Err(AppError::Validation("La frase de confirmación no coincide.".into()));
    }
    if owner_name.trim().len() < 3 {
        return Err(AppError::Validation("Escriba el nombre de la persona que autoriza el borrado.".into()));
    }
    let ts = now();
    conn.execute(
        "UPDATE resets SET status = 'confirmed', confirmed_at = ?1, confirmed_by = ?2, owner_name = ?3, updated_at = ?1 WHERE id = ?4",
        params![ts, user_id, owner_name.trim(), reset_id],
    )?;
    audit::record(conn, "reset_confirmed", Severity::Warning, Some(user_id), "", serde_json::json!({ "reset": reset_id, "method": record.method.as_str() }))?;
    get(conn, reset_id)
}

fn confirmed_recently(record: &ResetRecord) -> bool {
    record.confirmed_at.as_deref().and_then(|c| chrono::NaiveDateTime::parse_from_str(c, "%Y-%m-%dT%H:%M:%S").ok()).is_some_and(|c| {
        chrono::Local::now().naive_local() - c < chrono::Duration::minutes(ACTION_WINDOW_MIN)
    })
}

/// Records that the erase was started on the device (by the app's supporting action or by hand).
pub fn record_action(conn: &Connection, user_id: i64, reset_id: i64, kind: &str, ok: bool, detail: &str) -> AppResult<ResetRecord> {
    let record = get(conn, reset_id)?;
    if !matches!(record.status.as_str(), "confirmed" | "pending_verification") {
        return Err(AppError::Validation("Primero confirme el restablecimiento.".into()));
    }
    if !confirmed_recently(&record) {
        return Err(AppError::Validation(format!("La confirmación tiene más de {ACTION_WINDOW_MIN} minutos. Prepare y confirme de nuevo.")));
    }
    let mut actions = record.actions.clone();
    actions.push(Action { at: now(), kind: kind.to_string(), ok, detail: detail.to_string() });
    let status = if ok { "pending_verification" } else { record.status.as_str() };
    let ts = now();
    conn.execute(
        "UPDATE resets SET actions_json = ?1, status = ?2, updated_at = ?3 WHERE id = ?4",
        params![serde_json::to_string(&actions).unwrap_or_default(), status, ts, reset_id],
    )?;
    let operation = match kind {
        "open_settings" => "Restablecimiento: abrir Ajustes",
        "reboot_recovery" => "Restablecimiento: reinicio en Recovery",
        _ => "Restablecimiento: iniciado en el equipo",
    };
    conn.execute(
        "INSERT INTO operations (started_at, finished_at, device_id, user_id, operation, category, result, error_message, details_json) \
         VALUES (?1, ?1, ?2, ?3, ?4, 'reset', ?5, ?6, ?7)",
        params![ts, record.device_id, user_id, operation, if ok { "pending_verification" } else { "failed" }, if ok { "" } else { detail },
                serde_json::json!({ "reset": reset_id }).to_string()],
    )?;
    get(conn, reset_id)
}

pub fn verify(
    conn: &Connection,
    user_id: i64,
    reset_id: i64,
    setup_wizard_shown: bool,
    asks_previous_account: bool,
    note: &str,
    disconnect_observed: Option<String>,
) -> AppResult<ResetRecord> {
    let record = get(conn, reset_id)?;
    if record.status != "pending_verification" {
        return Err(AppError::Validation("Solo se verifica un restablecimiento iniciado en el equipo.".into()));
    }
    let (verified, message) = if !setup_wizard_shown {
        (false, "No se verificó: el equipo no muestra el asistente de configuración.".to_string())
    } else if asks_previous_account {
        (false, "El equipo pide una cuenta anterior (FRP): las cuentas no se retiraron. Siga el módulo Diagnóstico FRP.".to_string())
    } else {
        (true, "Verificado: el asistente de configuración aparece y no pide ninguna cuenta anterior.".to_string())
    };
    let v = Verification { at: now(), setup_wizard_shown, asks_previous_account, disconnect_observed, note: note.trim().to_string(), verified, message };
    conn.execute(
        "UPDATE resets SET verification_json = ?1, status = ?2, updated_at = ?3 WHERE id = ?4",
        params![serde_json::to_string(&v).unwrap_or_default(), if verified { "verified" } else { "failed" }, v.at, reset_id],
    )?;
    conn.execute(
        "INSERT INTO operations (started_at, finished_at, device_id, user_id, operation, category, result, error_message, details_json) \
         VALUES (?1, ?1, ?2, ?3, 'Verificación del restablecimiento', 'reset', ?4, ?5, ?6)",
        params![v.at, record.device_id, user_id, if verified { "success" } else { "failed" }, if verified { "" } else { v.message.as_str() },
                serde_json::json!({ "reset": reset_id }).to_string()],
    )?;
    audit::record(conn, if verified { "reset_verified" } else { "reset_failed" }, if verified { Severity::Info } else { Severity::Warning },
                  Some(user_id), "", serde_json::json!({ "reset": reset_id }))?;
    get(conn, reset_id)
}

pub fn cancel(conn: &Connection, user_id: i64, reset_id: i64) -> AppResult<ResetRecord> {
    let record = get(conn, reset_id)?;
    if !matches!(record.status.as_str(), "prepared" | "confirmed") {
        return Err(AppError::Validation("Solo se cancela antes de iniciar el borrado.".into()));
    }
    conn.execute("UPDATE resets SET status = 'cancelled', updated_at = ?1 WHERE id = ?2", params![now(), reset_id])?;
    audit::record(conn, "reset_cancelled", Severity::Info, Some(user_id), "", serde_json::json!({ "reset": reset_id }))?;
    get(conn, reset_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    use crate::services::transfer::ITEMS;

    fn ready_device(conn: &Connection) -> i64 {
        conn.execute("INSERT INTO users (username, display_name, role, password_hash, created_at) VALUES ('t','Tec','technician','h','t')", []).unwrap();
        conn.execute("INSERT INTO devices (fingerprint, serial_masked, serial_encrypted, brand, manufacturer, model, first_seen, last_seen) \
                      VALUES ('f','R58***DE','x','samsung','Samsung','SM-A145','t','t')", []).unwrap();
        conn.execute("INSERT INTO account_checks (device_id, created_at, summary_json, google_count, protected_count) VALUES (1, 't', '[]', 0, 0)", []).unwrap();
        let list = transfer::open(conn, 1, Some(1), "").unwrap();
        for d in ITEMS {
            transfer::set_item(conn, 1, list.id, d.key, true, "").unwrap();
        }
        1
    }

    #[test]
    fn preflight_blocks_without_authorization_checklist_and_account_removal() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.conn();
        conn.execute("INSERT INTO users (username, display_name, role, password_hash, created_at) VALUES ('t','Tec','technician','h','t')", []).unwrap();
        conn.execute("INSERT INTO devices (fingerprint, serial_masked, serial_encrypted, first_seen, last_seen) VALUES ('f','S','x','t','t')", []).unwrap();
        conn.execute("INSERT INTO account_checks (device_id, created_at, summary_json, google_count, protected_count) VALUES (1, 't', '[]', 1, 1)", []).unwrap();
        let pre = preflight(&conn, Some(1), "", None).unwrap();
        assert!(!pre.ready);
        let failed: Vec<&str> = pre.items.iter().filter(|i| i.status == Fail).map(|i| i.key).collect();
        assert_eq!(failed, vec!["authorization", "checklist", "accounts"]);
        assert!(prepare(&conn, 1, Some(1), "", None, Method::Settings).is_err());
    }

    #[test]
    fn full_flow_requires_fresh_confirmation_and_ends_with_verification() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.conn();
        let device = ready_device(&conn);
        assert!(preflight(&conn, Some(device), "", None).unwrap().ready);
        let r = prepare(&conn, 1, Some(device), "", None, Method::Settings).unwrap();
        assert_eq!(r.status, "prepared");
        assert!(r.instructions[1].contains("Administración general"), "Samsung path");
        assert!(prepare(&conn, 1, Some(device), "", None, Method::Recovery).is_err(), "one open reset per checklist");
        assert!(record_action(&conn, 1, r.id, "open_settings", true, "").is_err(), "no action before confirmation");

        let challenges = Challenges::default();
        let (token, phrase) = challenges.issue(r.id);
        assert!(confirm(&conn, &challenges, 1, r.id, &token, "borrar", "Ana Pérez").is_err());
        let (token, phrase2) = challenges.issue(r.id);
        assert_eq!(phrase, phrase2);
        assert!(confirm(&conn, &challenges, 1, r.id, &token, &phrase, "").is_err(), "needs the authorizing person");
        let (token, _) = challenges.issue(r.id);
        let r = confirm(&conn, &challenges, 1, r.id, &token, &phrase, "Ana Pérez").unwrap();
        assert_eq!(r.status, "confirmed");
        assert!(challenges.take(&token, r.id).is_err(), "tokens are single use");

        assert!(verify(&conn, 1, r.id, true, false, "", None).is_err(), "nothing to verify before the erase started");
        let r = record_action(&conn, 1, r.id, "open_settings", true, "").unwrap();
        assert_eq!(r.status, "pending_verification");
        let r = verify(&conn, 1, r.id, true, false, "Bienvenida visible", Some("2026-11-21T10:00:00".into())).unwrap();
        assert_eq!(r.status, "verified");
        assert!(r.verification.unwrap().verified);
        let events: i64 = conn.query_row("SELECT COUNT(*) FROM audit_log WHERE event IN ('reset_confirmed', 'reset_verified')", [], |x| x.get(0)).unwrap();
        assert_eq!(events, 2);
    }

    #[test]
    fn verification_fails_when_frp_appears_or_no_setup_wizard() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.conn();
        let device = ready_device(&conn);
        let challenges = Challenges::default();
        for (wizard, frp, expected) in [(true, true, "failed"), (false, false, "failed")] {
            let r = prepare(&conn, 1, Some(device), "", None, Method::Recovery).unwrap();
            let (t, p) = challenges.issue(r.id);
            confirm(&conn, &challenges, 1, r.id, &t, &p, "Ana Pérez").unwrap();
            record_action(&conn, 1, r.id, "manual_start", true, "").unwrap();
            let r = verify(&conn, 1, r.id, wizard, frp, "", None).unwrap();
            assert_eq!(r.status, expected);
            assert!(!r.verification.unwrap().verified);
        }
    }

    #[test]
    fn stale_confirmation_blocks_actions_and_cancel_rules() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.conn();
        let device = ready_device(&conn);
        let challenges = Challenges::default();
        let r = prepare(&conn, 1, Some(device), "", None, Method::Settings).unwrap();
        let (t, p) = challenges.issue(r.id);
        confirm(&conn, &challenges, 1, r.id, &t, &p, "Ana Pérez").unwrap();
        conn.execute("UPDATE resets SET confirmed_at = '2020-01-01T00:00:00' WHERE id = ?1", params![r.id]).unwrap();
        assert!(record_action(&conn, 1, r.id, "open_settings", true, "").unwrap_err().to_string().contains("minutos"));
        assert_eq!(cancel(&conn, 1, r.id).unwrap().status, "cancelled");
        assert!(cancel(&conn, 1, r.id).is_err());
        let bad = conn.execute("INSERT INTO resets (method, status, created_at, updated_at) VALUES ('settings', 'verified', 't', 't')", []);
        assert!(bad.is_err(), "the database refuses a verified reset without confirmation");
    }
}
