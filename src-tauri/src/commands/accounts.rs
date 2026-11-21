use serde::Serialize;
use tauri::State;

use crate::error::AppResult;
use crate::services::accounts::{self, AccountCheck};
use crate::services::procedures::{self, BrandView};
use crate::services::transfer::{self, Checklist};
use crate::services::users::Permission;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct Catalog {
    pub brands: Vec<BrandView>,
    /// Brand module matching the selected connected device, if any.
    pub device_brand: Option<String>,
}

#[tauri::command]
pub fn procedures_catalog(state: State<'_, AppState>, device: Option<String>) -> AppResult<Catalog> {
    state.require_user()?;
    let device_brand = match device {
        Some(id) => {
            let v = state.monitor.view(&id)?;
            procedures::brand_for(v.manufacturer.as_deref(), v.brand.as_deref()).map(str::to_string)
        }
        None => None,
    };
    Ok(Catalog { brands: procedures::catalog(&state.db.conn())?, device_brand })
}

#[tauri::command]
pub fn procedure_review(state: State<'_, AppState>, procedure_id: String, note: String) -> AppResult<()> {
    let user = state.require(Permission::ReviewProcedures)?;
    procedures::mark_reviewed(&state.db.conn(), user.id, &procedure_id, &note)
}

#[derive(Debug, Serialize)]
pub struct AccountCheckResult {
    pub check: AccountCheck,
    pub previous: Option<AccountCheck>,
    /// True when an earlier check found protected accounts and this one finds none.
    pub removal_verified: bool,
}

/// Reads account types and counts from the connected device and stores the check.
#[tauri::command]
pub fn accounts_check(state: State<'_, AppState>, device: String) -> AppResult<AccountCheckResult> {
    let user = state.require_user()?;
    let device_id = state.device_db_id(&device)?;
    let serial = state.monitor.serial_for(&device)?;
    let summary = accounts::read_account_types(&state.tools(), &serial)?;
    let conn = state.db.conn();
    let previous = accounts::checks_for(&conn, device_id, 1)?.into_iter().next();
    let check = accounts::record_check(&conn, device_id, user.id, &summary)?;
    let removal_verified = previous.as_ref().is_some_and(|p| p.protected_count > 0) && check.protected_count == 0;
    Ok(AccountCheckResult { check, previous, removal_verified })
}

#[tauri::command]
pub fn accounts_history(state: State<'_, AppState>, device: String) -> AppResult<Vec<AccountCheck>> {
    state.require_user()?;
    let device_id = state.device_db_id(&device)?;
    accounts::checks_for(&state.db.conn(), device_id, 20)
}

#[tauri::command]
pub fn checklist_open(state: State<'_, AppState>, device: Option<String>, device_label: String) -> AppResult<Checklist> {
    let user = state.require_user()?;
    let device_id = match device {
        Some(id) => Some(state.device_db_id(&id)?),
        None => None,
    };
    transfer::open(&state.db.conn(), user.id, device_id, &device_label)
}

#[tauri::command]
pub fn checklist_set(state: State<'_, AppState>, id: i64, key: String, done: bool, note: String) -> AppResult<Checklist> {
    let user = state.require_user()?;
    transfer::set_item(&state.db.conn(), user.id, id, &key, done, &note)
}
