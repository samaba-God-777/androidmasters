use serde::Serialize;
use tauri::State;

use crate::error::AppResult;
use crate::services::users::{self, Role, User};
use crate::services::audit::{self, Severity};
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct AuthStatus {
    pub has_users: bool,
    pub user: Option<User>,
}

#[tauri::command]
pub fn auth_status(state: State<'_, AppState>) -> AppResult<AuthStatus> {
    Ok(AuthStatus { has_users: users::has_users(&state.db.conn())?, user: state.current_user()? })
}

/// First run only: creates the administrator account and signs it in.
#[tauri::command]
pub fn create_first_admin(state: State<'_, AppState>, username: String, display_name: String, password: String) -> AppResult<User> {
    let conn = state.db.conn();
    let user = users::create(&conn, None, &username, &display_name, &password, Role::Admin)?;
    users::record_login(&conn, user.id)?;
    state.set_session(Some(user.id));
    users::get(&conn, user.id)
}

#[tauri::command]
pub fn login(state: State<'_, AppState>, username: String, password: String) -> AppResult<User> {
    let user = users::authenticate(&state.db.conn(), &username, &password)?;
    state.set_session(Some(user.id));
    Ok(user)
}

#[tauri::command]
pub fn logout(state: State<'_, AppState>) -> AppResult<()> {
    if let Some(user) = state.current_user()? {
        audit::record(&state.db.conn(), "logout", Severity::Info, Some(user.id), "", serde_json::json!({}))?;
    }
    state.set_session(None);
    Ok(())
}

#[tauri::command]
pub fn change_password(state: State<'_, AppState>, current: String, new_password: String) -> AppResult<()> {
    let user = state.require_user()?;
    users::change_own_password(&state.db.conn(), &user, &current, &new_password)
}
