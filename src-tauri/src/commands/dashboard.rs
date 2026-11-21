use tauri::State;

use crate::error::AppResult;
use crate::services::dashboard::{self, DashboardStats};
use crate::state::AppState;

#[tauri::command]
pub fn dashboard_stats(state: State<'_, AppState>) -> AppResult<DashboardStats> {
    state.require_user()?;
    dashboard::stats(&state.db.conn())
}
