use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::error::{AppError, AppResult};
use crate::services::audit::{self, Severity};
use crate::services::checks::CheckItem;
use crate::services::devices::{DeviceView, Snapshot};
use crate::services::users::Permission;
use crate::services::{settings, usb};
use crate::state::AppState;
use crate::util::now;

pub const EVENT_SNAPSHOT: &str = "devices://snapshot";
pub const EVENT_CONNECTION: &str = "devices://event";

#[tauri::command]
pub fn devices_snapshot(state: State<'_, AppState>) -> AppResult<Snapshot> {
    state.require_user()?;
    Ok(state.monitor.snapshot())
}

#[tauri::command]
pub fn device_refresh(app: AppHandle, state: State<'_, AppState>, id: String) -> AppResult<DeviceView> {
    state.require_user()?;
    let view = state.monitor.refresh(&id, &state.tools(), &state.db.conn(), &state.vault)?;
    let _ = app.emit(EVENT_SNAPSHOT, state.monitor.snapshot());
    Ok(view)
}

/// Full serial number, for administrators only. Every reveal is audited.
#[tauri::command]
pub fn device_reveal_serial(state: State<'_, AppState>, id: String) -> AppResult<String> {
    let user = state.require(Permission::RevealSerial)?;
    let serial = state.monitor.serial_for(&id)?;
    audit::record(&state.db.conn(), "serial_revealed", Severity::Warning, Some(user.id), &state.vault.fingerprint(&serial), serde_json::json!({}))?;
    Ok(serial)
}

#[tauri::command]
pub fn usb_diagnose(state: State<'_, AppState>, id: Option<String>) -> AppResult<Vec<CheckItem>> {
    let user = state.require_user()?;
    let tools = state.tools();
    let device = match &id {
        Some(id) => Some((state.monitor.view(id)?, state.monitor.serial_for(id)?)),
        None => None,
    };
    let started = now();
    let items = usb::diagnose(&tools, device.as_ref().map(|(v, s)| (v, s.as_str())));
    let failed = items.iter().filter(|i| i.status == crate::services::checks::CheckStatus::Fail).count();
    let conn = state.db.conn();
    let device_id: Option<i64> = device.as_ref().and_then(|(_, serial)| {
        conn.query_row("SELECT id FROM devices WHERE fingerprint = ?1", [state.vault.fingerprint(serial)], |r| r.get(0)).ok()
    });
    conn.execute(
        "INSERT INTO operations (started_at, finished_at, device_id, user_id, operation, category, result, details_json) \
         VALUES (?1, ?2, ?3, ?4, 'Diagnóstico de conexión USB', 'usb', ?5, ?6)",
        rusqlite::params![started, now(), device_id, user.id, if failed == 0 { "success" } else { "failed" },
                          serde_json::json!({ "checks": items.len(), "failed": failed }).to_string()],
    )?;
    Ok(items)
}

#[tauri::command]
pub fn adb_restart_server(state: State<'_, AppState>) -> AppResult<()> {
    let user = state.require_user()?;
    state.tools().restart_server()?;
    audit::record(&state.db.conn(), "adb_server_restarted", Severity::Info, Some(user.id), "", serde_json::json!({}))
}

#[derive(Debug, Serialize)]
pub struct ToolsSettings {
    pub platform_tools_dir: Option<String>,
    pub adb_path: Option<String>,
    pub fastboot_path: Option<String>,
}

#[tauri::command]
pub fn tools_settings(state: State<'_, AppState>) -> AppResult<ToolsSettings> {
    state.require_user()?;
    let tools = state.tools();
    Ok(ToolsSettings {
        platform_tools_dir: settings::get(&state.db.conn(), settings::PLATFORM_TOOLS_DIR)?,
        adb_path: tools.adb.map(|p| p.display().to_string()),
        fastboot_path: tools.fastboot.map(|p| p.display().to_string()),
    })
}

/// Sets (or clears, with `None`) the Platform-Tools folder. The folder must contain adb.
#[tauri::command]
pub fn tools_settings_set(state: State<'_, AppState>, dir: Option<String>) -> AppResult<ToolsSettings> {
    let user = state.require(Permission::ManageUsers)?;
    let dir = dir.map(|d| d.trim().to_string()).filter(|d| !d.is_empty());
    if let Some(d) = &dir {
        if crate::services::adb::locator::find("adb", Some(std::path::Path::new(d))).filter(|p| p.starts_with(d)).is_none() {
            return Err(AppError::Validation("Esa carpeta no contiene adb. Elija la carpeta «platform-tools».".into()));
        }
    }
    settings::set(&state.db.conn(), settings::PLATFORM_TOOLS_DIR, dir.as_deref())?;
    audit::record(&state.db.conn(), "settings_changed", Severity::Info, Some(user.id), "", serde_json::json!({ "platform_tools_dir": dir }))?;
    tools_settings(state)
}
