//! adb / fastboot client. Every command is checked against `security::policy` before it runs:
//! even AndroidMasters' own code cannot execute something outside the allow-list.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::error::{AppError, AppResult};
use crate::security::policy::{self, Risk};

use super::parsers::{self, AdbEntry, Battery, Storage};
use super::runner::{CmdOutput, Runner};

const QUICK: Duration = Duration::from_secs(10);
const NORMAL: Duration = Duration::from_secs(25);

#[derive(Clone)]
pub struct Tools {
    pub runner: Arc<dyn Runner>,
    pub adb: Option<PathBuf>,
    pub fastboot: Option<PathBuf>,
}

impl Tools {
    fn adb_path(&self) -> AppResult<&PathBuf> {
        self.adb.as_ref().ok_or_else(|| {
            AppError::Device("No se encontró adb. Instale Android SDK Platform-Tools o indique su carpeta en Configuración.".into())
        })
    }

    /// Runs `adb [-s serial] args…` after validating `args` with the command policy.
    pub fn adb(&self, serial: Option<&str>, args: &[&str], timeout: Duration) -> AppResult<CmdOutput> {
        let decision = policy::validate_adb(args);
        if !decision.allowed {
            return Err(AppError::Forbidden(decision.reason));
        }
        if decision.risk >= Risk::Destructive {
            return Err(AppError::Forbidden("Las operaciones destructivas no se ejecutan desde este cliente.".into()));
        }
        let mut argv: Vec<String> = Vec::new();
        if let Some(s) = serial {
            argv.extend(["-s".to_string(), s.to_string()]);
        }
        argv.extend(args.iter().map(|a| a.to_string()));
        Ok(self.runner.run(self.adb_path()?, &argv, timeout))
    }

    pub fn fastboot(&self, serial: Option<&str>, args: &[&str], timeout: Duration) -> AppResult<Option<CmdOutput>> {
        let Some(path) = &self.fastboot else { return Ok(None) };
        let decision = policy::validate_fastboot(args);
        if !decision.allowed || decision.risk > Risk::ReadOnly {
            return Err(AppError::Forbidden(decision.reason));
        }
        let mut argv: Vec<String> = Vec::new();
        if let Some(s) = serial {
            argv.extend(["-s".to_string(), s.to_string()]);
        }
        argv.extend(args.iter().map(|a| a.to_string()));
        Ok(Some(self.runner.run(path, &argv, timeout)))
    }

    pub fn adb_version(&self) -> Option<String> {
        let out = self.adb(None, &["version"], QUICK).ok()?;
        if !out.ok() {
            return None;
        }
        let lines: Vec<&str> = out.stdout.lines().collect();
        let version = lines.iter().find_map(|l| l.strip_prefix("Version ")).or_else(|| {
            lines.first().and_then(|l| l.strip_prefix("Android Debug Bridge version "))
        });
        version.map(|v| v.trim().to_string())
    }

    pub fn start_server(&self) -> AppResult<CmdOutput> {
        self.adb(None, &["start-server"], NORMAL)
    }

    pub fn restart_server(&self) -> AppResult<()> {
        let _ = self.adb(None, &["kill-server"], QUICK)?;
        let out = self.start_server()?;
        if out.ok() { Ok(()) } else { Err(AppError::Device(format!("No se pudo iniciar el servidor ADB: {}", out.failure()))) }
    }

    pub fn devices(&self) -> AppResult<Vec<AdbEntry>> {
        let out = self.adb(None, &["devices", "-l"], NORMAL)?;
        if !out.ok() {
            return Err(AppError::Device(format!("adb devices falló: {}", out.failure())));
        }
        Ok(parsers::parse_devices(&out.stdout))
    }

    pub fn fastboot_devices(&self) -> AppResult<Vec<String>> {
        Ok(match self.fastboot(None, &["devices"], NORMAL)? {
            Some(out) if out.ok() => parsers::parse_fastboot_devices(&out.stdout),
            _ => Vec::new(),
        })
    }

    pub fn getvar(&self, serial: &str, name: &str) -> Option<String> {
        let out = self.fastboot(Some(serial), &["getvar", name], QUICK).ok()??;
        parsers::parse_getvar(&out.combined(), name)
    }

    pub fn getprop(&self, serial: &str) -> AppResult<std::collections::BTreeMap<String, String>> {
        let out = self.adb(Some(serial), &["shell", "getprop"], NORMAL)?;
        if !out.ok() {
            return Err(device_error(&out, serial));
        }
        Ok(parsers::parse_getprop(&out.stdout))
    }

    pub fn battery(&self, serial: &str) -> Option<Battery> {
        let out = self.adb(Some(serial), &["shell", "dumpsys", "battery"], NORMAL).ok()?;
        (out.ok() && out.stdout.contains("level")).then(|| parsers::parse_battery(&out.stdout))
    }

    pub fn storage(&self, serial: &str) -> Option<Storage> {
        let out = self.adb(Some(serial), &["shell", "df", "-k", "/data"], NORMAL).ok()?;
        out.ok().then(|| parsers::parse_df(&out.stdout, "/data")).flatten()
    }

    /// Round trip through the ADB transport. Returns the latency when the device answered.
    pub fn ping(&self, serial: &str) -> AppResult<Option<Duration>> {
        let out = self.adb(Some(serial), &["shell", "echo", "androidmasters-ping"], QUICK)?;
        Ok((out.ok() && out.stdout.contains("androidmasters-ping")).then_some(out.duration))
    }

    pub fn get_state(&self, serial: &str) -> Option<String> {
        let out = self.adb(Some(serial), &["get-state"], QUICK).ok()?;
        out.ok().then(|| out.stdout.trim().to_string())
    }
}

fn device_error(out: &CmdOutput, serial: &str) -> AppError {
    let text = out.combined().to_lowercase();
    if text.contains("not found") {
        AppError::Device(format!("El dispositivo {} ya no está conectado.", crate::security::crypto::mask_serial(serial)))
    } else if text.contains("unauthorized") {
        AppError::Device("Depuración USB no autorizada: desbloquee el teléfono y acepte «¿Permitir depuración USB?».".into())
    } else if text.contains("offline") {
        AppError::Device("El dispositivo está fuera de línea. Vuelva a conectar el cable.".into())
    } else {
        AppError::Device(out.failure())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::adb::runner::fake::FakeRunner;

    fn tools(runner: Arc<FakeRunner>) -> Tools {
        Tools { runner, adb: Some(PathBuf::from("/x/adb")), fastboot: Some(PathBuf::from("/x/fastboot")) }
    }

    #[test]
    fn rejects_commands_outside_the_policy_before_running_them() {
        let runner = Arc::new(FakeRunner::default());
        let t = tools(runner.clone());
        assert!(matches!(t.adb(Some("S"), &["shell", "pm", "uninstall", "com.google.android.gsf.login"], QUICK), Err(AppError::Forbidden(_))));
        assert!(matches!(t.adb(Some("S"), &["sideload", "ota.zip"], QUICK), Err(AppError::Forbidden(_))));
        assert!(matches!(t.fastboot(Some("S"), &["erase", "frp"], QUICK), Err(AppError::Forbidden(_))));
        assert!(matches!(t.fastboot(Some("S"), &["flashing", "unlock"], QUICK), Err(AppError::Forbidden(_))));
        assert!(runner.calls.lock().unwrap().is_empty(), "nothing reached the process runner");
    }

    #[test]
    fn version_and_missing_tool() {
        let runner = Arc::new(FakeRunner::default());
        runner.on("adb version", "Android Debug Bridge version 1.0.41\nVersion 37.0.1-15733141\nInstalled as /opt/homebrew/bin/adb\n");
        assert_eq!(tools(runner.clone()).adb_version().as_deref(), Some("37.0.1-15733141"));
        let none = Tools { runner, adb: None, fastboot: None };
        assert!(matches!(none.devices(), Err(AppError::Device(m)) if m.contains("Platform-Tools")));
        assert!(none.fastboot_devices().unwrap().is_empty());
    }

    #[test]
    fn device_errors_are_explained() {
        let runner = Arc::new(FakeRunner::default());
        runner.respond("adb -s R58 shell getprop", CmdOutput { code: Some(1), stderr: "error: device unauthorized.".into(), ..Default::default() });
        let err = tools(runner).getprop("R58").unwrap_err().to_string();
        assert!(err.contains("Permitir depuración USB"));
    }
}
