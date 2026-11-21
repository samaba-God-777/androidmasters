use tauri::State;

use crate::error::AppResult;
use crate::services::users::{self, Permission, Role, User};
use crate::state::AppState;

#[tauri::command]
pub fn users_list(state: State<'_, AppState>) -> AppResult<Vec<User>> {
    state.require(Permission::ManageUsers)?;
    users::list(&state.db.conn())
}

#[tauri::command]
pub fn users_create(state: State<'_, AppState>, username: String, display_name: String, password: String, role: Role) -> AppResult<User> {
    let actor = state.require(Permission::ManageUsers)?;
    users::create(&state.db.conn(), Some(&actor), &username, &display_name, &password, role)
}

#[tauri::command]
pub fn users_set_active(state: State<'_, AppState>, id: i64, active: bool) -> AppResult<User> {
    let actor = state.require(Permission::ManageUsers)?;
    users::set_active(&state.db.conn(), &actor, id, active)
}

#[tauri::command]
pub fn users_set_role(state: State<'_, AppState>, id: i64, role: Role) -> AppResult<User> {
    let actor = state.require(Permission::ManageUsers)?;
    users::set_role(&state.db.conn(), &actor, id, role)
}

#[tauri::command]
pub fn users_reset_password(state: State<'_, AppState>, id: i64, password: String) -> AppResult<()> {
    let actor = state.require(Permission::ManageUsers)?;
    users::reset_password(&state.db.conn(), &actor, id, &password)
}
