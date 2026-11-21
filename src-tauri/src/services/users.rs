//! Technician accounts with roles. First account must be an administrator; at least one active
//! administrator always remains. Repeated failed sign-ins lock the account temporarily.

use regex::Regex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

use crate::error::{AppError, AppResult};
use crate::security::passwords;
use crate::services::audit::{self, Severity};
use crate::util::now;

pub const MAX_FAILED_ATTEMPTS: i64 = 5;
pub const LOCK_MINUTES: i64 = 15;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Admin,
    Technician,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Admin => "admin",
            Role::Technician => "technician",
        }
    }
    fn parse(value: &str) -> AppResult<Self> {
        match value {
            "admin" => Ok(Role::Admin),
            "technician" => Ok(Role::Technician),
            other => Err(AppError::Internal(format!("Rol desconocido: {other}"))),
        }
    }
}

/// What each role may do. Technicians do service work; administrators also manage the workshop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    ManageUsers,
    ManageBackups,
    ReviewProcedures,
    ViewAudit,
    RevealSerial,
    CancelOrders,
    ManageSettings,
}

#[derive(Debug, Clone, Serialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub display_name: String,
    pub role: Role,
    pub active: bool,
    pub last_login: Option<String>,
}

impl User {
    pub fn can(&self, _permission: Permission) -> bool {
        self.active && self.role == Role::Admin
    }
}

fn username_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[a-z0-9._-]{3,32}$").expect("valid regex"))
}

fn validate_identity(username: &str, display_name: &str) -> AppResult<()> {
    if !username_re().is_match(username) {
        return Err(AppError::Validation(
            "Usuario: 3 a 32 caracteres en minúscula, números, punto, guion o guion bajo.".into(),
        ));
    }
    if display_name.trim().is_empty() || display_name.chars().count() > 80 {
        return Err(AppError::Validation("Escriba el nombre de la persona (máximo 80 caracteres).".into()));
    }
    Ok(())
}

const COLUMNS: &str = "id, username, display_name, role, active, last_login";

fn row_to_user(r: &rusqlite::Row<'_>) -> rusqlite::Result<(i64, String, String, String, bool, Option<String>)> {
    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get::<_, i64>(4)? == 1, r.get(5)?))
}

fn to_user(t: (i64, String, String, String, bool, Option<String>)) -> AppResult<User> {
    Ok(User { id: t.0, username: t.1, display_name: t.2, role: Role::parse(&t.3)?, active: t.4, last_login: t.5 })
}

pub fn has_users(conn: &Connection) -> AppResult<bool> {
    Ok(conn.query_row("SELECT COUNT(*) FROM users", [], |r| r.get::<_, i64>(0))? > 0)
}

pub fn get(conn: &Connection, id: i64) -> AppResult<User> {
    let row = conn
        .query_row(&format!("SELECT {COLUMNS} FROM users WHERE id = ?1"), params![id], row_to_user)
        .optional()?
        .ok_or_else(|| AppError::NotFound("Usuario no encontrado.".into()))?;
    to_user(row)
}

pub fn list(conn: &Connection) -> AppResult<Vec<User>> {
    let mut stmt = conn.prepare(&format!("SELECT {COLUMNS} FROM users ORDER BY active DESC, display_name"))?;
    let rows = stmt.query_map([], row_to_user)?.collect::<Result<Vec<_>, _>>()?;
    rows.into_iter().map(to_user).collect()
}

fn active_admins(conn: &Connection) -> AppResult<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM users WHERE role = 'admin' AND active = 1", [], |r| r.get(0))?)
}

/// Creates a user. With no users yet, only an administrator can be created (first-run setup).
pub fn create(
    conn: &Connection,
    actor: Option<&User>,
    username: &str,
    display_name: &str,
    password: &str,
    role: Role,
) -> AppResult<User> {
    let first = !has_users(conn)?;
    match (first, actor) {
        (true, _) if role != Role::Admin => return Err(AppError::Validation("La primera cuenta debe ser de administrador.".into())),
        (false, None) => return Err(AppError::Forbidden("Inicie sesión como administrador.".into())),
        (false, Some(a)) if !a.can(Permission::ManageUsers) => {
            return Err(AppError::Forbidden("Solo un administrador puede crear cuentas.".into()))
        }
        _ => {}
    }
    let username = username.trim().to_lowercase();
    validate_identity(&username, display_name)?;
    let exists: Option<i64> = conn.query_row("SELECT id FROM users WHERE username = ?1", params![username], |r| r.get(0)).optional()?;
    if exists.is_some() {
        return Err(AppError::Validation("Ese nombre de usuario ya existe.".into()));
    }
    let hash = passwords::hash(password)?;
    conn.execute(
        "INSERT INTO users (username, display_name, role, password_hash, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![username, display_name.trim(), role.as_str(), hash, now()],
    )?;
    let id = conn.last_insert_rowid();
    audit::record(
        conn,
        "user_created",
        Severity::Warning,
        actor.map(|a| a.id).or(Some(id)),
        "",
        serde_json::json!({ "username": username, "role": role.as_str(), "first_run": first }),
    )?;
    get(conn, id)
}

pub fn authenticate(conn: &Connection, username: &str, password: &str) -> AppResult<User> {
    let username = username.trim().to_lowercase();
    let generic = || AppError::Validation("Usuario o contraseña incorrectos.".into());
    let row: Option<(i64, String, i64, i64, Option<String>)> = conn
        .query_row(
            "SELECT id, password_hash, active, failed_attempts, locked_until FROM users WHERE username = ?1",
            params![username],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .optional()?;
    let Some((id, hash, active, failed, locked_until)) = row else {
        // Same work as a real check so timing does not reveal which users exist.
        let _ = passwords::verify(password, "$argon2id$v=19$m=19456,t=2,p=1$c29tZXNhbHQ$aGFzaGhhc2hoYXNoaGFzaA");
        audit::record(conn, "login_failed", Severity::Warning, None, "", serde_json::json!({ "username": username }))?;
        return Err(generic());
    };
    let now_ts = chrono::Local::now().naive_local();
    if let Some(until) = locked_until.as_deref().and_then(|u| chrono::NaiveDateTime::parse_from_str(u, "%Y-%m-%dT%H:%M:%S").ok()) {
        if until > now_ts {
            return Err(AppError::Forbidden("Demasiados intentos fallidos. La cuenta está bloqueada unos minutos.".into()));
        }
    }
    if active != 1 {
        return Err(AppError::Forbidden("Esta cuenta está desactivada. Consulte a un administrador.".into()));
    }
    if !passwords::verify(password, &hash) {
        let failed = failed + 1;
        let lock = (failed >= MAX_FAILED_ATTEMPTS)
            .then(|| (now_ts + chrono::Duration::minutes(LOCK_MINUTES)).format("%Y-%m-%dT%H:%M:%S").to_string());
        conn.execute(
            "UPDATE users SET failed_attempts = ?1, locked_until = ?2 WHERE id = ?3",
            params![if lock.is_some() { 0 } else { failed }, lock, id],
        )?;
        audit::record(conn, "login_failed", Severity::Warning, Some(id), "", serde_json::json!({ "locked": lock.is_some() }))?;
        return Err(generic());
    }
    record_login(conn, id)?;
    get(conn, id)
}

/// Marks a successful sign-in (also used right after first-run setup signs the administrator in).
pub fn record_login(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute("UPDATE users SET failed_attempts = 0, locked_until = NULL, last_login = ?1 WHERE id = ?2", params![now(), id])?;
    audit::record(conn, "login", Severity::Info, Some(id), "", serde_json::json!({}))
}

fn require_admin(actor: &User) -> AppResult<()> {
    if actor.can(Permission::ManageUsers) {
        Ok(())
    } else {
        Err(AppError::Forbidden("Solo un administrador puede gestionar cuentas.".into()))
    }
}

pub fn set_active(conn: &Connection, actor: &User, id: i64, active: bool) -> AppResult<User> {
    require_admin(actor)?;
    let target = get(conn, id)?;
    if !active && target.id == actor.id {
        return Err(AppError::Validation("No puede desactivar su propia cuenta.".into()));
    }
    if !active && target.role == Role::Admin && target.active && active_admins(conn)? <= 1 {
        return Err(AppError::Validation("Debe quedar al menos un administrador activo.".into()));
    }
    conn.execute("UPDATE users SET active = ?1 WHERE id = ?2", params![active as i64, id])?;
    audit::record(conn, if active { "user_enabled" } else { "user_disabled" }, Severity::Warning, Some(actor.id), "",
                  serde_json::json!({ "username": target.username }))?;
    get(conn, id)
}

pub fn set_role(conn: &Connection, actor: &User, id: i64, role: Role) -> AppResult<User> {
    require_admin(actor)?;
    let target = get(conn, id)?;
    if target.role == Role::Admin && role != Role::Admin && target.active && active_admins(conn)? <= 1 {
        return Err(AppError::Validation("Debe quedar al menos un administrador activo.".into()));
    }
    conn.execute("UPDATE users SET role = ?1 WHERE id = ?2", params![role.as_str(), id])?;
    audit::record(conn, "user_role_changed", Severity::Warning, Some(actor.id), "",
                  serde_json::json!({ "username": target.username, "role": role.as_str() }))?;
    get(conn, id)
}

pub fn reset_password(conn: &Connection, actor: &User, id: i64, new_password: &str) -> AppResult<()> {
    require_admin(actor)?;
    let target = get(conn, id)?;
    let hash = passwords::hash(new_password)?;
    conn.execute("UPDATE users SET password_hash = ?1, failed_attempts = 0, locked_until = NULL WHERE id = ?2", params![hash, id])?;
    audit::record(conn, "user_password_reset", Severity::Warning, Some(actor.id), "",
                  serde_json::json!({ "username": target.username }))?;
    Ok(())
}

pub fn change_own_password(conn: &Connection, actor: &User, current: &str, new_password: &str) -> AppResult<()> {
    let hash: String = conn.query_row("SELECT password_hash FROM users WHERE id = ?1", params![actor.id], |r| r.get(0))?;
    if !passwords::verify(current, &hash) {
        return Err(AppError::Validation("La contraseña actual no es correcta.".into()));
    }
    let new_hash = passwords::hash(new_password)?;
    conn.execute("UPDATE users SET password_hash = ?1 WHERE id = ?2", params![new_hash, actor.id])?;
    audit::record(conn, "password_changed", Severity::Info, Some(actor.id), "", serde_json::json!({}))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;

    const PW: &str = "clave-segura-1";

    #[test]
    fn first_user_must_be_admin_then_only_admins_create() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.conn();
        assert!(create(&conn, None, "ana", "Ana", PW, Role::Technician).is_err());
        let admin = create(&conn, None, "Ana", "Ana Pérez", PW, Role::Admin).unwrap();
        assert_eq!(admin.username, "ana");
        assert!(create(&conn, None, "luis", "Luis", PW, Role::Technician).is_err(), "needs a signed-in admin");
        let tech = create(&conn, Some(&admin), "luis", "Luis", PW, Role::Technician).unwrap();
        assert!(create(&conn, Some(&tech), "eva", "Eva", PW, Role::Technician).is_err(), "technicians cannot create users");
        assert!(create(&conn, Some(&admin), "luis", "Otro", PW, Role::Technician).is_err(), "duplicate username");
        assert!(create(&conn, Some(&admin), "x", "X", PW, Role::Technician).is_err(), "invalid username");
    }

    #[test]
    fn login_lockout_and_generic_errors() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.conn();
        create(&conn, None, "ana", "Ana", PW, Role::Admin).unwrap();
        let unknown = authenticate(&conn, "nadie", PW).unwrap_err().to_string();
        let wrong = authenticate(&conn, "ana", "incorrecta-123").unwrap_err().to_string();
        assert_eq!(unknown, wrong, "message must not reveal whether the user exists");
        for _ in 0..MAX_FAILED_ATTEMPTS - 1 {
            let _ = authenticate(&conn, "ana", "incorrecta-123");
        }
        assert!(matches!(authenticate(&conn, "ana", PW), Err(AppError::Forbidden(_))), "locked after 5 failures");
        let failures: i64 = conn.query_row("SELECT COUNT(*) FROM audit_log WHERE event = 'login_failed'", [], |r| r.get(0)).unwrap();
        assert_eq!(failures, 6);
    }

    #[test]
    fn last_active_admin_is_protected() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.conn();
        let admin = create(&conn, None, "ana", "Ana", PW, Role::Admin).unwrap();
        let tech = create(&conn, Some(&admin), "luis", "Luis", PW, Role::Technician).unwrap();
        assert!(set_active(&conn, &admin, admin.id, false).is_err(), "cannot disable yourself");
        assert!(set_role(&conn, &admin, admin.id, Role::Technician).is_err(), "last admin keeps the role");
        assert!(set_active(&conn, &tech, admin.id, false).is_err(), "technician cannot manage users");
        let disabled = set_active(&conn, &admin, tech.id, false).unwrap();
        assert!(!disabled.active);
        assert!(authenticate(&conn, "luis", PW).is_err());
    }

    #[test]
    fn record_login_sets_last_login_and_audits() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.conn();
        let admin = create(&conn, None, "ana", "Ana", PW, Role::Admin).unwrap();
        assert!(admin.last_login.is_none());
        record_login(&conn, admin.id).unwrap();
        assert!(get(&conn, admin.id).unwrap().last_login.is_some());
        let logins: i64 = conn.query_row("SELECT COUNT(*) FROM audit_log WHERE event = 'login'", [], |r| r.get(0)).unwrap();
        assert_eq!(logins, 1);
    }

    #[test]
    fn passwords_change_and_reset() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.conn();
        let admin = create(&conn, None, "ana", "Ana", PW, Role::Admin).unwrap();
        assert!(change_own_password(&conn, &admin, "mala-clave-99", "nueva-clave-22").is_err());
        change_own_password(&conn, &admin, PW, "nueva-clave-22").unwrap();
        assert!(authenticate(&conn, "ana", "nueva-clave-22").is_ok());
        let tech = create(&conn, Some(&admin), "luis", "Luis", PW, Role::Technician).unwrap();
        reset_password(&conn, &admin, tech.id, "temporal-clave-1").unwrap();
        assert!(authenticate(&conn, "luis", "temporal-clave-1").is_ok());
    }
}
