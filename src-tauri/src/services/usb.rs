//! USB / ADB connection diagnostics built only from what adb and the system actually report.

use std::path::Path;

use crate::services::adb::client::Tools;
use crate::services::adb::parsers::Mode;
use crate::services::checks::{CheckItem, CheckStatus::*};
use crate::services::devices::{mode_label, DeviceView};

pub fn diagnose(tools: &Tools, device: Option<(&DeviceView, &str)>) -> Vec<CheckItem> {
    let mut items = Vec::new();
    match &tools.adb {
        None => {
            items.push(
                CheckItem::new("adb", "Herramienta adb", Fail, "No encontrada")
                    .advise(install_hint()),
            );
            return items;
        }
        Some(path) => {
            let version = tools.adb_version().unwrap_or_else(|| "versión desconocida".into());
            items.push(CheckItem::new("adb", "Herramienta adb", Pass, format!("{} · {version}", path.display())));
        }
    }
    items.push(match &tools.fastboot {
        Some(p) => CheckItem::new("fastboot", "Herramienta fastboot", Pass, p.display().to_string()),
        None => CheckItem::new("fastboot", "Herramienta fastboot", Info, "No encontrada")
            .advise("Solo hace falta para equipos en modo bootloader; viene con Platform-Tools."),
    });
    let listed = match tools.devices() {
        Ok(list) => {
            items.push(CheckItem::new("server", "Servidor ADB", Pass, format!("Responde · {} dispositivo(s) en la lista", list.len())));
            list
        }
        Err(e) => {
            items.push(CheckItem::new("server", "Servidor ADB", Fail, e.to_string())
                .advise("Use «Reiniciar servidor ADB». Si persiste, cierre otros programas de teléfonos que usen ADB (puerto 5037)."));
            return items;
        }
    };
    items.push(drivers_check());

    let Some((view, serial)) = device else {
        if listed.is_empty() {
            items.push(
                CheckItem::new("presence", "Dispositivo conectado", Fail, "Ningún dispositivo detectado").advise(
                    "Use un cable USB de datos (no solo de carga) y conéctelo directamente al equipo. En el teléfono: active Opciones de \
                     desarrollador › Depuración USB, desbloquee la pantalla y acepte «¿Permitir depuración USB?».",
                ),
            );
        }
        return items;
    };

    let present = listed.iter().find(|e| e.serial == serial);
    let Some(entry) = present else {
        items.push(CheckItem::new("presence", "Dispositivo conectado", Fail, "Se desconectó durante la comprobación")
            .advise("Vuelva a conectar el cable y revise el puerto del teléfono."));
        return items;
    };
    let (status, advice) = match entry.mode {
        Mode::Adb => (Pass, ""),
        Mode::Unauthorized => (Warn, "Desbloquee el teléfono y acepte «¿Permitir depuración USB?» (marque «Permitir siempre desde este equipo»). Si no aparece, en Opciones de desarrollador use «Revocar autorizaciones de depuración USB» y reconecte."),
        Mode::Offline => (Fail, "Desconecte y vuelva a conectar el cable; si continúa, use «Reiniciar servidor ADB»."),
        Mode::NoPermissions => (Fail, "El sistema no permite acceder al USB: en Linux instale las reglas udev de Android; en Windows instale el controlador USB del fabricante."),
        Mode::Recovery | Mode::Sideload | Mode::Rescue | Mode::Bootloader | Mode::Fastboot => (Info, "El teléfono no está en Android; algunas lecturas no están disponibles en este modo."),
        Mode::Unknown => (Warn, "Estado no reconocido: reconecte el dispositivo."),
    };
    items.push(CheckItem::new("mode", "Estado de la conexión", status, mode_label(entry.mode)).advise(advice));
    items.push(if view.network {
        CheckItem::new("transport", "Tipo de conexión", Info, "Red (TCP/IP), no USB")
    } else {
        CheckItem::new("transport", "Tipo de conexión", Pass, format!("USB{}", entry.usb_path.as_ref().map(|p| format!(" · puerto {p}")).unwrap_or_default()))
    });
    if entry.mode == Mode::Adb {
        items.push(match tools.ping(serial) {
            Ok(Some(latency)) if latency.as_millis() < 1500 => CheckItem::new("roundtrip", "Comunicación ADB", Pass, format!("Respuesta en {} ms", latency.as_millis())),
            Ok(Some(latency)) => CheckItem::new("roundtrip", "Comunicación ADB", Warn, format!("Respuesta lenta: {} ms", latency.as_millis()))
                .advise("Pruebe otro puerto USB del equipo (sin concentradores) u otro cable."),
            Ok(None) => CheckItem::new("roundtrip", "Comunicación ADB", Fail, "El teléfono no respondió").advise("Reconecte el cable o reinicie el servidor ADB."),
            Err(e) => CheckItem::new("roundtrip", "Comunicación ADB", Fail, e.to_string()),
        });
    }
    items
}

fn install_hint() -> &'static str {
    if cfg!(target_os = "macos") {
        "Instale Android SDK Platform-Tools (por ejemplo: brew install --cask android-platform-tools) o indique su carpeta en Configuración."
    } else if cfg!(windows) {
        "Instale Android SDK Platform-Tools (por ejemplo: winget install Google.PlatformTools) o indique su carpeta en Configuración."
    } else {
        "Instale Android SDK Platform-Tools (paquete android-sdk-platform-tools) o indique su carpeta en Configuración."
    }
}

fn drivers_check() -> CheckItem {
    if cfg!(target_os = "macos") {
        CheckItem::new("drivers", "Controladores USB", Pass, "macOS no necesita controladores para ADB")
    } else if cfg!(windows) {
        CheckItem::new("drivers", "Controladores USB", Info, "Windows necesita el controlador del fabricante")
            .advise("Si el teléfono no aparece o aparece «sin permisos», instale el controlador USB oficial del fabricante o el Google USB Driver.")
    } else {
        let rules = ["/etc/udev/rules.d", "/lib/udev/rules.d", "/usr/lib/udev/rules.d"].iter().any(|d| {
            std::fs::read_dir(Path::new(d)).is_ok_and(|mut it| it.any(|f| f.is_ok_and(|f| f.file_name().to_string_lossy().contains("android"))))
        });
        if rules {
            CheckItem::new("drivers", "Reglas udev", Pass, "Reglas de Android encontradas")
        } else {
            CheckItem::new("drivers", "Reglas udev", Warn, "No se encontraron reglas udev de Android").advise("Instale el paquete android-udev-rules.")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::adb::runner::fake::FakeRunner;
    use crate::services::adb::runner::CmdOutput;
    use std::path::PathBuf;
    use std::sync::Arc;

    fn tools(r: Arc<FakeRunner>) -> Tools {
        Tools { runner: r, adb: Some(PathBuf::from("/x/adb")), fastboot: None }
    }

    #[test]
    fn no_adb_stops_with_install_advice() {
        let t = Tools { runner: Arc::new(FakeRunner::default()), adb: None, fastboot: None };
        let items = diagnose(&t, None);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].status, Fail);
        assert!(items[0].recommendation.contains("Platform-Tools"));
    }

    #[test]
    fn empty_bus_explains_how_to_connect() {
        let r = Arc::new(FakeRunner::default());
        r.on("adb version", "Version 37.0.1\n").on("adb devices -l", "List of devices attached\n\n");
        let items = diagnose(&tools(r), None);
        let presence = items.iter().find(|i| i.key == "presence").unwrap();
        assert_eq!(presence.status, Fail);
        assert!(presence.recommendation.contains("Depuración USB"));
    }

    #[test]
    fn server_failure_is_reported() {
        let r = Arc::new(FakeRunner::default());
        r.respond("adb devices -l", CmdOutput { code: Some(1), stderr: "cannot connect to daemon".into(), ..Default::default() });
        let items = diagnose(&tools(r), None);
        assert_eq!(items.last().unwrap().key, "server");
        assert_eq!(items.last().unwrap().status, Fail);
    }
}
