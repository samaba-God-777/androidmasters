use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::State;

use crate::error::{AppError, AppResult};
use crate::services::audit::{self, Severity};
use crate::services::orders::{self, ConsentDecision, LinkCandidate, NewEntry, NewOrder, Order, OrderFilter, OrderStatus, OrderSummary, OwnershipMethod};
use crate::services::reports::{self, PeriodSummary};
use crate::services::settings;
use crate::services::users::Permission;
use crate::state::AppState;
use crate::util::now;

#[tauri::command]
pub fn orders_list(state: State<'_, AppState>, filter: OrderFilter) -> AppResult<Vec<OrderSummary>> {
    state.require_user()?;
    orders::list(&state.db.conn(), &filter)
}

#[tauri::command]
pub fn order_get(state: State<'_, AppState>, id: i64) -> AppResult<Order> {
    state.require_user()?;
    orders::get(&state.db.conn(), id)
}

/// `device` is the anonymous id of a connected, identified device; otherwise `order.device_label` is used.
#[tauri::command]
pub fn order_create(state: State<'_, AppState>, device: Option<String>, order: NewOrder) -> AppResult<Order> {
    let user = state.require_user()?;
    let device_id = match device.as_deref() {
        Some(id) => Some(state.device_db_id(id)?),
        None => None,
    };
    orders::create(&state.db.conn(), user.id, device_id, &order)
}

#[tauri::command]
pub fn order_consent(
    state: State<'_, AppState>,
    id: i64,
    decision: ConsentDecision,
    by_name: String,
    method: Option<OwnershipMethod>,
    note: String,
) -> AppResult<Order> {
    let user = state.require_user()?;
    orders::record_consent(&state.db.conn(), user.id, id, decision, &by_name, method, &note)
}

#[tauri::command]
pub fn order_add_entry(state: State<'_, AppState>, id: i64, entry: NewEntry) -> AppResult<Order> {
    let user = state.require_user()?;
    orders::add_entry(&state.db.conn(), user.id, id, &entry)
}

#[tauri::command]
pub fn order_set_status(state: State<'_, AppState>, id: i64, status: OrderStatus, note: String) -> AppResult<Order> {
    let user = state.require_user()?;
    orders::set_status(&state.db.conn(), user.id, user.can(Permission::CancelOrders), id, status, &note)
}

#[tauri::command]
pub fn order_set_notes(state: State<'_, AppState>, id: i64, notes: String) -> AppResult<Order> {
    state.require_user()?;
    orders::set_notes(&state.db.conn(), id, &notes)
}

#[tauri::command]
pub fn order_candidates(state: State<'_, AppState>, id: i64) -> AppResult<Vec<LinkCandidate>> {
    state.require_user()?;
    orders::candidates(&state.db.conn(), id)
}

#[tauri::command]
pub fn order_link(state: State<'_, AppState>, id: i64, sources: Vec<String>) -> AppResult<Order> {
    let user = state.require_user()?;
    orders::link(&state.db.conn(), user.id, id, &sources)
}

#[derive(Debug, Serialize)]
pub struct SavedFile {
    pub path: String,
    pub name: String,
    pub folder: String,
}

fn saved(path: PathBuf) -> SavedFile {
    SavedFile {
        name: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        folder: path.parent().map(|p| p.display().to_string()).unwrap_or_default(),
        path: path.display().to_string(),
    }
}

fn workshop(state: &AppState) -> String {
    settings::get(&state.db.conn(), settings::WORKSHOP_NAME).ok().flatten().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| "AndroidMasters".into())
}

#[tauri::command]
pub fn order_report(state: State<'_, AppState>, id: i64) -> AppResult<SavedFile> {
    let user = state.require_user()?;
    let order = orders::get(&state.db.conn(), id)?;
    let bytes = reports::order_pdf(&order, &workshop(&state), &user.display_name, &now());
    let path = reports::save(&state.reports_dir, &format!("Reporte {}", order.number), "pdf", &bytes)?;
    let file = saved(path);
    orders::record_report(&state.db.conn(), user.id, id, &file.name)?;
    Ok(file)
}

#[tauri::command]
pub fn reports_summary(state: State<'_, AppState>, from: String, to: String) -> AppResult<PeriodSummary> {
    state.require_user()?;
    reports::period_summary(&state.db.conn(), &from, &to)
}

fn period_orders(state: &AppState, from: &str, to: &str) -> AppResult<Vec<OrderSummary>> {
    reports::validate_range(from, to)?;
    orders::list(&state.db.conn(), &OrderFilter { from: Some(from.into()), to: Some(to.into()), limit: Some(5000), ..Default::default() })
}

#[tauri::command]
pub fn reports_period_pdf(state: State<'_, AppState>, from: String, to: String) -> AppResult<SavedFile> {
    let user = state.require_user()?;
    let summary = reports::period_summary(&state.db.conn(), &from, &to)?;
    let list = period_orders(&state, &from, &to)?;
    let bytes = reports::period_pdf(&summary, &list, &workshop(&state), &user.display_name, &now());
    let path = reports::save(&state.reports_dir, &format!("Reporte del periodo {from} a {to}"), "pdf", &bytes)?;
    audit::record(&state.db.conn(), "period_report_generated", Severity::Info, Some(user.id), "", serde_json::json!({ "from": from, "to": to }))?;
    Ok(saved(path))
}

#[tauri::command]
pub fn reports_export_csv(state: State<'_, AppState>, from: String, to: String) -> AppResult<SavedFile> {
    let user = state.require_user()?;
    let list = period_orders(&state, &from, &to)?;
    let path = reports::save(&state.reports_dir, &format!("Ordenes {from} a {to}"), "csv", reports::orders_csv(&list).as_bytes())?;
    audit::record(&state.db.conn(), "orders_exported", Severity::Info, Some(user.id), "", serde_json::json!({ "from": from, "to": to, "count": list.len() }))?;
    Ok(saved(path))
}

#[derive(Debug, Serialize)]
pub struct ReportSettings {
    pub workshop_name: String,
    pub reports_dir: String,
}

#[tauri::command]
pub fn report_settings(state: State<'_, AppState>) -> AppResult<ReportSettings> {
    state.require_user()?;
    Ok(ReportSettings {
        workshop_name: settings::get(&state.db.conn(), settings::WORKSHOP_NAME)?.unwrap_or_default(),
        reports_dir: state.reports_dir.display().to_string(),
    })
}

#[tauri::command]
pub fn report_settings_set(state: State<'_, AppState>, workshop_name: String) -> AppResult<ReportSettings> {
    let user = state.require(Permission::ManageSettings)?;
    let name = workshop_name.trim();
    if name.chars().count() > 80 {
        return Err(AppError::Validation("El nombre del taller es demasiado largo (máximo 80 caracteres).".into()));
    }
    settings::set(&state.db.conn(), settings::WORKSHOP_NAME, Some(name).filter(|n| !n.is_empty()))?;
    audit::record(&state.db.conn(), "settings_changed", Severity::Info, Some(user.id), "", serde_json::json!({ "workshop_name": name }))?;
    report_settings(state)
}

/// Shows a generated report in Finder / Explorer. Only files inside the reports folder are accepted.
#[tauri::command]
pub fn reveal_report(state: State<'_, AppState>, path: String) -> AppResult<()> {
    state.require_user()?;
    let file = Path::new(&path).canonicalize().map_err(|_| AppError::NotFound("El archivo ya no existe.".into()))?;
    let dir = state.reports_dir.canonicalize().map_err(|_| AppError::NotFound("La carpeta de reportes no existe.".into()))?;
    if file.parent() != Some(dir.as_path()) || !file.is_file() {
        return Err(AppError::Forbidden("Solo se pueden mostrar archivos de la carpeta de reportes.".into()));
    }
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg("-R").arg(&file).spawn();
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("explorer").arg(format!("/select,{}", file.display())).spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let result = std::process::Command::new("xdg-open").arg(&dir).spawn();
    result.map(|_| ()).map_err(|e| AppError::Internal(format!("No se pudo abrir la carpeta: {e}")))
}
