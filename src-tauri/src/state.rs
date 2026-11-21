//! Application state shared by every command.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::database::Database;
use crate::error::{AppError, AppResult};
use crate::security::crypto::Vault;
use crate::services::adb::client::Tools;
use crate::services::adb::locator;
use crate::services::adb::runner::{Runner, SystemRunner};
use crate::services::devices::DeviceMonitor;
use crate::services::settings;
use crate::services::users::{self, Permission, User};

pub struct AppState {
    pub db: Database,
    pub vault: Vault,
    pub data_dir: PathBuf,
    pub monitor: DeviceMonitor,
    pub reset_challenges: crate::services::reset::Challenges,
    /// Where generated reports are saved (Downloads/AndroidMasters in normal use).
    pub reports_dir: PathBuf,
    runner: Arc<dyn Runner>,
    /// Signed-in user and the time of their last command.
    session: Mutex<Option<(i64, Instant)>>,
    idle_timeout: Duration,
}

/// A session closes after this much inactivity (shared workshop computers).
pub const SESSION_IDLE: Duration = Duration::from_secs(30 * 60);

impl AppState {
    pub fn open(data_dir: PathBuf) -> AppResult<Self> {
        std::fs::create_dir_all(&data_dir)?;
        // Customer names, consents and the encryption key live here: owner-only access.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&data_dir, std::fs::Permissions::from_mode(0o700))?;
        }
        let db = Database::open(&data_dir.join("androidmasters.sqlite3"))?;
        let vault = Vault::load_or_create(&data_dir.join("secret.key"))?;
        let reports_dir = data_dir.join("reports");
        Ok(Self { db, vault, reports_dir, data_dir, monitor: DeviceMonitor::default(), reset_challenges: Default::default(), runner: Arc::new(SystemRunner), session: Mutex::new(None), idle_timeout: SESSION_IDLE })
    }

    /// adb / fastboot resolved from the configured Platform-Tools folder or the standard locations.
    pub fn tools(&self) -> Tools {
        let configured = settings::get(&self.db.conn(), settings::PLATFORM_TOOLS_DIR).ok().flatten().map(PathBuf::from);
        Tools {
            runner: self.runner.clone(),
            adb: locator::find("adb", configured.as_deref()),
            fastboot: locator::find("fastboot", configured.as_deref()),
        }
    }

    pub fn set_session(&self, user_id: Option<i64>) {
        *self.session.lock().unwrap_or_else(|p| p.into_inner()) = user_id.map(|id| (id, Instant::now()));
    }

    #[cfg(test)]
    pub fn set_idle_timeout(&mut self, timeout: Duration) {
        self.idle_timeout = timeout;
    }

    /// Signed-in user id, or `Err(SessionExpired)` when the session went idle (it is closed and audited).
    fn session_user_id(&self, touch: bool) -> AppResult<Option<i64>> {
        let mut session = self.session.lock().unwrap_or_else(|p| p.into_inner());
        let Some((id, last)) = *session else { return Ok(None) };
        if last.elapsed() > self.idle_timeout {
            *session = None;
            drop(session);
            crate::services::audit::record(&self.db.conn(), "session_expired", crate::services::audit::Severity::Info, Some(id), "", serde_json::json!({}))?;
            return Err(AppError::SessionExpired("La sesión se cerró por inactividad. Vuelva a iniciar sesión.".into()));
        }
        if touch {
            *session = Some((id, Instant::now()));
        }
        Ok(Some(id))
    }

    /// Signed-in user, re-read from the database so role or deactivation changes apply immediately.
    /// An idle session counts as signed out.
    pub fn current_user(&self) -> AppResult<Option<User>> {
        let id = match self.session_user_id(false) {
            Ok(id) => id,
            Err(AppError::SessionExpired(_)) => None,
            Err(e) => return Err(e),
        };
        let Some(id) = id else { return Ok(None) };
        self.user_if_active(id)
    }

    fn user_if_active(&self, id: i64) -> AppResult<Option<User>> {
        let user = users::get(&self.db.conn(), id)?;
        if !user.active {
            self.set_session(None);
            return Ok(None);
        }
        Ok(Some(user))
    }

    /// Database id of a connected device that has been identified (stored).
    pub fn device_db_id(&self, id: &str) -> AppResult<i64> {
        let serial = self.monitor.serial_for(id)?;
        self.db
            .conn()
            .query_row("SELECT id FROM devices WHERE fingerprint = ?1", [self.vault.fingerprint(&serial)], |r| r.get(0))
            .map_err(|_| AppError::Device("El equipo todavía no está identificado: autorice la depuración USB en el teléfono.".into()))
    }

    /// The signed-in user; every call counts as activity and keeps the session open.
    pub fn require_user(&self) -> AppResult<User> {
        let id = self.session_user_id(true)?.ok_or_else(|| AppError::Forbidden("Inicie sesión para continuar.".into()))?;
        self.user_if_active(id)?.ok_or_else(|| AppError::Forbidden("Inicie sesión para continuar.".into()))
    }

    pub fn require(&self, permission: Permission) -> AppResult<User> {
        let user = self.require_user()?;
        if user.can(permission) {
            Ok(user)
        } else {
            Err(AppError::Forbidden("Su rol no permite esta acción. Consulte a un administrador.".into()))
        }
    }

    pub fn backups_dir(&self) -> PathBuf {
        self.data_dir.join("backups")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_session_expires_and_is_audited() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = AppState::open(dir.path().join("data")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(dir.path().join("data")).unwrap().permissions().mode() & 0o777, 0o700);
        }
        let user = users::create(&state.db.conn(), None, "ana", "Ana", "contraseña-larga-1", users::Role::Admin).unwrap();
        state.set_session(Some(user.id));
        assert_eq!(state.require_user().unwrap().id, user.id);
        state.set_idle_timeout(Duration::from_millis(30));
        std::thread::sleep(Duration::from_millis(60));
        assert!(matches!(state.require_user(), Err(AppError::SessionExpired(_))));
        assert!(matches!(state.require_user(), Err(AppError::Forbidden(_))), "closed after expiring");
        let audited: i64 = state.db.conn().query_row("SELECT COUNT(*) FROM audit_log WHERE event = 'session_expired'", [], |r| r.get(0)).unwrap();
        assert_eq!(audited, 1);
    }

    #[test]
    fn activity_keeps_the_session_open() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = AppState::open(dir.path().to_path_buf()).unwrap();
        let user = users::create(&state.db.conn(), None, "ana", "Ana", "contraseña-larga-1", users::Role::Admin).unwrap();
        state.set_idle_timeout(Duration::from_millis(80));
        state.set_session(Some(user.id));
        for _ in 0..4 {
            std::thread::sleep(Duration::from_millis(40));
            assert!(state.require_user().is_ok());
        }
    }
}
