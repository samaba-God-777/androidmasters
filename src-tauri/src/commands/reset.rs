use serde::Serialize;
use tauri::State;

use crate::error::{AppError, AppResult};
use crate::services::adb::parsers::Mode;
use crate::services::reset::{self, Method, Preflight, ResetRecord};
use crate::state::AppState;

fn resolve(state: &AppState, device: &Option<String>) -> AppResult<(Option<i64>, Option<crate::services::devices::DeviceView>)> {
    match device {
        Some(id) => {
            let view = state.monitor.view(id)?;
            let db_id = state.device_db_id(id).ok();
            Ok((db_id, Some(view)))
        }
        None => Ok((None, None)),
    }
}

#[tauri::command]
pub fn reset_preflight(state: State<'_, AppState>, device: Option<String>, device_label: String) -> AppResult<Preflight> {
    state.require_user()?;
    let (db_id, view) = resolve(&state, &device)?;
    reset::preflight(&state.db.conn(), db_id, &device_label, view.as_ref())
}

#[tauri::command]
pub fn reset_prepare(state: State<'_, AppState>, device: Option<String>, device_label: String, method: Method) -> AppResult<ResetRecord> {
    let user = state.require_user()?;
    let (db_id, view) = resolve(&state, &device)?;
    reset::prepare(&state.db.conn(), user.id, db_id, &device_label, view.as_ref(), method)
}

#[derive(Debug, Serialize)]
pub struct Challenge {
    pub token: String,
    pub phrase: String,
    pub expires_in_seconds: u64,
}

#[tauri::command]
pub fn reset_challenge(state: State<'_, AppState>, id: i64) -> AppResult<Challenge> {
    state.require_user()?;
    reset::get(&state.db.conn(), id)?;
    let (token, phrase) = state.reset_challenges.issue(id);
    Ok(Challenge { token, phrase, expires_in_seconds: reset::CHALLENGE_TTL.as_secs() })
}

#[tauri::command]
pub fn reset_confirm(state: State<'_, AppState>, id: i64, token: String, typed: String, owner_name: String) -> AppResult<ResetRecord> {
    let user = state.require_user()?;
    reset::confirm(&state.db.conn(), &state.reset_challenges, user.id, id, &token, &typed, &owner_name)
}

/// Supporting action on the confirmed device: open Settings or reboot into Recovery. The wipe itself is
/// always confirmed on the phone by the owner/technician.
#[tauri::command]
pub fn reset_assist(state: State<'_, AppState>, id: i64, device: String, action: String) -> AppResult<ResetRecord> {
    let user = state.require_user()?;
    let record = reset::get(&state.db.conn(), id)?;
    let db_id = state.device_db_id(&device)?;
    if record.device_id != Some(db_id) {
        return Err(AppError::Validation("El equipo conectado no es el que se confirmó para este restablecimiento.".into()));
    }
    let view = state.monitor.view(&device)?;
    if view.mode != Mode::Adb {
        return Err(AppError::Device("El equipo debe estar en Android con la depuración USB autorizada.".into()));
    }
    let serial = state.monitor.serial_for(&device)?;
    let args: &[&str] = match action.as_str() {
        "open_settings" => &["shell", "am", "start", "-a", "android.settings.SETTINGS"],
        "reboot_recovery" => &["reboot", "recovery"],
        _ => return Err(AppError::Validation("Acción no permitida.".into())),
    };
    let out = state.tools().adb(Some(&serial), args, std::time::Duration::from_secs(20))?;
    let ok = out.ok();
    let detail = if ok { String::new() } else { out.failure() };
    let record = reset::record_action(&state.db.conn(), user.id, id, &action, ok, &detail)?;
    if !ok {
        return Err(AppError::Device(format!("El teléfono no aceptó la acción: {detail}")));
    }
    Ok(record)
}

/// The technician states that the erase was started on the phone (no app action involved).
#[tauri::command]
pub fn reset_manual_start(state: State<'_, AppState>, id: i64) -> AppResult<ResetRecord> {
    let user = state.require_user()?;
    reset::record_action(&state.db.conn(), user.id, id, "manual_start", true, "Iniciado en el equipo por el técnico o el propietario")
}

#[tauri::command]
pub fn reset_verify(
    state: State<'_, AppState>,
    id: i64,
    device: Option<String>,
    setup_wizard_shown: bool,
    asks_previous_account: bool,
    note: String,
) -> AppResult<ResetRecord> {
    let user = state.require_user()?;
    let observed = device.as_deref().and_then(|d| state.monitor.last_disconnect(d));
    reset::verify(&state.db.conn(), user.id, id, setup_wizard_shown, asks_previous_account, &note, observed)
}

#[tauri::command]
pub fn reset_cancel(state: State<'_, AppState>, id: i64) -> AppResult<ResetRecord> {
    let user = state.require_user()?;
    reset::cancel(&state.db.conn(), user.id, id)
}

#[tauri::command]
pub fn resets_list(state: State<'_, AppState>) -> AppResult<Vec<ResetRecord>> {
    state.require_user()?;
    reset::list(&state.db.conn(), 30)
}
