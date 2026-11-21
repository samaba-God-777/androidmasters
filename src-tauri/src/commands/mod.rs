//! Tauri commands: the only interface between the UI and the backend.
//! Every command except `auth_status`, `login` and first-run setup requires a signed-in user.

pub mod accounts;
pub mod auth;
pub mod dashboard;
pub mod devices;
pub mod frp;
pub mod orders;
pub mod reset;
pub mod system;
pub mod users;
