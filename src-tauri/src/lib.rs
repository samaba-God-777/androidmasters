//! AndroidMasters backend.
//!
//! Module map (see docs/ARCHITECTURE.md):
//! * `commands`  — Tauri commands, the only surface exposed to the UI.
//! * `services`  — business logic (ADB/USB, FRP, accounts, reset, orders, reports).
//! * `security`  — command policy, identifier encryption, passwords.
//! * `database`  — SQLite with embedded, versioned migrations (`../migrations`) and backups.

pub mod commands;
pub mod database;
pub mod error;
pub mod security;
pub mod services;
pub mod state;
pub mod util;

use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Development builds can use a throwaway data folder (tests, screenshots); release builds cannot.
            let dev_dir = std::env::var_os("ANDROIDMASTERS_DATA_DIR").filter(|_| cfg!(debug_assertions));
            let data_dir = match &dev_dir {
                Some(dir) => std::path::PathBuf::from(dir),
                None => app.path().app_data_dir()?,
            };
            let mut state = state::AppState::open(data_dir).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
            // Reports go to the user's Downloads folder (the throwaway data folder keeps them in development).
            if dev_dir.is_none() {
                if let Ok(downloads) = app.path().download_dir() {
                    state.reports_dir = downloads.join("AndroidMasters");
                }
            }
            app.manage(state);
            start_device_monitor(app.handle().clone());
            #[cfg(target_os = "macos")]
            app.set_menu(spanish_menu(app.handle())?)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::auth::auth_status,
            commands::auth::create_first_admin,
            commands::auth::login,
            commands::auth::logout,
            commands::auth::change_password,
            commands::users::users_list,
            commands::users::users_create,
            commands::users::users_set_active,
            commands::users::users_set_role,
            commands::users::users_reset_password,
            commands::dashboard::dashboard_stats,
            commands::devices::devices_snapshot,
            commands::devices::device_refresh,
            commands::devices::device_reveal_serial,
            commands::devices::usb_diagnose,
            commands::devices::adb_restart_server,
            commands::devices::tools_settings,
            commands::devices::tools_settings_set,
            commands::frp::frp_indicators,
            commands::frp::frp_assess,
            commands::frp::frp_record,
            commands::frp::frp_history,
            commands::accounts::procedures_catalog,
            commands::accounts::procedure_review,
            commands::accounts::accounts_check,
            commands::accounts::accounts_history,
            commands::accounts::checklist_open,
            commands::accounts::checklist_set,
            commands::reset::reset_preflight,
            commands::reset::reset_prepare,
            commands::reset::reset_challenge,
            commands::reset::reset_confirm,
            commands::reset::reset_assist,
            commands::reset::reset_manual_start,
            commands::reset::reset_verify,
            commands::reset::reset_cancel,
            commands::reset::resets_list,
            commands::orders::orders_list,
            commands::orders::order_get,
            commands::orders::order_create,
            commands::orders::order_consent,
            commands::orders::order_add_entry,
            commands::orders::order_set_status,
            commands::orders::order_set_notes,
            commands::orders::order_candidates,
            commands::orders::order_link,
            commands::orders::order_report,
            commands::orders::reports_summary,
            commands::orders::reports_period_pdf,
            commands::orders::reports_export_csv,
            commands::orders::report_settings,
            commands::orders::report_settings_set,
            commands::orders::reveal_report,
            commands::system::app_info,
            commands::system::create_backup,
        ])
        .run(tauri::generate_context!())
        .expect("error while running AndroidMasters");
}

/// macOS menu bar in Spanish (Windows windows have no menu bar).
#[cfg(target_os = "macos")]
fn spanish_menu(app: &tauri::AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::{AboutMetadata, Menu, PredefinedMenuItem as Item, Submenu};
    let about = AboutMetadata { name: Some("AndroidMasters".into()), ..Default::default() };
    let app_menu = Submenu::with_items(app, "AndroidMasters", true, &[
        &Item::about(app, Some("Acerca de AndroidMasters"), Some(about))?,
        &Item::separator(app)?,
        &Item::hide(app, Some("Ocultar AndroidMasters"))?,
        &Item::hide_others(app, Some("Ocultar otros"))?,
        &Item::show_all(app, Some("Mostrar todo"))?,
        &Item::separator(app)?,
        &Item::quit(app, Some("Salir de AndroidMasters"))?,
    ])?;
    let edit = Submenu::with_items(app, "Edición", true, &[
        &Item::undo(app, Some("Deshacer"))?,
        &Item::redo(app, Some("Rehacer"))?,
        &Item::separator(app)?,
        &Item::cut(app, Some("Cortar"))?,
        &Item::copy(app, Some("Copiar"))?,
        &Item::paste(app, Some("Pegar"))?,
        &Item::select_all(app, Some("Seleccionar todo"))?,
    ])?;
    let view = Submenu::with_items(app, "Ver", true, &[&Item::fullscreen(app, Some("Pantalla completa"))?])?;
    let window = Submenu::with_items(app, "Ventana", true, &[
        &Item::minimize(app, Some("Minimizar"))?,
        &Item::maximize(app, Some("Zoom"))?,
        &Item::separator(app)?,
        &Item::close_window(app, Some("Cerrar ventana"))?,
    ])?;
    Menu::with_items(app, &[&app_menu, &edit, &view, &window])
}

/// Background USB polling: emits a full snapshot when something changed and one event per
/// connection, disconnection, mode change or tool error.
fn start_device_monitor(app: tauri::AppHandle) {
    std::thread::Builder::new()
        .name("device-monitor".into())
        .spawn(move || loop {
            let state = app.state::<state::AppState>();
            let tools = state.tools();
            let result = {
                let conn = state.db.conn();
                state.monitor.scan(&tools, &conn, &state.vault)
            };
            if result.changed {
                let _ = app.emit(commands::devices::EVENT_SNAPSHOT, state.monitor.snapshot());
            }
            for event in result.events {
                let _ = app.emit(commands::devices::EVENT_CONNECTION, event);
            }
            let interval = services::settings::poll_interval_ms(&state.db.conn());
            std::thread::sleep(std::time::Duration::from_millis(interval));
        })
        .expect("spawn device monitor");
}
