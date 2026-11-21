//! Service orders (SKILL.md, «Ficha de servicio y trazabilidad»).
//!
//! Every entry says what it is: **diagnosis** (what the device reported), **instruction** (steps for the owner),
//! **action** (an operation the application actually performed), **verification** (evidence afterwards),
//! plus consent, incidents and notes. Technicians can write diagnoses, instructions, verifications, incidents and
//! notes; executed actions and consents only come from the application's own records (`link`), so a guide can
//! never be presented as a completed operation.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::services::audit::{self, Severity};
use crate::util::now;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestType {
    Diagnose,
    Accounts,
    Frp,
    Reset,
    Transfer,
    Maintenance,
}

impl RequestType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Diagnose => "diagnose",
            Self::Accounts => "accounts",
            Self::Frp => "frp",
            Self::Reset => "reset",
            Self::Transfer => "transfer",
            Self::Maintenance => "maintenance",
        }
    }
    pub fn label(s: &str) -> &'static str {
        match s {
            "diagnose" => "Diagnóstico",
            "accounts" => "Gestión de cuentas",
            "frp" => "Revisión FRP",
            "reset" => "Restablecimiento",
            "transfer" => "Preparación para transferencia",
            "maintenance" => "Mantenimiento",
            _ => "Otro",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    Open,
    InProgress,
    PendingVerification,
    Completed,
    Delivered,
    Cancelled,
}

impl OrderStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::InProgress => "in_progress",
            Self::PendingVerification => "pending_verification",
            Self::Completed => "completed",
            Self::Delivered => "delivered",
            Self::Cancelled => "cancelled",
        }
    }
    pub fn parse(s: &str) -> AppResult<Self> {
        Ok(match s {
            "open" => Self::Open,
            "in_progress" => Self::InProgress,
            "pending_verification" => Self::PendingVerification,
            "completed" => Self::Completed,
            "delivered" => Self::Delivered,
            "cancelled" => Self::Cancelled,
            _ => return Err(AppError::Internal(format!("estado de orden desconocido: {s}"))),
        })
    }
    pub fn label(s: &str) -> &'static str {
        match s {
            "open" => "Abierta",
            "in_progress" => "En curso",
            "pending_verification" => "Pendiente de verificación",
            "completed" => "Completada",
            "delivered" => "Entregada",
            "cancelled" => "Cancelada",
            _ => "—",
        }
    }
    fn closed(self) -> bool {
        matches!(self, Self::Delivered | Self::Cancelled)
    }
    /// Transitions a technician may request from this status.
    pub fn next(self) -> &'static [OrderStatus] {
        use OrderStatus::*;
        match self {
            Open => &[InProgress, Completed, Cancelled],
            InProgress => &[PendingVerification, Completed, Cancelled],
            PendingVerification => &[InProgress, Completed, Cancelled],
            Completed => &[Delivered, InProgress],
            Delivered | Cancelled => &[],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    Diagnosis,
    Instruction,
    Action,
    Verification,
    Consent,
    Incident,
    Note,
}

impl EntryKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Diagnosis => "diagnosis",
            Self::Instruction => "instruction",
            Self::Action => "action",
            Self::Verification => "verification",
            Self::Consent => "consent",
            Self::Incident => "incident",
            Self::Note => "note",
        }
    }
    pub fn label(s: &str) -> &'static str {
        match s {
            "diagnosis" => "Diagnóstico",
            "instruction" => "Instrucción",
            "action" => "Acción ejecutada",
            "verification" => "Verificación",
            "consent" => "Consentimiento",
            "incident" => "Incidencia",
            "note" => "Nota",
            _ => "—",
        }
    }
    /// Statuses allowed for an entry of this kind.
    fn allows(self, status: EntryStatus) -> bool {
        use EntryStatus::*;
        match self {
            Self::Diagnosis => matches!(status, Info | Ok | Failed),
            // An instruction is guidance: it is never "done" (also enforced by a CHECK in the database).
            Self::Instruction => matches!(status, Info | PendingVerification),
            Self::Action => matches!(status, Ok | Failed | PendingVerification),
            Self::Verification => matches!(status, Ok | Failed),
            Self::Consent => matches!(status, Ok | Failed | Info),
            Self::Incident => matches!(status, Failed | Info),
            Self::Note => matches!(status, Info),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryStatus {
    Ok,
    Failed,
    PendingVerification,
    Info,
}

impl EntryStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Failed => "failed",
            Self::PendingVerification => "pending_verification",
            Self::Info => "info",
        }
    }
    pub fn label(s: &str) -> &'static str {
        match s {
            "ok" => "Correcto",
            "failed" => "Fallido",
            "pending_verification" => "Pendiente de verificación",
            "info" => "Informativo",
            _ => "—",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnershipMethod {
    OwnerPresentId,
    PurchaseProof,
    AuthorizedRepresentative,
    CompanyDevice,
}

impl OwnershipMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OwnerPresentId => "owner_present_id",
            Self::PurchaseProof => "purchase_proof",
            Self::AuthorizedRepresentative => "authorized_representative",
            Self::CompanyDevice => "company_device",
        }
    }
    pub fn label(s: &str) -> &'static str {
        match s {
            "owner_present_id" => "Propietario presente con identificación",
            "purchase_proof" => "Factura o prueba de compra a su nombre",
            "authorized_representative" => "Representante con autorización escrita",
            "company_device" => "Equipo de empresa: responsable autorizado",
            _ => "",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsentDecision {
    Granted,
    Denied,
    Revoked,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewOrder {
    pub customer_name: String,
    #[serde(default)]
    pub customer_contact: String,
    #[serde(default)]
    pub device_label: String,
    pub request_type: RequestType,
    #[serde(default)]
    pub reported_problem: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OrderSummary {
    pub id: i64,
    pub number: String,
    pub customer: String,
    pub device: String,
    pub request_type: String,
    pub status: String,
    pub consent_status: String,
    pub technician: Option<String>,
    pub received_at: String,
    pub updated_at: String,
    pub pending_verification: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    pub id: i64,
    pub created_at: String,
    pub kind: String,
    pub title: String,
    pub detail: String,
    pub status: String,
    pub author: Option<String>,
    pub source: String,
    pub verifies_entry_id: Option<i64>,
    /// For entries pending verification: the verification entry that closed it, if any.
    pub verified_by_entry_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeviceInfo {
    pub manufacturer: String,
    pub model: String,
    pub serial_masked: String,
    pub android_version: String,
    pub api_level: Option<i64>,
    pub security_patch: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Order {
    pub id: i64,
    pub number: String,
    pub customer_id: i64,
    pub customer: String,
    pub customer_contact: String,
    pub device_id: Option<i64>,
    pub device_label: String,
    pub device: Option<DeviceInfo>,
    pub request_type: String,
    pub reported_problem: String,
    pub status: String,
    pub consent_status: String,
    pub consent_by: String,
    pub consent_at: Option<String>,
    pub ownership_method: String,
    pub technician: Option<String>,
    pub received_at: String,
    pub delivered_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub notes: String,
    pub entries: Vec<Entry>,
    pub next_statuses: Vec<OrderStatus>,
}

impl Order {
    /// Display name of the device: identified data first, then the label typed by the technician.
    pub fn device_name(&self) -> String {
        match &self.device {
            Some(d) => {
                let name = format!("{} {}", d.manufacturer, d.model).trim().to_string();
                if name.is_empty() { self.device_label.clone() } else { name }
            }
            None => self.device_label.clone(),
        }
    }
    /// Entries still waiting for evidence.
    pub fn unverified(&self) -> Vec<&Entry> {
        self.entries.iter().filter(|e| e.status == "pending_verification" && e.verified_by_entry_id.is_none()).collect()
    }
}

const MAX_TEXT: usize = 4000;

fn clean(value: &str, field: &str, max: usize) -> AppResult<String> {
    let v = value.trim();
    if v.chars().count() > max {
        return Err(AppError::Validation(format!("«{field}» es demasiado largo (máximo {max} caracteres).")));
    }
    Ok(v.to_string())
}

fn next_number(conn: &Connection) -> AppResult<String> {
    let year = chrono::Local::now().format("%Y").to_string();
    let prefix = format!("OS-{year}-");
    let last: Option<String> = conn
        .query_row("SELECT number FROM service_orders WHERE number LIKE ?1 ORDER BY number DESC LIMIT 1", [format!("{prefix}%")], |r| r.get(0))
        .optional()?;
    let n = last.and_then(|s| s[prefix.len()..].parse::<u32>().ok()).unwrap_or(0) + 1;
    Ok(format!("{prefix}{n:04}"))
}

#[allow(clippy::too_many_arguments)]
fn insert_entry(
    conn: &Connection,
    order_id: i64,
    user_id: Option<i64>,
    kind: EntryKind,
    title: &str,
    detail: &str,
    status: EntryStatus,
    source: &str,
    verifies: Option<i64>,
) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO order_entries (order_id, created_at, kind, title, detail, status, author_id, verifies_entry_id, source) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![order_id, now(), kind.as_str(), title, detail, status.as_str(), user_id, verifies, source],
    )?;
    conn.execute("UPDATE service_orders SET updated_at = ?1 WHERE id = ?2", params![now(), order_id])?;
    Ok(conn.last_insert_rowid())
}

pub fn create(conn: &Connection, user_id: i64, device_id: Option<i64>, new: &NewOrder) -> AppResult<Order> {
    let customer = clean(&new.customer_name, "Cliente", 120)?;
    if customer.is_empty() {
        return Err(AppError::Validation("Escriba el nombre del cliente.".into()));
    }
    let contact = clean(&new.customer_contact, "Contacto", 120)?;
    let label = clean(&new.device_label, "Marca y modelo", 120)?;
    if device_id.is_none() && label.is_empty() {
        return Err(AppError::Validation("Seleccione el equipo conectado o escriba su marca y modelo.".into()));
    }
    let problem = clean(&new.reported_problem, "Problema", MAX_TEXT)?;
    let ts = now();
    conn.execute("INSERT INTO customers (display_name, contact, created_at) VALUES (?1, ?2, ?3)", params![customer, contact, ts])?;
    let customer_id = conn.last_insert_rowid();
    let number = next_number(conn)?;
    conn.execute(
        "INSERT INTO service_orders (number, customer_id, device_id, device_label, request_type, reported_problem, status, technician_id, received_at, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'open', ?7, ?8, ?8, ?8)",
        params![number, customer_id, device_id, label, new.request_type.as_str(), problem, user_id, ts],
    )?;
    let id = conn.last_insert_rowid();
    insert_entry(conn, id, Some(user_id), EntryKind::Note, "Orden creada", &problem, EntryStatus::Info, "", None)?;
    if let Some(info) = device_info(conn, device_id)? {
        let detail = format!(
            "{} {} · Android {} · API {} · parche {} · serie {}",
            info.manufacturer,
            info.model,
            or_dash(&info.android_version),
            info.api_level.map(|v| v.to_string()).unwrap_or_else(|| "—".into()),
            or_dash(&info.security_patch),
            info.serial_masked
        );
        insert_entry(conn, id, Some(user_id), EntryKind::Diagnosis, "Equipo identificado por USB", &detail, EntryStatus::Info, "device", None)?;
    }
    audit::record(conn, "order_created", Severity::Info, Some(user_id), "", serde_json::json!({ "order": number }))?;
    get(conn, id)
}

fn or_dash(s: &str) -> &str {
    if s.trim().is_empty() { "—" } else { s }
}

fn device_info(conn: &Connection, device_id: Option<i64>) -> AppResult<Option<DeviceInfo>> {
    let Some(id) = device_id else { return Ok(None) };
    Ok(conn
        .query_row(
            "SELECT manufacturer, model, serial_masked, android_version, api_level, security_patch FROM devices WHERE id = ?1",
            [id],
            |r| {
                Ok(DeviceInfo {
                    manufacturer: r.get(0)?,
                    model: r.get(1)?,
                    serial_masked: r.get(2)?,
                    android_version: r.get(3)?,
                    api_level: r.get(4)?,
                    security_patch: r.get(5)?,
                })
            },
        )
        .optional()?)
}

pub fn entries(conn: &Connection, order_id: i64) -> AppResult<Vec<Entry>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.created_at, e.kind, e.title, e.detail, e.status, u.display_name, e.source, e.verifies_entry_id, \
         (SELECT v.id FROM order_entries v WHERE v.verifies_entry_id = e.id ORDER BY v.id DESC LIMIT 1) \
         FROM order_entries e LEFT JOIN users u ON u.id = e.author_id WHERE e.order_id = ?1 ORDER BY e.created_at, e.id",
    )?;
    let rows = stmt.query_map([order_id], |r| {
        Ok(Entry {
            id: r.get(0)?,
            created_at: r.get(1)?,
            kind: r.get(2)?,
            title: r.get(3)?,
            detail: r.get(4)?,
            status: r.get(5)?,
            author: r.get(6)?,
            source: r.get(7)?,
            verifies_entry_id: r.get(8)?,
            verified_by_entry_id: r.get(9)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn get(conn: &Connection, id: i64) -> AppResult<Order> {
    let mut order = conn
        .query_row(
            "SELECT o.id, o.number, o.customer_id, c.display_name, c.contact, o.device_id, o.device_label, o.request_type, o.reported_problem, \
             o.status, o.consent_status, o.consent_by, o.consent_at, o.ownership_method, u.display_name, o.received_at, o.delivered_at, \
             o.created_at, o.updated_at, o.notes \
             FROM service_orders o JOIN customers c ON c.id = o.customer_id LEFT JOIN users u ON u.id = o.technician_id WHERE o.id = ?1",
            [id],
            |r| {
                Ok(Order {
                    id: r.get(0)?,
                    number: r.get(1)?,
                    customer_id: r.get(2)?,
                    customer: r.get(3)?,
                    customer_contact: r.get(4)?,
                    device_id: r.get(5)?,
                    device_label: r.get(6)?,
                    device: None,
                    request_type: r.get(7)?,
                    reported_problem: r.get(8)?,
                    status: r.get(9)?,
                    consent_status: r.get(10)?,
                    consent_by: r.get(11)?,
                    consent_at: r.get(12)?,
                    ownership_method: r.get(13)?,
                    technician: r.get(14)?,
                    received_at: r.get(15)?,
                    delivered_at: r.get(16)?,
                    created_at: r.get(17)?,
                    updated_at: r.get(18)?,
                    notes: r.get(19)?,
                    entries: Vec::new(),
                    next_statuses: Vec::new(),
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound("La orden no existe.".into()))?;
    order.device = device_info(conn, order.device_id)?;
    order.entries = entries(conn, id)?;
    let has_pending = !order.unverified().is_empty();
    order.next_statuses = OrderStatus::parse(&order.status)?
        .next()
        .iter()
        .copied()
        .filter(|s| *s != OrderStatus::PendingVerification || has_pending)
        .collect();
    Ok(order)
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct OrderFilter {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub query: String,
    /// Inclusive range on `received_at` (YYYY-MM-DD).
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
}

pub fn list(conn: &Connection, filter: &OrderFilter) -> AppResult<Vec<OrderSummary>> {
    let status = filter.status.as_deref().filter(|s| !s.is_empty());
    if let Some(s) = status {
        if s != "active" {
            OrderStatus::parse(s).map_err(|_| AppError::Validation("Estado no válido.".into()))?;
        }
    }
    let query = filter.query.trim().to_lowercase();
    let like = format!("%{}%", query.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
    let mut stmt = conn.prepare(
        "SELECT o.id, o.number, c.display_name, \
         COALESCE(NULLIF(TRIM(COALESCE(d.manufacturer, '') || ' ' || COALESCE(d.model, '')), ''), NULLIF(o.device_label, ''), '—'), \
         o.request_type, o.status, o.consent_status, u.display_name, o.received_at, o.updated_at, \
         (SELECT COUNT(*) FROM order_entries e WHERE e.order_id = o.id AND e.status = 'pending_verification' \
            AND NOT EXISTS (SELECT 1 FROM order_entries v WHERE v.verifies_entry_id = e.id)) \
         FROM service_orders o JOIN customers c ON c.id = o.customer_id LEFT JOIN devices d ON d.id = o.device_id \
         LEFT JOIN users u ON u.id = o.technician_id \
         WHERE (?1 IS NULL OR (?1 = 'active' AND o.status NOT IN ('delivered', 'cancelled')) OR o.status = ?1) \
           AND (?2 = '' OR lower(o.number) LIKE ?3 ESCAPE '\\' OR lower(c.display_name) LIKE ?3 ESCAPE '\\' \
                OR lower(o.device_label) LIKE ?3 ESCAPE '\\' OR lower(COALESCE(d.manufacturer, '') || ' ' || COALESCE(d.model, '')) LIKE ?3 ESCAPE '\\') \
           AND (?4 IS NULL OR substr(o.received_at, 1, 10) >= ?4) AND (?5 IS NULL OR substr(o.received_at, 1, 10) <= ?5) \
         ORDER BY o.id DESC LIMIT ?6",
    )?;
    let rows = stmt.query_map(
        params![status, query, like, filter.from, filter.to, filter.limit.unwrap_or(200).clamp(1, 5000)],
        |r| {
            Ok(OrderSummary {
                id: r.get(0)?,
                number: r.get(1)?,
                customer: r.get(2)?,
                device: r.get(3)?,
                request_type: r.get(4)?,
                status: r.get(5)?,
                consent_status: r.get(6)?,
                technician: r.get(7)?,
                received_at: r.get(8)?,
                updated_at: r.get(9)?,
                pending_verification: r.get(10)?,
            })
        },
    )?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn ensure_open(order: &Order) -> AppResult<OrderStatus> {
    let status = OrderStatus::parse(&order.status)?;
    if status.closed() {
        return Err(AppError::Validation(format!("La orden {} está {}: no admite cambios.", order.number, OrderStatus::label(&order.status).to_lowercase())));
    }
    Ok(status)
}

pub fn record_consent(
    conn: &Connection,
    user_id: i64,
    id: i64,
    decision: ConsentDecision,
    by_name: &str,
    method: Option<OwnershipMethod>,
    note: &str,
) -> AppResult<Order> {
    let order = get(conn, id)?;
    ensure_open(&order)?;
    let by = clean(by_name, "Nombre", 120)?;
    let note = clean(note, "Observaciones", MAX_TEXT)?;
    let ts = now();
    match decision {
        ConsentDecision::Granted => {
            if by.is_empty() {
                return Err(AppError::Validation("Escriba el nombre de quien autoriza.".into()));
            }
            let Some(method) = method else {
                return Err(AppError::Validation("Indique cómo se comprobó la propiedad o la autorización.".into()));
            };
            if order.consent_status == "granted" {
                return Err(AppError::Validation("El consentimiento ya está registrado.".into()));
            }
            conn.execute(
                "UPDATE service_orders SET consent_status = 'granted', consent_by = ?1, consent_at = ?2, ownership_method = ?3, updated_at = ?2 WHERE id = ?4",
                params![by, ts, method.as_str(), id],
            )?;
            let detail = format!("{by} · {}{}", OwnershipMethod::label(method.as_str()), if note.is_empty() { String::new() } else { format!(" · {note}") });
            insert_entry(conn, id, Some(user_id), EntryKind::Consent, "Consentimiento concedido", &detail, EntryStatus::Ok, "", None)?;
            audit::record(conn, "order_consent_granted", Severity::Info, Some(user_id), "", serde_json::json!({ "order": order.number, "method": method }))?;
        }
        ConsentDecision::Denied => {
            if order.consent_status == "granted" {
                return Err(AppError::Validation("El consentimiento ya se concedió: si el cliente lo retira, regístrelo como revocado.".into()));
            }
            conn.execute(
                "UPDATE service_orders SET consent_status = 'denied', consent_by = ?1, consent_at = ?2, updated_at = ?2 WHERE id = ?3",
                params![by, ts, id],
            )?;
            insert_entry(conn, id, Some(user_id), EntryKind::Consent, "Consentimiento no concedido", &note, EntryStatus::Failed, "", None)?;
            insert_entry(
                conn,
                id,
                Some(user_id),
                EntryKind::Incident,
                "Sin autorización comprobada: servicio detenido",
                "No se realizarán operaciones sobre las cuentas ni borrados. Recomendado: contactar con el propietario anterior, el vendedor, el fabricante o un servicio autorizado.",
                EntryStatus::Info,
                "",
                None,
            )?;
            audit::record(conn, "order_consent_denied", Severity::Warning, Some(user_id), "", serde_json::json!({ "order": order.number }))?;
        }
        ConsentDecision::Revoked => {
            if order.consent_status != "granted" {
                return Err(AppError::Validation("Solo se puede revocar un consentimiento concedido.".into()));
            }
            conn.execute("UPDATE service_orders SET consent_status = 'revoked', updated_at = ?1 WHERE id = ?2", params![ts, id])?;
            insert_entry(conn, id, Some(user_id), EntryKind::Consent, "Consentimiento revocado por el cliente", &note, EntryStatus::Failed, "", None)?;
            audit::record(conn, "order_consent_revoked", Severity::Warning, Some(user_id), "", serde_json::json!({ "order": order.number }))?;
        }
    }
    get(conn, id)
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewEntry {
    pub kind: EntryKind,
    pub title: String,
    #[serde(default)]
    pub detail: String,
    pub status: EntryStatus,
    #[serde(default)]
    pub verifies_entry_id: Option<i64>,
}

pub fn add_entry(conn: &Connection, user_id: i64, id: i64, new: &NewEntry) -> AppResult<Order> {
    let order = get(conn, id)?;
    let status = ensure_open(&order)?;
    if matches!(new.kind, EntryKind::Action | EntryKind::Consent) {
        return Err(AppError::Validation(
            "Las acciones ejecutadas y los consentimientos los registra la aplicación cuando ocurren. Use «Vincular trabajo del equipo» o el apartado de consentimiento.".into(),
        ));
    }
    if !new.kind.allows(new.status) {
        return Err(AppError::Validation(format!(
            "Una entrada de tipo «{}» no puede tener el estado «{}».",
            EntryKind::label(new.kind.as_str()),
            EntryStatus::label(new.status.as_str())
        )));
    }
    let title = clean(&new.title, "Título", 200)?;
    if title.is_empty() {
        return Err(AppError::Validation("Escriba un título.".into()));
    }
    let detail = clean(&new.detail, "Detalle", MAX_TEXT)?;
    if let Some(target) = new.verifies_entry_id {
        if new.kind != EntryKind::Verification {
            return Err(AppError::Validation("Solo una verificación puede cerrar una entrada pendiente.".into()));
        }
        let entry = order.entries.iter().find(|e| e.id == target).ok_or_else(|| AppError::Validation("La entrada a verificar no pertenece a esta orden.".into()))?;
        if entry.status != "pending_verification" || entry.verified_by_entry_id.is_some() {
            return Err(AppError::Validation("Esa entrada no está pendiente de verificación.".into()));
        }
    }
    insert_entry(conn, id, Some(user_id), new.kind, &title, &detail, new.status, "", new.verifies_entry_id)?;
    if new.status == EntryStatus::PendingVerification && status == OrderStatus::InProgress {
        set_status_raw(conn, id, OrderStatus::PendingVerification)?;
    }
    get(conn, id)
}

fn set_status_raw(conn: &Connection, id: i64, status: OrderStatus) -> AppResult<()> {
    let ts = now();
    let delivered = if status == OrderStatus::Delivered { Some(ts.clone()) } else { None };
    conn.execute(
        "UPDATE service_orders SET status = ?1, updated_at = ?2, delivered_at = COALESCE(?3, delivered_at) WHERE id = ?4",
        params![status.as_str(), ts, delivered, id],
    )?;
    Ok(())
}

/// Requested status change. `can_cancel` comes from the user's role (administrators).
pub fn set_status(conn: &Connection, user_id: i64, can_cancel: bool, id: i64, to: OrderStatus, note: &str) -> AppResult<Order> {
    let order = get(conn, id)?;
    let from = OrderStatus::parse(&order.status)?;
    if !from.next().contains(&to) {
        return Err(AppError::Validation(format!(
            "No se puede pasar de «{}» a «{}».",
            OrderStatus::label(from.as_str()),
            OrderStatus::label(to.as_str())
        )));
    }
    let note = clean(note, "Observaciones", MAX_TEXT)?;
    let unverified = order.unverified().len();
    match to {
        OrderStatus::InProgress if order.consent_status != "granted" => {
            return Err(AppError::ConsentRequired("Registre el consentimiento del propietario antes de empezar el trabajo.".into()));
        }
        OrderStatus::PendingVerification if unverified == 0 => {
            return Err(AppError::Validation("No hay entradas pendientes de verificación.".into()));
        }
        OrderStatus::Completed => {
            if unverified > 0 {
                return Err(AppError::Validation(format!(
                    "Quedan {unverified} entradas pendientes de verificación. Registre la verificación (correcta o fallida) antes de completar."
                )));
            }
            match order.consent_status.as_str() {
                "granted" => {
                    let has_work = order.entries.iter().any(|e| matches!(e.kind.as_str(), "diagnosis" | "action" | "verification") && e.source != "device");
                    if !has_work {
                        return Err(AppError::Validation("Registre al menos un diagnóstico, acción o verificación antes de completar.".into()));
                    }
                }
                "denied" | "revoked" => {
                    if !order.entries.iter().any(|e| e.kind == "incident") {
                        return Err(AppError::Validation("Registre una incidencia que explique por qué se detuvo el servicio.".into()));
                    }
                }
                _ => return Err(AppError::ConsentRequired("Registre la decisión de consentimiento antes de completar la orden.".into())),
            }
        }
        OrderStatus::Cancelled if !can_cancel => {
            return Err(AppError::Forbidden("Solo un administrador puede cancelar órdenes.".into()));
        }
        OrderStatus::Cancelled if note.is_empty() => {
            return Err(AppError::Validation("Indique el motivo de la cancelación.".into()));
        }
        _ => {}
    }
    set_status_raw(conn, id, to)?;
    let title = format!("Estado: {} → {}", OrderStatus::label(from.as_str()), OrderStatus::label(to.as_str()));
    insert_entry(conn, id, Some(user_id), EntryKind::Note, &title, &note, EntryStatus::Info, "", None)?;
    audit::record(
        conn,
        "order_status_changed",
        if to == OrderStatus::Cancelled { Severity::Warning } else { Severity::Info },
        Some(user_id),
        "",
        serde_json::json!({ "order": order.number, "from": from, "to": to }),
    )?;
    get(conn, id)
}

pub fn set_notes(conn: &Connection, id: i64, notes: &str) -> AppResult<Order> {
    let order = get(conn, id)?;
    ensure_open(&order)?;
    let notes = clean(notes, "Observaciones", MAX_TEXT)?;
    conn.execute("UPDATE service_orders SET notes = ?1, updated_at = ?2 WHERE id = ?3", params![notes, now(), id])?;
    get(conn, id)
}

/// Notes a generated report in the order (also on delivered or cancelled orders: the report is part of the record).
pub fn record_report(conn: &Connection, user_id: i64, id: i64, file_name: &str) -> AppResult<()> {
    let order = get(conn, id)?;
    insert_entry(conn, id, Some(user_id), EntryKind::Note, "Reporte PDF generado", file_name, EntryStatus::Info, "", None)?;
    audit::record(conn, "order_report_generated", Severity::Info, Some(user_id), "", serde_json::json!({ "order": order.number }))?;
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Linking work already recorded by other modules (FRP, account checks, checklists, resets).
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct LinkCandidate {
    pub source: String,
    pub created_at: String,
    pub kind: EntryKind,
    pub title: String,
    pub detail: String,
    pub status: EntryStatus,
    /// Source of the entry this one verifies (resolved to an entry id when linking).
    pub verifies_source: Option<String>,
    pub linked: bool,
}

fn label_matches(sql_column: &str) -> String {
    format!("(?2 <> '' AND lower(trim({sql_column})) = ?2)")
}

pub fn candidates(conn: &Connection, id: i64) -> AppResult<Vec<LinkCandidate>> {
    let order = get(conn, id)?;
    let label = order.device_label.trim().to_lowercase();
    let device = order.device_id;
    let mut out: Vec<LinkCandidate> = Vec::new();

    // FRP assessments → diagnosis (and an incident when the process was stopped for lack of authorization).
    {
        let sql = format!(
            "SELECT id, created_at, outcome, observations_json FROM frp_assessments \
             WHERE (?1 IS NOT NULL AND device_id = ?1) OR (device_id IS NULL AND {}) ORDER BY id DESC LIMIT 20",
            label_matches("json_extract(observations_json, '$.device_label')")
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![device, label], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?)))?;
        for row in rows {
            let (fid, at, outcome, json) = row?;
            let data: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
            let causes: Vec<String> = data["possible_causes"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
            let stop = outcome == "stop_no_authorization";
            out.push(LinkCandidate {
                source: format!("frp:{fid}"),
                created_at: at.clone(),
                kind: EntryKind::Diagnosis,
                title: format!("Diagnóstico FRP: {}", frp_outcome_label(&outcome)),
                detail: if causes.is_empty() { String::new() } else { format!("Causas posibles: {}", causes.join("; ")) },
                status: EntryStatus::Info,
                verifies_source: None,
                linked: false,
            });
            if stop {
                out.push(LinkCandidate {
                    source: format!("frp:{fid}:stop"),
                    created_at: at,
                    kind: EntryKind::Incident,
                    title: "FRP: no se demostró propiedad o autorización; proceso detenido".into(),
                    detail: "Orientado a recuperación oficial de la cuenta, propietario anterior, vendedor o soporte del fabricante.".into(),
                    status: EntryStatus::Info,
                    verifies_source: None,
                    linked: false,
                });
            }
        }
    }

    // Account checks (connected devices only) → diagnosis with types and counts, never names.
    if let Some(dev) = device {
        let mut stmt = conn.prepare("SELECT id, created_at, google_count, protected_count, summary_json FROM account_checks WHERE device_id = ?1 ORDER BY id DESC LIMIT 10")?;
        let rows = stmt.query_map([dev], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?, r.get::<_, String>(4)?)))?;
        for row in rows {
            let (cid, at, google, protected, json) = row?;
            let summary: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
            let types: Vec<String> = summary
                .as_array()
                .map(|a| a.iter().map(|t| format!("{}: {}", t["label"].as_str().unwrap_or("?"), t["count"].as_i64().unwrap_or(0))).collect())
                .unwrap_or_default();
            out.push(LinkCandidate {
                source: format!("accounts:{cid}"),
                created_at: at,
                kind: EntryKind::Diagnosis,
                title: if google == 0 && protected == 0 {
                    "Comprobación de cuentas: sin cuentas en el equipo".into()
                } else {
                    format!("Comprobación de cuentas: {google} Google, {protected} con protección antirrobo")
                },
                detail: if types.is_empty() { "Sin cuentas".into() } else { types.join(" · ") },
                status: EntryStatus::Info,
                verifies_source: None,
                linked: false,
            });
        }
    }

    // Completed preparation lists → verification of the preparation.
    {
        let sql = format!(
            "SELECT id, updated_at, items_json FROM checklists WHERE status = 'complete' AND ((?1 IS NOT NULL AND device_id = ?1) OR (device_id IS NULL AND {})) ORDER BY id DESC LIMIT 10",
            label_matches("device_label")
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![device, label], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
        for row in rows {
            let (lid, at) = row?;
            out.push(LinkCandidate {
                source: format!("checklist:{lid}"),
                created_at: at,
                kind: EntryKind::Verification,
                title: "Preparación para nuevo propietario completa".into(),
                detail: "Autorización, copia de seguridad, retiro de cuentas con evidencia, SIM/eSIM, accesorios y confirmación del borrado.".into(),
                status: EntryStatus::Ok,
                verifies_source: None,
                linked: false,
            });
        }
    }

    // Resets → consent (owner confirmation), action/start (pending verification) and its verification.
    {
        let sql = format!(
            "SELECT r.id, r.created_at, r.status, r.method, r.owner_name, r.confirmed_at, r.actions_json, r.verification_json, r.updated_at \
             FROM resets r WHERE ((?1 IS NOT NULL AND r.device_id = ?1) OR (r.device_id IS NULL AND {})) AND r.confirmed_at IS NOT NULL ORDER BY r.id DESC LIMIT 10",
            label_matches("r.device_label")
        );
        let mut stmt = conn.prepare(&sql)?;
        type Row = (i64, String, String, String, String, String, String, String, String);
        let rows = stmt.query_map(params![device, label], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?))
        })?;
        for row in rows {
            let (rid, _created, status, method, owner, confirmed_at, actions_json, verification_json, updated): Row = row?;
            let method_label = if method == "recovery" { "Recovery" } else { "Ajustes" };
            out.push(LinkCandidate {
                source: format!("reset:{rid}:confirmed"),
                created_at: confirmed_at.clone(),
                kind: EntryKind::Consent,
                title: format!("Borrado confirmado por {owner}"),
                detail: format!("Restablecimiento n.º {rid} · método {method_label} · confirmación escrita de un solo uso"),
                status: EntryStatus::Ok,
                verifies_source: None,
                linked: false,
            });
            let actions: Vec<serde_json::Value> = serde_json::from_str(&actions_json).unwrap_or_default();
            if !actions.is_empty() {
                let app_action = actions.iter().any(|a| a["kind"].as_str() != Some("manual_start") && a["ok"].as_bool() == Some(true));
                let first_at = actions.first().and_then(|a| a["at"].as_str()).unwrap_or(&confirmed_at).to_string();
                let described: Vec<String> = actions
                    .iter()
                    .map(|a| {
                        let what = match a["kind"].as_str() {
                            Some("open_settings") => "AndroidMasters abrió Ajustes en el teléfono",
                            Some("reboot_recovery") => "AndroidMasters reinició el teléfono en Recovery",
                            _ => "Borrado iniciado en el teléfono por el técnico o el propietario",
                        };
                        if a["ok"].as_bool() == Some(false) { format!("{what} (falló)") } else { what.to_string() }
                    })
                    .collect();
                out.push(LinkCandidate {
                    source: format!("reset:{rid}:started"),
                    created_at: first_at,
                    // Only an operation the app performed is an "action"; a wipe started by hand on the phone is a note.
                    kind: if app_action { EntryKind::Action } else { EntryKind::Note },
                    title: format!("Restablecimiento de fábrica iniciado ({method_label})"),
                    detail: described.join(" · "),
                    status: if app_action { EntryStatus::PendingVerification } else { EntryStatus::Info },
                    verifies_source: None,
                    linked: false,
                });
            }
            if status == "verified" || status == "failed" {
                let v: serde_json::Value = serde_json::from_str(&verification_json).unwrap_or_default();
                out.push(LinkCandidate {
                    source: format!("reset:{rid}:{status}"),
                    created_at: v["at"].as_str().unwrap_or(&updated).to_string(),
                    kind: EntryKind::Verification,
                    title: if status == "verified" { "Restablecimiento verificado".into() } else { "Restablecimiento no verificado".into() },
                    detail: v["message"].as_str().unwrap_or("").to_string(),
                    status: if status == "verified" { EntryStatus::Ok } else { EntryStatus::Failed },
                    verifies_source: Some(format!("reset:{rid}:started")),
                    linked: false,
                });
            }
        }
    }

    let linked: std::collections::HashSet<&str> = order.entries.iter().map(|e| e.source.as_str()).filter(|s| !s.is_empty()).collect();
    for c in &mut out {
        c.linked = linked.contains(c.source.as_str());
    }
    out.sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.source.cmp(&b.source)));
    Ok(out)
}

fn frp_outcome_label(outcome: &str) -> &'static str {
    match outcome {
        "stop_no_authorization" => "detenido, sin autorización comprobada",
        "no_indication" => "sin indicios de FRP",
        "owner_signs_in" => "el propietario inicia sesión con su cuenta",
        "account_recovery" => "recuperación oficial de la cuenta",
        "wait_security_period" => "periodo de seguridad tras cambio de contraseña",
        "manufacturer_support" => "soporte oficial del fabricante",
        _ => "registrado",
    }
}

/// Adds the selected candidates to the order, in chronological order, skipping those already linked.
pub fn link(conn: &Connection, user_id: i64, id: i64, sources: &[String]) -> AppResult<Order> {
    let order = get(conn, id)?;
    let status = ensure_open(&order)?;
    let all = candidates(conn, id)?;
    let mut added = 0;
    let mut pending_added = false;
    for c in all.iter().filter(|c| !c.linked && sources.contains(&c.source)) {
        let verifies = match &c.verifies_source {
            Some(src) => conn
                .query_row("SELECT id FROM order_entries WHERE order_id = ?1 AND source = ?2", params![id, src], |r| r.get::<_, i64>(0))
                .optional()?
                .filter(|eid| {
                    // Only close entries that are still waiting for evidence.
                    conn.query_row(
                        "SELECT status = 'pending_verification' AND NOT EXISTS (SELECT 1 FROM order_entries v WHERE v.verifies_entry_id = ?1) FROM order_entries WHERE id = ?1",
                        [eid],
                        |r| r.get::<_, bool>(0),
                    )
                    .unwrap_or(false)
                }),
            None => None,
        };
        let entry_id = insert_entry(conn, id, Some(user_id), c.kind, &c.title, &c.detail, c.status, &c.source, verifies)?;
        // The entry keeps the time the work was actually recorded, not the time it was linked.
        conn.execute("UPDATE order_entries SET created_at = ?1 WHERE id = ?2", params![c.created_at, entry_id])?;
        pending_added |= c.status == EntryStatus::PendingVerification;
        added += 1;
    }
    if added == 0 {
        return Err(AppError::Validation("No hay registros nuevos para vincular.".into()));
    }
    if pending_added && status == OrderStatus::InProgress && !get(conn, id)?.unverified().is_empty() {
        set_status_raw(conn, id, OrderStatus::PendingVerification)?;
    }
    audit::record(conn, "order_work_linked", Severity::Info, Some(user_id), "", serde_json::json!({ "order": order.number, "count": added }))?;
    get(conn, id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;

    fn setup(conn: &Connection) {
        conn.execute("INSERT INTO users (username, display_name, role, password_hash, created_at) VALUES ('t','Tec','technician','h','t')", []).unwrap();
        conn.execute(
            "INSERT INTO devices (fingerprint, serial_masked, serial_encrypted, brand, manufacturer, model, android_version, api_level, security_patch, first_seen, last_seen) \
             VALUES ('f','R58***DE','x','samsung','Samsung','SM-A145','14',34,'2026-09-01','t','t')",
            [],
        )
        .unwrap();
    }

    fn new_order(label: &str) -> NewOrder {
        NewOrder {
            customer_name: "Ana Cliente".into(),
            customer_contact: "600 000 000".into(),
            device_label: label.into(),
            request_type: RequestType::Transfer,
            reported_problem: "Venta del equipo".into(),
        }
    }

    fn grant(conn: &Connection, id: i64) -> Order {
        record_consent(conn, 1, id, ConsentDecision::Granted, "Ana Cliente", Some(OwnershipMethod::OwnerPresentId), "").unwrap()
    }

    #[test]
    fn create_numbers_and_identifies_device() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.conn();
        setup(&conn);
        let a = create(&conn, 1, Some(1), &new_order("")).unwrap();
        let b = create(&conn, 1, None, &new_order("Xiaomi Redmi 12")).unwrap();
        assert!(a.number.ends_with("-0001") && b.number.ends_with("-0002"));
        assert_eq!(a.device_name(), "Samsung SM-A145");
        assert!(a.entries.iter().any(|e| e.kind == "diagnosis" && e.detail.contains("R58***DE")), "masked serial only");
        assert_eq!(b.device_name(), "Xiaomi Redmi 12");
        assert!(create(&conn, 1, None, &new_order("")).is_err(), "device required");
        let mut nameless = new_order("X");
        nameless.customer_name = "  ".into();
        assert!(create(&conn, 1, None, &nameless).is_err());
        let found = list(&conn, &OrderFilter { query: "redmi".into(), ..Default::default() }).unwrap();
        assert_eq!(found.len(), 1);
        let found = list(&conn, &OrderFilter { query: "100%".into(), ..Default::default() }).unwrap();
        assert!(found.is_empty(), "LIKE wildcards are escaped");
    }

    #[test]
    fn consent_gates_work_and_completion() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.conn();
        setup(&conn);
        let o = create(&conn, 1, Some(1), &new_order("")).unwrap();
        assert!(matches!(set_status(&conn, 1, false, o.id, OrderStatus::InProgress, ""), Err(AppError::ConsentRequired(_))));
        assert!(record_consent(&conn, 1, o.id, ConsentDecision::Granted, "Ana", None, "").is_err(), "method required");
        let o = grant(&conn, o.id);
        assert_eq!(o.consent_status, "granted");
        let o = set_status(&conn, 1, false, o.id, OrderStatus::InProgress, "").unwrap();
        assert!(!o.next_statuses.contains(&OrderStatus::PendingVerification), "nothing pending: not offered");
        assert!(set_status(&conn, 1, false, o.id, OrderStatus::Completed, "").is_err(), "needs real work, not only the identification");
        let o = add_entry(&conn, 1, o.id, &NewEntry { kind: EntryKind::Diagnosis, title: "Batería 86 %".into(), detail: String::new(), status: EntryStatus::Info, verifies_entry_id: None }).unwrap();
        let o = set_status(&conn, 1, false, o.id, OrderStatus::Completed, "").unwrap();
        let o = set_status(&conn, 1, false, o.id, OrderStatus::Delivered, "").unwrap();
        assert!(o.delivered_at.is_some());
        assert!(o.next_statuses.is_empty());
        assert!(add_entry(&conn, 1, o.id, &NewEntry { kind: EntryKind::Note, title: "x".into(), detail: String::new(), status: EntryStatus::Info, verifies_entry_id: None }).is_err(), "delivered orders are closed");
    }

    #[test]
    fn denied_consent_stops_and_needs_incident() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.conn();
        setup(&conn);
        let o = create(&conn, 1, None, &new_order("Moto G")).unwrap();
        let o = record_consent(&conn, 1, o.id, ConsentDecision::Denied, "", None, "No aporta prueba de compra").unwrap();
        assert_eq!(o.consent_status, "denied");
        assert!(o.entries.iter().any(|e| e.kind == "incident"));
        assert!(set_status(&conn, 1, false, o.id, OrderStatus::InProgress, "").is_err());
        assert!(record_consent(&conn, 1, o.id, ConsentDecision::Revoked, "", None, "").is_err());
        let o = set_status(&conn, 1, false, o.id, OrderStatus::Completed, "").unwrap();
        assert_eq!(o.status, "completed");
    }

    #[test]
    fn entries_respect_their_kind() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.conn();
        setup(&conn);
        let o = grant(&conn, create(&conn, 1, Some(1), &new_order("")).unwrap().id);
        let e = |kind, status| NewEntry { kind, title: "t".into(), detail: String::new(), status, verifies_entry_id: None };
        assert!(add_entry(&conn, 1, o.id, &e(EntryKind::Action, EntryStatus::Ok)).is_err(), "actions only from the app");
        assert!(add_entry(&conn, 1, o.id, &e(EntryKind::Consent, EntryStatus::Ok)).is_err());
        assert!(add_entry(&conn, 1, o.id, &e(EntryKind::Instruction, EntryStatus::Ok)).is_err(), "an instruction is never done");
        assert!(add_entry(&conn, 1, o.id, &e(EntryKind::Verification, EntryStatus::PendingVerification)).is_err());
        let o = add_entry(&conn, 1, o.id, &e(EntryKind::Instruction, EntryStatus::PendingVerification)).unwrap();
        let pending = o.unverified()[0].id;
        assert!(set_status(&conn, 1, false, o.id, OrderStatus::InProgress, "").is_ok());
        assert!(set_status(&conn, 1, false, o.id, OrderStatus::Completed, "").is_err(), "pending verification blocks completion");
        let mut v = e(EntryKind::Verification, EntryStatus::Ok);
        v.verifies_entry_id = Some(pending);
        let o = add_entry(&conn, 1, o.id, &v).unwrap();
        assert!(o.unverified().is_empty());
        assert!(add_entry(&conn, 1, o.id, &v).is_err(), "already verified");
        assert!(set_status(&conn, 1, false, o.id, OrderStatus::Cancelled, "x").is_err(), "technicians cannot cancel");
        assert!(set_status(&conn, 1, true, o.id, OrderStatus::Cancelled, "").is_err(), "reason required");
        assert_eq!(set_status(&conn, 1, true, o.id, OrderStatus::Cancelled, "Cliente desiste").unwrap().status, "cancelled");
    }

    #[test]
    fn links_recorded_work_once_and_chains_verification() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.conn();
        setup(&conn);
        conn.execute("INSERT INTO account_checks (device_id, created_at, author_id, summary_json, google_count, protected_count) VALUES (1, '2026-11-20T10:00:00', 1, '[]', 0, 0)", []).unwrap();
        conn.execute(
            "INSERT INTO resets (device_id, device_label, method, status, confirmed_at, confirmed_by, owner_name, actions_json, verification_json, created_at, updated_at, author_id) \
             VALUES (1, '', 'settings', 'pending_verification', '2026-11-20T11:00:00', 1, 'Ana Cliente', \
             '[{\"kind\":\"open_settings\",\"ok\":true,\"at\":\"2026-11-20T11:01:00\",\"detail\":\"\"}]', '{}', '2026-11-20T10:59:00', '2026-11-20T11:01:00', 1)",
            [],
        )
        .unwrap();
        let o = grant(&conn, create(&conn, 1, Some(1), &new_order("")).unwrap().id);
        let o = set_status(&conn, 1, false, o.id, OrderStatus::InProgress, "").unwrap();
        let c = candidates(&conn, o.id).unwrap();
        let sources: Vec<String> = c.iter().map(|c| c.source.clone()).collect();
        assert_eq!(sources, vec!["accounts:1", "reset:1:confirmed", "reset:1:started"]);
        assert!(c.iter().all(|c| !c.detail.contains('@')));
        let o = link(&conn, 1, o.id, &sources).unwrap();
        assert_eq!(o.status, "pending_verification", "a started reset waits for verification");
        assert_eq!(o.entries.iter().find(|e| e.source == "accounts:1").unwrap().created_at, "2026-11-20T10:00:00", "original time kept");
        assert_eq!(o.entries[0].source, "accounts:1", "register is chronological");
        assert!(o.next_statuses.contains(&OrderStatus::Completed));
        let started = o.entries.iter().find(|e| e.source == "reset:1:started").unwrap();
        assert_eq!((started.kind.as_str(), started.status.as_str()), ("action", "pending_verification"));
        assert!(link(&conn, 1, o.id, &sources).is_err(), "nothing new to link");

        conn.execute("UPDATE resets SET status = 'verified', verification_json = '{\"at\":\"2026-11-20T11:20:00\",\"message\":\"Asistente visible\"}' WHERE id = 1", []).unwrap();
        let o = link(&conn, 1, o.id, &["reset:1:verified".to_string()]).unwrap();
        assert!(o.unverified().is_empty(), "the verification closes the started reset");
        let o = set_status(&conn, 1, false, o.id, OrderStatus::Completed, "").unwrap();
        assert_eq!(o.status, "completed");
    }

    #[test]
    fn manual_wipe_start_is_not_an_executed_action() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.conn();
        setup(&conn);
        conn.execute(
            "INSERT INTO resets (device_id, device_label, method, status, confirmed_at, confirmed_by, owner_name, actions_json, created_at, updated_at, author_id) \
             VALUES (NULL, 'Moto G', 'recovery', 'pending_verification', 't1', 1, 'Luis', '[{\"kind\":\"manual_start\",\"ok\":true,\"at\":\"t2\",\"detail\":\"\"}]', 't0', 't2', 1)",
            [],
        )
        .unwrap();
        let o = create(&conn, 1, None, &new_order("moto g ")).unwrap();
        let c = candidates(&conn, o.id).unwrap();
        let started = c.iter().find(|c| c.source == "reset:1:started").expect("label matches case-insensitively");
        assert_eq!(started.kind, EntryKind::Note);
    }
}
