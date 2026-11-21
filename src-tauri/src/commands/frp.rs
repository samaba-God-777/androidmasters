use serde::Serialize;
use tauri::State;

use crate::error::AppResult;
use crate::services::accounts::AccountSummary;
use crate::services::checks::CheckItem;
use crate::services::frp::{self, Assessment, Observation, RecordedAssessment};
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct FrpIndicators {
    pub items: Vec<CheckItem>,
    pub accounts: Option<AccountSummary>,
}

/// Read-only indicators from a connected device (account types and counts only, never names).
#[tauri::command]
pub fn frp_indicators(state: State<'_, AppState>, id: String) -> AppResult<FrpIndicators> {
    state.require_user()?;
    let view = state.monitor.view(&id)?;
    let serial = state.monitor.serial_for(&id)?;
    let (items, accounts) = frp::indicators(&state.tools(), &view, &serial);
    Ok(FrpIndicators { items, accounts })
}

/// Evaluates the answers without saving anything (live preview in the form).
#[tauri::command]
pub fn frp_assess(state: State<'_, AppState>, observation: Observation) -> AppResult<Assessment> {
    state.require_user()?;
    Ok(frp::assess(&observation))
}

#[derive(Debug, Serialize)]
pub struct FrpRecorded {
    pub id: i64,
    pub assessment: Assessment,
}

#[tauri::command]
pub fn frp_record(
    state: State<'_, AppState>,
    device: Option<String>,
    device_label: String,
    observation: Observation,
    notes: String,
) -> AppResult<FrpRecorded> {
    let user = state.require_user()?;
    let conn = state.db.conn();
    let device_id = match &device {
        Some(id) => {
            let serial = state.monitor.serial_for(id)?;
            conn.query_row("SELECT id FROM devices WHERE fingerprint = ?1", [state.vault.fingerprint(&serial)], |r| r.get(0)).ok()
        }
        None => None,
    };
    let label = match (&device, device_id) {
        (Some(id), None) => state.monitor.view(id).map(|v| v.model.unwrap_or(v.serial_masked)).unwrap_or_default(),
        _ => device_label,
    };
    let (id, assessment) = frp::record(&conn, user.id, device_id, &label, &observation, &notes)?;
    Ok(FrpRecorded { id, assessment })
}

#[tauri::command]
pub fn frp_history(state: State<'_, AppState>) -> AppResult<Vec<RecordedAssessment>> {
    state.require_user()?;
    frp::history(&state.db.conn(), 50)
}
