use serde::Serialize;
use tauri::State;

use crate::error::AppResult;
use crate::services::audit::{self, Severity};
use crate::services::users::Permission;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct AppInfo {
    pub version: String,
    pub schema_version: i64,
    pub database_ok: bool,
    pub platform: String,
    pub arch: String,
}

#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> AppResult<AppInfo> {
    state.require_user()?;
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        schema_version: state.db.schema_version()?,
        database_ok: state.db.integrity_ok()?,
        platform: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
    })
}

#[derive(Debug, Serialize)]
pub struct BackupResult {
    pub path: String,
    pub size_bytes: u64,
}

/// Consistent copy of the database into the app's `backups` folder.
#[tauri::command]
pub fn create_backup(state: State<'_, AppState>) -> AppResult<BackupResult> {
    let user = state.require(Permission::ManageBackups)?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let dest = state.backups_dir().join(format!("androidmasters-{stamp}.sqlite3"));
    state.db.backup_to(&dest)?;
    audit::record(&state.db.conn(), "backup_created", Severity::Info, Some(user.id), "",
                  serde_json::json!({ "file": dest.file_name().map(|f| f.to_string_lossy().to_string()) }))?;
    Ok(BackupResult { size_bytes: std::fs::metadata(&dest)?.len(), path: dest.display().to_string() })
}
